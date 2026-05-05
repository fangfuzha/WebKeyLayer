//! 鼠标监听模块
//!
//! 实现鼠标事件监听（按键、移动、滚轮），用于在 v1.0 支持鼠标事件。
//!
//! 设计约束：
//! - 使用平台 API 获取鼠标按键/移动/滚轮事件
//! - 线程安全状态共享（Arc<Mutex<>>）
//!
use crate::websocket_server::WebSocketServer;
use crate::{Error, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle as TokioJoinHandle;
use tracing::{debug, info, warn};

#[cfg(windows)]
use std::thread::JoinHandle as ThreadJoinHandle;

#[cfg(windows)]
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};

#[cfg(windows)]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

#[cfg(windows)]
use windows::Win32::System::Threading::GetCurrentThreadId;

#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, PeekMessageW, PostThreadMessageW, SetWindowsHookExW,
    UnhookWindowsHookEx, HC_ACTION, HHOOK, MSG, MSLLHOOKSTRUCT, PM_NOREMOVE, WH_MOUSE_LL,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL,
    WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_XBUTTONDOWN, WM_XBUTTONUP, XBUTTON1, XBUTTON2,
};

const MOUSE_BUTTON_LEFT: u8 = 1;
const MOUSE_BUTTON_RIGHT: u8 = 2;
const MOUSE_BUTTON_MIDDLE: u8 = 3;
const MOUSE_BUTTON_X1: u8 = 4;
const MOUSE_BUTTON_X2: u8 = 5;
const DIRECTION_SECTOR_DEGREES: f64 = 45.0;
const DIRECTION_HALF_SECTOR_DEGREES: f64 = DIRECTION_SECTOR_DEGREES / 2.0;
const DIRECTION_HYSTERESIS_DEGREES: f64 = 8.0;
const DIRECTION_MIN_DISTANCE_PIXELS: i32 = 4;

static MOUSE_EVENT_SENDER: OnceLock<Mutex<Option<UnboundedSender<MouseEvent>>>> = OnceLock::new();

/// 鼠标 Hook 运行时句柄。
struct MouseHookRuntime {
    event_task: TokioJoinHandle<()>,
    #[cfg(windows)]
    hook_thread: Option<ThreadJoinHandle<()>>,
    #[cfg(windows)]
    hook_thread_id: u32,
}

/// 从平台 Hook 捕获到的鼠标事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MouseEvent {
    Move {
        x: i32,
        y: i32,
    },
    Button {
        button: u8,
        pressed: bool,
        x: i32,
        y: i32,
    },
    Wheel {
        delta: i32,
        x: i32,
        y: i32,
    },
}

/// 鼠标移动方向（按 8 方向离散化）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseDirection {
    Up,
    Down,
    Left,
    Right,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
    Idle,
}

impl MouseDirection {
    /// 由相对位移计算方向。
    ///
    /// 参数:
    /// - `dx`: X 轴相对位移
    /// - `dy`: Y 轴相对位移
    ///
    /// 返回:
    /// - 离散化后的 8 向方向或 `Idle`
    pub fn from_delta(dx: i32, dy: i32) -> Self {
        Self::from_delta_with_previous(dx, dy, None)
    }

    /// 由相对位移和上一方向计算稳定方向。
    ///
    /// 参数:
    /// - `dx`: X 轴相对位移
    /// - `dy`: Y 轴相对位移
    /// - `previous`: 上一次输出方向
    ///
    /// 返回:
    /// - 带边界滞后的 8 向方向或 `Idle`
    pub fn from_delta_with_previous(dx: i32, dy: i32, previous: Option<MouseDirection>) -> Self {
        if dx == 0 && dy == 0 {
            return Self::Idle;
        }

        let angle = (dy as f64).atan2(dx as f64).to_degrees();
        if let Some(previous) = previous.filter(|direction| *direction != Self::Idle) {
            let distance = angle_distance_degrees(angle, previous.center_angle_degrees());
            if distance <= DIRECTION_HALF_SECTOR_DEGREES + DIRECTION_HYSTERESIS_DEGREES {
                return previous;
            }
        }

        direction_from_angle_degrees(angle)
    }

