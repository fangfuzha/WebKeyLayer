//! 鼠标监听模块
//!
//! 实现鼠标事件监听（按键、移动、滚轮），用于在 v1.0 支持鼠标事件。
//!
//! 设计约束：
//! - 使用平台 API 获取鼠标按键/移动/滚轮事件
//! - 线程安全状态共享（Arc<RwLock<>>）
//!
use crate::Result;
use crate::websocket_server::WebSocketServer;
use std::collections::HashMap;

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
        let sx = dx.signum();
        let sy = dy.signum();
        match (sx, sy) {
            (0, 0) => Self::Idle,
            (0, -1) => Self::Up,
            (0, 1) => Self::Down,
            (-1, 0) => Self::Left,
            (1, 0) => Self::Right,
            (-1, -1) => Self::UpLeft,
            (1, -1) => Self::UpRight,
            (-1, 1) => Self::DownLeft,
            (1, 1) => Self::DownRight,
            _ => Self::Idle,
        }
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
    /// 最后一次采样到的位置（像素）
    pub last_sample_x: i32,
    pub last_sample_y: i32,
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
            self.has_sample = true;
            return None;
        }

        let dx = x - self.last_sample_x;
        let dy = y - self.last_sample_y;
        self.last_sample_x = x;
        self.last_sample_y = y;

        let direction = MouseDirection::from_delta(dx, dy);
        if direction == MouseDirection::Idle {
            if self.last_judged_direction == Some(MouseDirection::Idle) {
                return None;
            }
            self.last_judged_direction = Some(MouseDirection::Idle);
            return Some(MouseMotionEvent::Idle);
        }

        if self.last_judged_direction == Some(direction) {
            None
        } else {
            self.last_judged_direction = Some(direction);
            Some(MouseMotionEvent::DirectionChanged { dx, dy, direction })
        }
    }

    /// 清理状态为初始值。
    pub fn clear(&mut self) {
        self.buttons.clear();
        self.last_sample_x = 0;
        self.last_sample_y = 0;
        self.has_sample = false;
        self.last_judged_direction = None;
    }
}

/// 鼠标 Hook 骨架
pub struct MouseHook {
    state: MouseState,
}

impl MouseHook {
    /// 创建鼠标 Hook。
    ///
    /// 返回:
    /// - 可用于采样判定的 `MouseHook`
    pub fn new() -> Result<Self> {
        Ok(Self {
            state: MouseState::new(),
        })
    }

    /// 启动鼠标监听（平台实现 TODO）
    pub async fn start(&mut self) -> Result<()> {
        // TODO: 平台特定的鼠标事件订阅
        // 推荐实现：在平台回调中调用 `process_sample` / `update_button`，
        // 并将 `DirectionChanged` 与 `Idle` 事件转发到 WebSocket 层。
        Ok(())
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
        if let Some(event) = self.state.process_sample(x, y) {
            websocket.broadcast_mouse_motion_event(event).await?;
        }
        Ok(())
    }

    /// 停止鼠标监听
    pub async fn stop(&mut self) -> Result<()> {
        // TODO: 清理资源
        Ok(())
    }

    /// 获取只读状态。
    ///
    /// 返回:
    /// - 当前鼠标状态引用
    pub fn get_state(&self) -> &MouseState {
        &self.state
    }

    /// 获取可变状态。
    ///
    /// 返回:
    /// - 当前鼠标状态可变引用
    pub fn get_state_mut(&mut self) -> &mut MouseState {
        &mut self.state
    }
}

impl Default for MouseHook {
    fn default() -> Self {
        Self::new().expect("Failed to create MouseHook")
    }
}
