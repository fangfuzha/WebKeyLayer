//! 键盘监听模块
//!
//! 实现 Windows 全局键盘监听，不受窗口焦点影响。
//!
//! # 设计约束
//! - 系统级 Hook（RegisterHotKey 或 SetWindowsHookEx）
//! - 线程安全状态共享（Arc<Mutex<>>）
//! - Linux 预留支持空间

use crate::state::KeyboardState;
use crate::websocket_server::WebSocketServer;
use crate::{Error, Result};
use std::sync::{Arc, Mutex, OnceLock};
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
    UnhookWindowsHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSG, PM_NOREMOVE, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

static KEYBOARD_EVENT_SENDER: OnceLock<Mutex<Option<UnboundedSender<KeyboardEvent>>>> =
    OnceLock::new();

/// 键盘事件处理运行时句柄。
struct KeyboardHookRuntime {
    event_task: TokioJoinHandle<()>,
    #[cfg(windows)]
    hook_thread: Option<ThreadJoinHandle<()>>,
    #[cfg(windows)]
    hook_thread_id: u32,
}

/// 从平台 Hook 捕获到的按键事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KeyboardEvent {
    keycode: u16,
    pressed: bool,
}

/// 全局键盘 Hook 实现
pub struct KeyboardHook {
    state: Arc<Mutex<KeyboardState>>,
    runtime: Option<KeyboardHookRuntime>,
}

impl KeyboardHook {
    /// 创建键盘 Hook。
    ///
    /// 返回:
    /// - 可用于平台监听的 [`KeyboardHook`]
    pub fn new() -> Result<Self> {
        Ok(Self {
            state: Arc::new(Mutex::new(KeyboardState::new())),
            runtime: None,
        })
    }

    /// 启动键盘监听并将状态变化广播到 WebSocket。
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
        replace_keyboard_event_sender(Some(event_sender));

        let event_task =
            spawn_keyboard_event_processor(Arc::clone(&self.state), websocket, event_receiver);

        #[cfg(windows)]
        {
            match spawn_windows_keyboard_hook_thread() {
                Ok((hook_thread_id, hook_thread)) => {
                    self.runtime = Some(KeyboardHookRuntime {
                        event_task,
                        hook_thread: Some(hook_thread),
                        hook_thread_id,
                    });
                    info!(hook_thread_id, "keyboard hook started");
                    Ok(())
                }
                Err(error) => {
                    replace_keyboard_event_sender(None);
                    let _ = event_task.await;
                    Err(error)
                }
            }
        }

        #[cfg(not(windows))]
        {
            replace_keyboard_event_sender(None);
            let _ = event_task.await;
            Err(Error::KeyboardHook(
                "keyboard hook is currently implemented only on Windows".to_string(),
            ))
        }
    }

    /// 停止键盘监听。
    ///
    /// 返回:
    /// - Hook 成功停止或尚未启动时返回 `Ok(())`
    pub async fn stop(&mut self) -> Result<()> {
        let Some(mut runtime) = self.runtime.take() else {
            return Ok(());
        };

        replace_keyboard_event_sender(None);

        #[cfg(windows)]
        if let Some(hook_thread) = runtime.hook_thread.take() {
            stop_windows_keyboard_hook_thread(runtime.hook_thread_id, hook_thread).await?;
        }

        let _ = runtime.event_task.await;
        info!("keyboard hook stopped");
        Ok(())
    }

    /// 获取共享键盘状态。
    ///
    /// 返回:
    /// - 可跨线程读取的键盘状态句柄
    pub fn get_state(&self) -> Arc<Mutex<KeyboardState>> {
        Arc::clone(&self.state)
    }

    /// 返回当前所有处于按下状态的按键码。
    ///
    /// 返回:
    /// - 已按下按键码列表；状态锁异常时返回空列表
    pub fn pressed_keys(&self) -> Vec<u16> {
        self.state
            .lock()
            .map(|state| state.pressed_keys())
            .unwrap_or_default()
    }
}

/// 启动键盘事件处理任务。
///
/// 参数:
/// - `state`: 共享键盘状态
/// - `websocket`: WebSocket 广播服务
/// - `event_receiver`: 平台 Hook 事件接收器
///
/// 返回:
/// - Tokio 任务句柄
fn spawn_keyboard_event_processor(
    state: Arc<Mutex<KeyboardState>>,
    websocket: WebSocketServer,
    mut event_receiver: UnboundedReceiver<KeyboardEvent>,
) -> TokioJoinHandle<()> {
    tokio::spawn(async move {
        while let Some(event) = event_receiver.recv().await {
            let should_broadcast = match state.lock() {
                Ok(mut state) => {
                    if state.is_pressed(event.keycode) == event.pressed {
                        false
                    } else {
                        state.update(event.keycode, event.pressed);
                        true
                    }
                }
                Err(error) => {
                    warn!(%error, "keyboard state lock poisoned");
                    false
                }
            };

            if should_broadcast {
                if let Err(error) = websocket
                    .broadcast_key_event(event.keycode, event.pressed)
                    .await
                {
                    warn!(%error, keycode = event.keycode, pressed = event.pressed, "failed to broadcast keyboard event");
                }
            }
        }

        debug!("keyboard event processor stopped");
    })
}

/// 替换平台 Hook 回调使用的事件发送器。
///
/// 参数:
/// - `sender`: 新发送器；`None` 表示停止投递事件
fn replace_keyboard_event_sender(sender: Option<UnboundedSender<KeyboardEvent>>) {
    let slot = KEYBOARD_EVENT_SENDER.get_or_init(|| Mutex::new(None));
    match slot.lock() {
        Ok(mut guard) => {
            *guard = sender;
        }
        Err(error) => {
            warn!(%error, "keyboard sender lock poisoned");
        }
    }
}