    /// 返回协议层可直接使用的方向字符串。
    ///
    /// 返回:
    /// - 方向名称（snake_case）
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Left => "left",
            Self::Right => "right",
            Self::UpLeft => "up_left",
            Self::UpRight => "up_right",
            Self::DownLeft => "down_left",
            Self::DownRight => "down_right",
            Self::Idle => "idle",
        }
    }

    fn center_angle_degrees(self) -> f64 {
        match self {
            Self::Right => 0.0,
            Self::DownRight => 45.0,
            Self::Down => 90.0,
            Self::DownLeft => 135.0,
            Self::Left => 180.0,
            Self::UpLeft => -135.0,
            Self::Up => -90.0,
            Self::UpRight => -45.0,
            Self::Idle => 0.0,
        }
    }
}

fn direction_from_angle_degrees(angle: f64) -> MouseDirection {
    let index = ((angle / DIRECTION_SECTOR_DEGREES).round() as i32).rem_euclid(8);
    match index {
        0 => MouseDirection::Right,
        1 => MouseDirection::DownRight,
        2 => MouseDirection::Down,
        3 => MouseDirection::DownLeft,
        4 => MouseDirection::Left,
        5 => MouseDirection::UpLeft,
        6 => MouseDirection::Up,
        7 => MouseDirection::UpRight,
        _ => MouseDirection::Idle,
    }
}

fn angle_distance_degrees(a: f64, b: f64) -> f64 {
    let difference = (a - b).rem_euclid(360.0);
    difference.min(360.0 - difference)
}

/// 鼠标移动采样后可触发的事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseMotionEvent {
    /// 方向发生变化时触发（附带当前采样增量）
    DirectionChanged {
        dx: i32,
        dy: i32,
        direction: MouseDirection,
    },
    /// 相对上一次采样无位移时触发（仅进入静止状态时触发一次）
    Idle,
}

/// 鼠标按键状态（button code -> pressed）
pub struct MouseState {
    buttons: HashMap<u8, bool>,
    /// 最后一次用于方向判定的采样位置（像素）
    pub last_sample_x: i32,
    pub last_sample_y: i32,
    /// 最后一次观察到的原始位置（像素）
    pub last_observed_x: i32,
    pub last_observed_y: i32,
    /// 是否已经有过首个位置采样
    pub has_sample: bool,
    /// 上一次已判定并输出的方向
    pub last_judged_direction: Option<MouseDirection>,
}

impl MouseState {
    /// 创建默认鼠标状态。
    ///
    /// 返回:
    /// - 初始状态（无采样、无方向）
    pub fn new() -> Self {
        Self {
            buttons: HashMap::new(),
            last_sample_x: 0,
            last_sample_y: 0,
            last_observed_x: 0,
            last_observed_y: 0,
            has_sample: false,
            last_judged_direction: None,
        }
    }

    /// 更新某个鼠标按键状态。
    ///
    /// 参数:
    /// - `button`: 按键编码
    /// - `pressed`: 是否按下
    pub fn update_button(&mut self, button: u8, pressed: bool) {
        self.buttons.insert(button, pressed);
    }

    /// 查询某个按键是否处于按下状态。
    ///
    /// 参数:
    /// - `button`: 按键编码
    ///
    /// 返回:
    /// - `true` 表示按下
    pub fn is_pressed(&self, button: u8) -> bool {
        self.buttons.get(&button).copied().unwrap_or(false)
    }

