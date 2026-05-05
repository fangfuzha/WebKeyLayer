# WebKeyLayer 后端开发指南

## 项目结构

```
backend/
├── Cargo.toml                     # 依赖配置
├── build.rs                       # 编译脚本（待实现）
├── src/
│   ├── main.rs                    # 应用入口
│   ├── lib.rs                     # 库导出
│   ├── error.rs                   # 错误类型定义
│   ├── log.rs                     # 日志管理
│   ├── util.rs                    # 工具函数
│   ├── keyboard_hook.rs           # 键盘监听 (输入采集层)
│   ├── websocket_server.rs        # WebSocket 服务 (事件分发层)
│   ├── config/                    # 配置管理模块 (配置管理层)
│   │   ├── mod.rs
│   │   ├── schema.rs              # 配置结构定义
│   │   └── loader.rs              # TOML 加载器
│   ├── preset/                    # 预设兼容层 (⭐ 核心差异化)
│   │   ├── mod.rs
│   │   ├── importer.rs            # Input Overlay 导入
│   │   ├── schema.rs              # 内部预设格式
│   │   └── renderer.rs            # 预设渲染
│   ├── ui/                        # UI 层 (本地宿主 + 管理 API)
│   │   ├── mod.rs
│   │   ├── tray.rs                # 系统托盘
│   │   ├── webview.rs             # WebView 宿主
│   │   └── server.rs              # 管理 API 服务
│   └── state/                     # 状态管理层
│       ├── mod.rs
│       ├── keyboard_state.rs      # 按键状态
│       ├── config_state.rs        # 配置状态
│       └── sync.rs                # 跨线程同步
└── README.md (当前文件)
```

## 核心模块

### 1. keyboard_hook.rs (输入采集层)

**职责**：全局键盘监听，Windows 系统级事件捕获

**实现细节**：

- 使用 Windows API SetWindowsHookExW + WH_KEYBOARD_LL
- 线程安全：Arc<Mutex<KeyboardState>>
- 事件流向：Hook 回调 → Tokio channel → KeyboardState → WebSocket 广播
- 自动过滤重复按下/松开状态，避免按键长按重复广播

**核心接口**:

```rust
pub struct KeyboardHook {
    // Windows Hook 线程与 Tokio 事件处理任务
}

impl KeyboardHook {
    pub async fn start(&mut self, websocket: WebSocketServer) -> Result<()> { }
    pub async fn stop(&mut self) -> Result<()> { }
}
```

### 2. websocket_server.rs (事件分发层)

**职责**：WebSocket 服务，事件广播到多个连接客户端

**实现细节**：

- Tokio 异步 WebSocket 服务 (0.0.0.0:8080)
- 连接管理与消息广播
- 心跳检测与重连机制
- 目标延迟：< 20ms (消息往返)

**消息格式**：参考 [docs/API_PROTOCOL.md](../docs/API_PROTOCOL.md)

### 3. mouse_hook.rs (输入采集层)

**职责**：全局鼠标监听，Windows 系统级事件捕获

**实现细节**：

- 使用 Windows API SetWindowsHookExW + WH_MOUSE_LL
- 线程安全：Arc<Mutex<MouseState>>
- 事件流向：Hook 回调 → Tokio channel → MouseState → WebSocket 广播
- 鼠标移动只在方向变化时广播 `mouse_move_direction_changed`
- 鼠标停止后由 33ms 定时复采样触发一次 `mouse_idle`
- 鼠标按键状态变化广播 `mouse_button_pressed` / `mouse_button_released`
- 鼠标滚轮广播 `mouse_wheel`

**核心接口**:

```rust
pub struct MouseHook {
    // Windows Hook 线程与 Tokio 事件处理任务
}

impl MouseHook {
    pub async fn start(&mut self, websocket: WebSocketServer) -> Result<()> { }
    pub async fn stop(&mut self) -> Result<()> { }
}
```

### 4. config/ (配置管理层)

**职责**：TOML 配置加载、验证、热重载

**配置文件位置**：`%APPDATA%/WebKeyLayer/config.toml`

**功能**：

- 配置版本号管理
- 文件变化监听 (notify crate)
- 热重载（修改配置即时生效，无需重启）

### 5. preset/ (预设兼容层) ⭐

**职责**：Input Overlay 预设格式解析与内部格式转换

**支持的元素类型**（第一阶段）：

- `ET_KEYBOARD_KEY` (1)
- `ET_MOUSE_BUTTON` (2)
- `ET_TEXTURE` (3)

**字段映射**：

| Input Overlay     | WebKeyLayer  | 含义     |
| ----------------- | ------------ | -------- |
| type              | element_type | 元素类型 |
| code              | keycode      | 按键码   |
| pos [x,y]         | position     | 页面坐标 |
| mapping [x,y,w,h] | texture      | 贴图切片 |