/// 向事件处理任务投递键盘事件。
///
/// 参数:
/// - `event`: 捕获到的键盘事件
fn send_keyboard_event(event: KeyboardEvent) {
    let Some(slot) = KEYBOARD_EVENT_SENDER.get() else {
        return;
    };

    match slot.lock() {
        Ok(guard) => {
            if let Some(sender) = guard.as_ref() {
                let _ = sender.send(event);
            }
        }
        Err(error) => {
            warn!(%error, "keyboard sender lock poisoned");
        }
    }
}

/// 启动 Windows 低级键盘 Hook 线程。
///
/// 返回:
/// - Hook 线程 ID 与线程句柄
#[cfg(windows)]
fn spawn_windows_keyboard_hook_thread() -> Result<(u32, ThreadJoinHandle<()>)> {
    let (ready_sender, ready_receiver) = std::sync::mpsc::channel();
    let hook_thread = std::thread::Builder::new()
        .name("webkeylayer-keyboard-hook".to_string())
        .spawn(move || {
            let setup_result = install_windows_keyboard_hook();
            match setup_result {
                Ok((hook, hook_thread_id)) => {
                    let _ = ready_sender.send(Ok(hook_thread_id));
                    run_windows_keyboard_message_loop();
                    unsafe {
                        let _ = UnhookWindowsHookEx(hook);
                    }
                }
                Err(error) => {
                    let _ = ready_sender.send(Err(error));
                }
            }
        })
        .map_err(|error| Error::KeyboardHook(format!("failed to spawn hook thread: {error}")))?;

    match ready_receiver.recv() {
        Ok(Ok(hook_thread_id)) => Ok((hook_thread_id, hook_thread)),
        Ok(Err(error)) => {
            let _ = hook_thread.join();
            Err(Error::KeyboardHook(error))
        }
        Err(error) => {
            let _ = hook_thread.join();
            Err(Error::KeyboardHook(format!(
                "failed to receive hook thread startup result: {error}"
            )))
        }
    }
}

/// 安装 Windows 低级键盘 Hook。
///
/// 返回:
/// - Hook 句柄与 Hook 线程 ID
#[cfg(windows)]
fn install_windows_keyboard_hook() -> std::result::Result<(HHOOK, u32), String> {
    unsafe {
        let hook_thread_id = GetCurrentThreadId();
        let module = GetModuleHandleW(None)
            .map_err(|error| format!("failed to get module handle: {error}"))?;
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), module, 0)
            .map_err(|error| format!("failed to install low-level keyboard hook: {error}"))?;

        let mut message = MSG::default();
        let _ = PeekMessageW(&mut message, HWND(0), 0, 0, PM_NOREMOVE);

        Ok((hook, hook_thread_id))
    }
}

/// 运行 Windows Hook 线程消息循环。
#[cfg(windows)]
fn run_windows_keyboard_message_loop() {
    unsafe {
        let mut message = MSG::default();
        loop {
            let result = GetMessageW(&mut message, HWND(0), 0, 0).0;
            if result == -1 {
                warn!("keyboard hook message loop failed");
                break;
            }
            if result == 0 {
                break;
            }
        }
    }
}

/// 停止 Windows 低级键盘 Hook 线程。
///
/// 参数:
/// - `hook_thread_id`: Hook 线程 ID
/// - `hook_thread`: Hook 线程句柄
///
/// 返回:
/// - 线程成功退出时返回 `Ok(())`
#[cfg(windows)]
async fn stop_windows_keyboard_hook_thread(
    hook_thread_id: u32,
    hook_thread: ThreadJoinHandle<()>,
) -> Result<()> {
    unsafe {
        let posted = PostThreadMessageW(hook_thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        if !posted.as_bool() {
            warn!(hook_thread_id, "failed to post keyboard hook quit message");
        }
    }

    tokio::task::spawn_blocking(move || hook_thread.join())
        .await
        .map_err(|error| Error::KeyboardHook(format!("failed to join hook thread: {error}")))?
        .map_err(|_| Error::KeyboardHook("keyboard hook thread panicked".to_string()))
}

/// Windows 低级键盘 Hook 回调。
///
/// 参数:
/// - `code`: Hook 事件代码
/// - `wparam`: Windows 键盘消息类型
/// - `lparam`: 指向 [`KBDLLHOOKSTRUCT`] 的指针
///
/// 返回:
/// - 下一个 Hook 的返回值
#[cfg(windows)]
unsafe extern "system" fn low_level_keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code == HC_ACTION as i32 {
        if let Some(pressed) = keyboard_message_pressed_state(wparam.0 as u32) {
            let keyboard = *(lparam.0 as *const KBDLLHOOKSTRUCT);
            let keycode = keyboard.vkCode as u16;
            send_keyboard_event(KeyboardEvent { keycode, pressed });
        }
    }

    CallNextHookEx(HHOOK(0), code, wparam, lparam)
}

/// 将 Windows 键盘消息映射为按下/松开状态。
///
/// 参数:
/// - `message`: Windows 键盘消息常量
///
/// 返回:
/// - `Some(true)` 表示按下，`Some(false)` 表示松开，`None` 表示忽略
#[cfg(windows)]
fn keyboard_message_pressed_state(message: u32) -> Option<bool> {
    match message {
        WM_KEYDOWN | WM_SYSKEYDOWN => Some(true),
        WM_KEYUP | WM_SYSKEYUP => Some(false),
        _ => None,
    }
}

impl Default for KeyboardHook {
    fn default() -> Self {
        Self::new().expect("Failed to create KeyboardHook")
    }
}