    /// 处理一次鼠标位置采样，并根据“相对于上一次采样”规则输出事件。
    ///
    /// 规则:
    /// - 首个采样仅建立基线，不输出事件
    /// - 短距离抖动先累积，不立即判定方向
    /// - 与上次采样相比方向变化时，输出 `DirectionChanged`
    /// - 与上次采样无位移时，若刚进入静止，输出 `Idle`
    /// - 连续同方向移动或连续静止不重复输出
    ///
    /// 参数:
    /// - `x`: 当前采样 X 坐标
    /// - `y`: 当前采样 Y 坐标
    ///
    /// 返回:
    /// - 需要向外发送的事件；若无需发送则返回 `None`
    pub fn process_sample(&mut self, x: i32, y: i32) -> Option<MouseMotionEvent> {
        if !self.has_sample {
            self.last_sample_x = x;
            self.last_sample_y = y;
            self.last_observed_x = x;
            self.last_observed_y = y;
            self.has_sample = true;
            return None;
        }

        let moved_since_last_observed = x != self.last_observed_x || y != self.last_observed_y;
        self.last_observed_x = x;
        self.last_observed_y = y;

        let dx = x - self.last_sample_x;
        let dy = y - self.last_sample_y;
        if dx == 0 && dy == 0 {
            return self.enter_idle();
        }

        if movement_distance_squared(dx, dy)
            < movement_distance_squared(DIRECTION_MIN_DISTANCE_PIXELS, 0)
        {
            if moved_since_last_observed {
                return None;
            }

            self.last_sample_x = x;
            self.last_sample_y = y;
            return self.enter_idle();
        }

        let direction =
            MouseDirection::from_delta_with_previous(dx, dy, self.last_judged_direction);
        self.last_sample_x = x;
        self.last_sample_y = y;

        if self.last_judged_direction == Some(direction) {
            None
        } else {
            self.last_judged_direction = Some(direction);
            Some(MouseMotionEvent::DirectionChanged { dx, dy, direction })
        }
    }

    fn enter_idle(&mut self) -> Option<MouseMotionEvent> {
        if self.last_judged_direction == Some(MouseDirection::Idle) {
            return None;
        }

        self.last_judged_direction = Some(MouseDirection::Idle);
        Some(MouseMotionEvent::Idle)
    }

    /// 清理状态为初始值。
    pub fn clear(&mut self) {
        self.buttons.clear();
        self.last_sample_x = 0;
        self.last_sample_y = 0;
        self.last_observed_x = 0;
        self.last_observed_y = 0;
        self.has_sample = false;
        self.last_judged_direction = None;
    }
}

fn movement_distance_squared(dx: i32, dy: i32) -> i64 {
    let dx = i64::from(dx);
    let dy = i64::from(dy);
    dx * dx + dy * dy
}

/// 鼠标 Hook 实现
pub struct MouseHook {
    state: Arc<Mutex<MouseState>>,
    runtime: Option<MouseHookRuntime>,
}

impl MouseHook {
    /// 创建鼠标 Hook。
    ///
    /// 返回:
    /// - 可用于采样判定的 `MouseHook`
    pub fn new() -> Result<Self> {
        Ok(Self {
            state: Arc::new(Mutex::new(MouseState::new())),
            runtime: None,
        })
    }

    /// 启动鼠标监听并将事件广播到 WebSocket。
    ///
    /// 参数:
    /// - `websocket`: 已启动的 WebSocket 广播服务
    ///
    /// 返回:
    /// - Hook 成功启动时返回 `Ok(())`
    pub async fn start(&mut self, websocket: WebSocketServer) -> Result<()> {
        if self.runtime.is_some() {
            return Ok(());
        }

        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        replace_mouse_event_sender(Some(event_sender));

        let event_task =
            spawn_mouse_event_processor(Arc::clone(&self.state), websocket, event_receiver);

        #[cfg(windows)]
        {
            match spawn_windows_mouse_hook_thread() {
                Ok((hook_thread_id, hook_thread)) => {
                    self.runtime = Some(MouseHookRuntime {
                        event_task,
                        hook_thread: Some(hook_thread),
                        hook_thread_id,
                    });
                    info!(hook_thread_id, "mouse hook started");
                    Ok(())
                }
                Err(error) => {
                    replace_mouse_event_sender(None);
                    let _ = event_task.await;
                    Err(error)
                }
            }
        }

        #[cfg(not(windows))]
        {
            replace_mouse_event_sender(None);
            let _ = event_task.await;
            Err(Error::MouseHook(
                "mouse hook is currently implemented only on Windows".to_string(),
            ))
        }
    }