**兼容模式**：

- Strict: 拒绝不兼容的预设
- Lenient: 跳过不兼容项，继续加载

### 6. ui/ (UI 层)

**职责**：系统托盘宿主、本地管理网页、管理 API 服务

#### ui/tray.rs - 系统托盘

```
右键菜单项：
- 打开管理面板 (双击也可)
- 启动/停止服务
- 显示/复制连接地址
- 查看日志
- 退出程序
```

#### ui/webview.rs - WebView 宿主

- Windows: WebView2 (系统自带)
- Linux: wry (预留)
- 加载地址：`http://127.0.0.1:8888/admin`

#### ui/server.rs - 管理 API

参考 [docs/API_PROTOCOL.md](../docs/API_PROTOCOL.md#后端管理页面-api) 的 11 个 API 端点

### 6. state/ (状态管理层)

**职责**：跨线程按键/配置状态同步

**全局状态**：

```rust
pub struct AppState {
    pub keyboard: Arc<RwLock<KeyboardState>>,
    pub config: Arc<RwLock<ConfigState>>,
}
```

## 开发步骤

### Step 1: 环境设置

```bash
cd backend
cargo build
cargo run     # 测试启动
```

### Step 2: 实现 KeyboardHook

1. 研究 Windows API (SetWindowsHookEx)
2. 实现回调函数处理按键事件
3. 更新 KeyboardState
4. 测试本地键盘事件捕获

### Step 3: 实现 WebSocketServer

1. 使用 tokio-tungstenite 创建监听器
2. 为每个连接创建处理任务
3. 实现消息序列化与广播
4. 测试多客户端连接

### Step 4: 实现 ConfigManager

1. 实现 TOML 解析与验证
2. 使用 notify crate 实现文件监听
3. 实现配置热重载
4. 测试配置更新

### Step 5: 实现 PresetImporter

1. 解析 Input Overlay JSON 格式
2. 实现字段映射与验证
3. 支持严格/宽松兼容模式
4. 测试预设导入

### Step 6: 实现 UI 层

1. 使用 winit + tray-icon 创建托盘
2. 使用 WebView2 加载管理页面
3. 实现管理 API 端点
4. 测试本地管理页面访问

## 编译与运行

```bash
# 开发编译
cargo build

# 发布编译 (优化)
cargo build --release

# 代码检查
cargo check

# 运行测试
cargo test

# 查看编译详情
cargo build -vv

# 清理编译产物
cargo clean
```

## 调试

### 启用日志输出

```bash
RUST_LOG=debug cargo run
```

### 日志模块

- `keyboard_hook`: 键盘事件日志
- `websocket_server`: WebSocket 连接日志
- `config`: 配置管理日志
- `preset`: 预设导入日志
- `ui`: UI 事件日志

## 性能优化注意事项

1. **按键延迟 < 30ms**
   - 使用 Tokio 异步避免阻塞
   - 键盘事件应立即序列化并发送
   - 避免在消息处理中进行重型计算

2. **支持 8+ 并发客户端**
   - 使用高效的数据结构 (Vec vs HashMap)
   - 避免全局锁竞争
   - 使用 Arc<RwLock<>> 进行读写分离

3. **消息大小 < 200 字节**
   - 按键事件只包含必需字段
   - 避免序列化大型对象

## 依赖说明

| 依赖              | 版本 | 用途              |
| ----------------- | ---- | ----------------- |
| tokio             | 1.35 | 异步运行时        |
| tokio-tungstenite | 0.21 | WebSocket         |
| serde             | 1.0  | 序列化            |
| toml              | 0.8  | 配置解析          |
| notify            | 6    | 文件监听          |
| winit             | 0.29 | 窗口系统          |
| tray-icon         | 0.1  | 系统托盘          |
| webview2          | 0.1  | WebView (Windows) |
| wry               | 0.24 | WebView (Linux)   |
| tracing           | 0.1  | 日志              |
| thiserror         | 1.0  | 错误处理          |
| anyhow            | 1.0  | 错误传播          |

## 常见问题

### Q: 如何测试 WebSocket 连接？

A: 使用 WebSocket 测试工具（如 wscat 或浏览器开发者工具）连接 `ws://127.0.0.1:8080/stream`

### Q: 配置文件在哪里？

A: `%APPDATA%/WebKeyLayer/config.toml`（Windows 用户数据目录）

### Q: 如何调整监听端口？

A: 修改 `config.toml` 中 `[network]` 的 `port` 字段，重新加载即可

### Q: 支持 Linux 吗？

A: 当前优先支持 Windows。Linux 支持已预留（需实现 Linux Hook 层）。

---

**文档更新**: 2026-05-04  
**当前状态**: 框架完成，等待实现