    /// 处理一次采样并按规则自动广播。
    ///
    /// 参数:
    /// - `websocket`: WebSocket 广播服务
    /// - `x`: 当前采样 X 坐标
    /// - `y`: 当前采样 Y 坐标
    ///
    /// 返回:
    /// - 成功返回 `Ok(())`
    pub async fn process_sample_and_broadcast(
        &mut self,
        websocket: &WebSocketServer,
        x: i32,
        y: i32,
    ) -> Result<()> {
        let event = match self.state.lock() {
            Ok(mut state) => state.process_sample(x, y),
            Err(error) => {
                warn!(%error, "mouse state lock poisoned");
                None
            }
        };

        if let Some(event) = event {
            websocket.broadcast_mouse_motion_event(event).await?;
        }
        Ok(())
    }

    /// 停止鼠标监听。
    ///
    /// 返回:
    /// - Hook 成功停止或尚未启动时返回 `Ok(())`
    pub async fn stop(&mut self) -> Result<()> {
        let Some(mut runtime) = self.runtime.take() else {
            return Ok(());
        };

        replace_mouse_event_sender(None);

        #[cfg(windows)]
        if let Some(hook_thread) = runtime.hook_thread.take() {
            stop_windows_mouse_hook_thread(runtime.hook_thread_id, hook_thread).await?;
        }

        let _ = runtime.event_task.await;
        info!("mouse hook stopped");
        Ok(())
    }

    /// 获取共享鼠标状态。
    ///
    /// 返回:
    /// - 可跨线程读取的鼠标状态句柄
    pub fn get_state(&self) -> Arc<Mutex<MouseState>> {
        Arc::clone(&self.state)
    }
}

/// 启动鼠标事件处理任务。
///
/// 参数:
/// - `state`: 共享鼠标状态
/// - `websocket`: WebSocket 广播服务
/// - `event_receiver`: 平台 Hook 事件接收器
///
/// 返回:
/// - Tokio 任务句柄
fn spawn_mouse_event_processor(
    state: Arc<Mutex<MouseState>>,
    websocket: WebSocketServer,
    mut event_receiver: UnboundedReceiver<MouseEvent>,
) -> TokioJoinHandle<()> {
    tokio::spawn(async move {
        let mut latest_position: Option<(i32, i32)> = None;
        let mut idle_interval = tokio::time::interval(Duration::from_millis(33));

        loop {
            tokio::select! {
                event = event_receiver.recv() => {
                    let Some(event) = event else {
                        break;
                    };

                    match event {
                        MouseEvent::Move { x, y } => {
                            latest_position = Some((x, y));
                        }
                        MouseEvent::Button { button, pressed, x, y } => {
                            latest_position = Some((x, y));
                            handle_mouse_button_event(&state, &websocket, button, pressed, x, y).await;
                        }
                        MouseEvent::Wheel { delta, x, y } => {
                            latest_position = Some((x, y));
                            if let Err(error) = websocket.broadcast_mouse_wheel(delta, x, y).await {
                                warn!(%error, delta, x, y, "failed to broadcast mouse wheel event");
                            }
                        }
                    }
                }
                _ = idle_interval.tick() => {
                    if let Some((x, y)) = latest_position {
                        handle_mouse_move_sample(&state, &websocket, x, y).await;
                    }
                }
            }
        }

        debug!("mouse event processor stopped");
    })
}

/// 处理一次鼠标移动采样并广播方向变化或静止事件。
///
/// 参数:
/// - `state`: 共享鼠标状态
/// - `websocket`: WebSocket 广播服务
/// - `x`: 当前 X 坐标
/// - `y`: 当前 Y 坐标
async fn handle_mouse_move_sample(
    state: &Arc<Mutex<MouseState>>,
    websocket: &WebSocketServer,
    x: i32,
    y: i32,
) {
    let motion_event = match state.lock() {
        Ok(mut state) => state.process_sample(x, y),
        Err(error) => {
            warn!(%error, "mouse state lock poisoned");
            None
        }
    };

    if let Some(motion_event) = motion_event {
        if let Err(error) = websocket.broadcast_mouse_motion_event(motion_event).await {
            warn!(%error, x, y, "failed to broadcast mouse motion event");
        }
    }
}

/// 处理鼠标按键状态变化并广播。
///
/// 参数:
/// - `state`: 共享鼠标状态
/// - `websocket`: WebSocket 广播服务
/// - `button`: 鼠标按键编码
/// - `pressed`: 是否按下
/// - `x`: 当前 X 坐标
/// - `y`: 当前 Y 坐标
async fn handle_mouse_button_event(
    state: &Arc<Mutex<MouseState>>,
    websocket: &WebSocketServer,
    button: u8,
    pressed: bool,
    x: i32,
    y: i32,
) {
    let should_broadcast = match state.lock() {
        Ok(mut state) => {
            if state.is_pressed(button) == pressed {
                false
            } else {
                state.update_button(button, pressed);
                true
            }
        }
        Err(error) => {
            warn!(%error, "mouse state lock poisoned");
            false
        }
    };

    if should_broadcast {
        if let Err(error) = websocket
            .broadcast_mouse_button(button, pressed, x, y)
            .await
        {
            warn!(%error, button, pressed, x, y, "failed to broadcast mouse button event");
        }
    }
}

/// 替换平台 Hook 回调使用的事件发送器。
///
/// 参数:
/// - `sender`: 新发送器；`None` 表示停止投递事件
fn replace_mouse_event_sender(sender: Option<UnboundedSender<MouseEvent>>) {
    let slot = MOUSE_EVENT_SENDER.get_or_init(|| Mutex::new(None));
    match slot.lock() {
        Ok(mut guard) => {
            *guard = sender;
        }
        Err(error) => {
            warn!(%error, "mouse sender lock poisoned");
        }
    }
}

/// 向事件处理任务投递鼠标事件。
///
/// 参数:
/// - `event`: 捕获到的鼠标事件
fn send_mouse_event(event: MouseEvent) {
    let Some(slot) = MOUSE_EVENT_SENDER.get() else {
        return;
    };

    match slot.lock() {
        Ok(guard) => {
            if let Some(sender) = guard.as_ref() {
                let _ = sender.send(event);
            }
        }
        Err(error) => {
            warn!(%error, "mouse sender lock poisoned");
        }
    }
}

/// 启动 Windows 低级鼠标 Hook 线程。
///
/// 返回:
/// - Hook 线程 ID 与线程句柄
#[cfg(windows)]
fn spawn_windows_mouse_hook_thread() -> Result<(u32, ThreadJoinHandle<()>)> {
    let (ready_sender, ready_receiver) = std::sync::mpsc::channel();
    let hook_thread = std::thread::Builder::new()
        .name("webkeylayer-mouse-hook".to_string())
        .spawn(move || {
            let setup_result = install_windows_mouse_hook();
            match setup_result {
                Ok((hook, hook_thread_id)) => {
                    let _ = ready_sender.send(Ok(hook_thread_id));
                    run_windows_mouse_message_loop();
                    unsafe {
                        let _ = UnhookWindowsHookEx(hook);
                    }
                }
                Err(error) => {
                    let _ = ready_sender.send(Err(error));
                }
            }
        })
        .map_err(|error| Error::MouseHook(format!("failed to spawn hook thread: {error}")))?;

    match ready_receiver.recv() {
        Ok(Ok(hook_thread_id)) => Ok((hook_thread_id, hook_thread)),
        Ok(Err(error)) => {
            let _ = hook_thread.join();
            Err(Error::MouseHook(error))
        }
        Err(error) => {
            let _ = hook_thread.join();
            Err(Error::MouseHook(format!(
                "failed to receive hook thread startup result: {error}"
            )))
        }
    }
}

/// 安装 Windows 低级鼠标 Hook。
///
/// 返回:
/// - Hook 句柄与 Hook 线程 ID
#[cfg(windows)]
fn install_windows_mouse_hook() -> std::result::Result<(HHOOK, u32), String> {
    unsafe {
        let hook_thread_id = GetCurrentThreadId();
        let module = GetModuleHandleW(None)
            .map_err(|error| format!("failed to get module handle: {error}"))?;
        let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(low_level_mouse_proc), module, 0)
            .map_err(|error| format!("failed to install low-level mouse hook: {error}"))?;

        let mut message = MSG::default();
        let _ = PeekMessageW(&mut message, HWND(0), 0, 0, PM_NOREMOVE);

        Ok((hook, hook_thread_id))
    }
}

/// 运行 Windows 鼠标 Hook 线程消息循环。
#[cfg(windows)]
fn run_windows_mouse_message_loop() {
    unsafe {
        let mut message = MSG::default();
        loop {
            let result = GetMessageW(&mut message, HWND(0), 0, 0).0;
            if result == -1 {
                warn!("mouse hook message loop failed");
                break;
            }
            if result == 0 {
                break;
            }
        }
    }
}

/// 停止 Windows 低级鼠标 Hook 线程。
///
/// 参数:
/// - `hook_thread_id`: Hook 线程 ID
/// - `hook_thread`: Hook 线程句柄
///
/// 返回:
/// - 线程成功退出时返回 `Ok(())`
#[cfg(windows)]
async fn stop_windows_mouse_hook_thread(
    hook_thread_id: u32,
    hook_thread: ThreadJoinHandle<()>,
) -> Result<()> {
    unsafe {
        let posted = PostThreadMessageW(hook_thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        if !posted.as_bool() {
            warn!(hook_thread_id, "failed to post mouse hook quit message");
        }
    }

    tokio::task::spawn_blocking(move || hook_thread.join())
        .await
        .map_err(|error| Error::MouseHook(format!("failed to join hook thread: {error}")))?
        .map_err(|_| Error::MouseHook("mouse hook thread panicked".to_string()))
}

/// Windows 低级鼠标 Hook 回调。
///
/// 参数:
/// - `code`: Hook 事件代码
/// - `wparam`: Windows 鼠标消息类型
/// - `lparam`: 指向 [`MSLLHOOKSTRUCT`] 的指针
///
/// 返回:
/// - 下一个 Hook 的返回值
#[cfg(windows)]
unsafe extern "system" fn low_level_mouse_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code == HC_ACTION as i32 {
        let message = wparam.0 as u32;
        let mouse = *(lparam.0 as *const MSLLHOOKSTRUCT);
        let x = mouse.pt.x;
        let y = mouse.pt.y;

        if message == WM_MOUSEMOVE {
            send_mouse_event(MouseEvent::Move { x, y });
        } else if message == WM_MOUSEWHEEL {
            send_mouse_event(MouseEvent::Wheel {
                delta: mouse_wheel_delta(mouse.mouseData),
                x,
                y,
            });
        } else if let Some((button, pressed)) = mouse_button_event(message, mouse.mouseData) {
            send_mouse_event(MouseEvent::Button {
                button,
                pressed,
                x,
                y,
            });
        }
    }

    CallNextHookEx(HHOOK(0), code, wparam, lparam)
}

/// 将 Windows 鼠标按键消息映射为协议按键编码。
///
/// 参数:
/// - `message`: Windows 鼠标消息常量
/// - `mouse_data`: 鼠标事件附加数据
///
/// 返回:
/// - 鼠标按键编码与按下状态；无法识别时返回 `None`
#[cfg(windows)]
fn mouse_button_event(message: u32, mouse_data: u32) -> Option<(u8, bool)> {
    match message {
        WM_LBUTTONDOWN => Some((MOUSE_BUTTON_LEFT, true)),
        WM_LBUTTONUP => Some((MOUSE_BUTTON_LEFT, false)),
        WM_RBUTTONDOWN => Some((MOUSE_BUTTON_RIGHT, true)),
        WM_RBUTTONUP => Some((MOUSE_BUTTON_RIGHT, false)),
        WM_MBUTTONDOWN => Some((MOUSE_BUTTON_MIDDLE, true)),
        WM_MBUTTONUP => Some((MOUSE_BUTTON_MIDDLE, false)),
        WM_XBUTTONDOWN => x_button_code(mouse_data).map(|button| (button, true)),
        WM_XBUTTONUP => x_button_code(mouse_data).map(|button| (button, false)),
        _ => None,
    }
}

/// 从 Windows XButton 附加数据中解析协议按键编码。
///
/// 参数:
/// - `mouse_data`: 鼠标事件附加数据
///
/// 返回:
/// - 协议按键编码；无法识别时返回 `None`
#[cfg(windows)]
fn x_button_code(mouse_data: u32) -> Option<u8> {
    match high_word(mouse_data) {
        XBUTTON1 => Some(MOUSE_BUTTON_X1),
        XBUTTON2 => Some(MOUSE_BUTTON_X2),
        _ => None,
    }
}

/// 从 Windows 滚轮附加数据中解析滚动增量。
///
/// 参数:
/// - `mouse_data`: 鼠标事件附加数据
///
/// 返回:
/// - 有符号滚轮增量，通常为 `120` 或 `-120` 的倍数
#[cfg(windows)]
fn mouse_wheel_delta(mouse_data: u32) -> i32 {
    high_word(mouse_data) as i16 as i32
}

/// 读取 32 位值的高 16 位。
///
/// 参数:
/// - `value`: 待读取的 32 位值
///
/// 返回:
/// - 高 16 位
#[cfg(windows)]
fn high_word(value: u32) -> u16 {
    ((value >> 16) & 0xffff) as u16
}

impl Default for MouseHook {
    fn default() -> Self {
        Self::new().expect("Failed to create MouseHook")
    }
}

#[cfg(test)]
mod tests {
    use super::{MouseDirection, MouseMotionEvent, MouseState};

    #[test]
    fn maps_delta_to_eight_way_direction() {
        assert_eq!(MouseDirection::from_delta(0, -4), MouseDirection::Up);
        assert_eq!(MouseDirection::from_delta(0, 4), MouseDirection::Down);
        assert_eq!(MouseDirection::from_delta(-3, 0), MouseDirection::Left);
        assert_eq!(MouseDirection::from_delta(3, 0), MouseDirection::Right);
        assert_eq!(MouseDirection::from_delta(10, 1), MouseDirection::Right);
        assert_eq!(MouseDirection::from_delta(1, -10), MouseDirection::Up);
        assert_eq!(MouseDirection::from_delta(-2, -1), MouseDirection::UpLeft);
        assert_eq!(MouseDirection::from_delta(2, -1), MouseDirection::UpRight);
        assert_eq!(MouseDirection::from_delta(-2, 1), MouseDirection::DownLeft);
        assert_eq!(MouseDirection::from_delta(2, 1), MouseDirection::DownRight);
        assert_eq!(MouseDirection::from_delta(0, 0), MouseDirection::Idle);
    }

    #[test]
    fn emits_motion_only_when_direction_changes_or_enters_idle() {
        let mut state = MouseState::new();

        assert_eq!(state.process_sample(100, 100), None);
        assert_eq!(
            state.process_sample(110, 100),
            Some(MouseMotionEvent::DirectionChanged {
                dx: 10,
                dy: 0,
                direction: MouseDirection::Right,
            })
        );
        assert_eq!(state.process_sample(120, 100), None);
        assert_eq!(state.process_sample(120, 100), Some(MouseMotionEvent::Idle));
        assert_eq!(state.process_sample(120, 100), None);
        assert_eq!(
            state.process_sample(116, 96),
            Some(MouseMotionEvent::DirectionChanged {
                dx: -4,
                dy: -4,
                direction: MouseDirection::UpLeft,
            })
        );
    }

    #[test]
    fn keeps_previous_direction_inside_boundary_hysteresis() {
        let mut state = MouseState::new();

        assert_eq!(state.process_sample(0, 0), None);
        assert_eq!(
            state.process_sample(10, 4),
            Some(MouseMotionEvent::DirectionChanged {
                dx: 10,
                dy: 4,
                direction: MouseDirection::Right,
            })
        );
        assert_eq!(state.process_sample(20, 9), None);
        assert_eq!(
            state.process_sample(30, 19),
            Some(MouseMotionEvent::DirectionChanged {
                dx: 10,
                dy: 10,
                direction: MouseDirection::DownRight,
            })
        );
    }

    #[test]
    fn accumulates_short_jitter_before_judging_direction() {
        let mut state = MouseState::new();

        assert_eq!(state.process_sample(0, 0), None);
        assert_eq!(state.process_sample(1, 1), None);
        assert_eq!(
            state.process_sample(5, 1),
            Some(MouseMotionEvent::DirectionChanged {
                dx: 5,
                dy: 1,
                direction: MouseDirection::Right,
            })
        );
    }
}
