# WebKeyLayer 项目初始化完成

## 📋 项目概况

WebKeyLayer 是一款**跨平台、局域网通用、支持双机推流的键盘按键可视化浮层工具**。

- **技术栈**：Rust (Tokio) + WebSocket + 原生 WebView
- **架构**：C/S 模型，后端托盘程序 + 前端网页 + 预设兼容性层
- **目标**：让游戏主机上按下的键盘映射实时显示在推流机的直播画面上

---

## 📁 项目结构

```
WebKeyLayer/
├── backend/                    # Rust 后端 - 核心业务逻辑
│   ├── Cargo.toml             # 依赖配置
│   ├── src/
│   │   ├── main.rs            # 应用入口
│   │   ├── lib.rs             # 库导出
│   │   ├── keyboard_hook.rs   # 全局键盘监听
│   │   ├── websocket_server.rs# WebSocket 事件分发
│   │   ├── config/            # 配置管理模块
│   │   ├── preset/            # Input Overlay 预设兼容层
│   │   ├── ui/                # 托盘与管理网页宿主
│   │   ├── state/             # 状态管理层
│   │   └── ...                # 其他模块
│   └── README.md
│
├── docs/                      # 项目文档
│   ├── 软件需求.md           # 功能与非功能需求规范 ✅
│   ├── 项目架构设计.md        # 完整架构与模块设计 ✅
│   ├── API_PROTOCOL.md       # WebSocket 协议规范 ✅
│   ├── IMPLEMENTATION.md      # 实现指南（待写）
│   └── BUILD.md               # 编译与部署说明（待写）
│
├── frontend/                  # Web 前端（待创建）
│   ├── public/                # 推流显示页面
│   └── admin/                 # 后端管理页面
│
├── presets/                   # 预设包（待创建）
│   └── input-overlay/         # Input Overlay 兼容预设
│
├── refs/                      # 参考资源
│   └── input-overlay/         # Input Overlay 源代码库
│
└── README.md                  # 项目总览（当前文件）
```

---

## ✅ 已完成工作

### 1. Cargo 项目初始化

- ✅ `backend/Cargo.toml` - 完整依赖配置
  - Tokio (异步运行时)
  - tokio-tungstenite (WebSocket)
  - serde/toml (配置管理)
  - winit + tray-icon (系统托盘)
  - tracing (日志系统)
  - 平台特定：webview2 (Windows), wry (Linux)

- ✅ 全部 22 个源文件编译成功
  - 通过 `cargo check` 验证 ✓
  - 仅有预期的 dead_code 警告（框架代码）

### 2. Rust 模块框架

已创建以下模块的完整框架（包含 Doc 注释）：

| 模块       | 文件                  | 职责                         |
| ---------- | --------------------- | ---------------------------- |
| 输入采集层 | `keyboard_hook.rs`    | Windows 全局键盘监听         |
| 事件分发层 | `websocket_server.rs` | WebSocket 服务与事件广播     |
| 配置管理层 | `config/`             | TOML 配置加载与热重载        |
| 预设兼容层 | `preset/`             | Input Overlay 格式解析与转换 |
| UI 层      | `ui/`                 | 系统托盘 + 管理网页 API      |
| 状态管理   | `state/`              | 跨线程键盘/配置状态同步      |
| 错误处理   | `error.rs`            | 统一错误类型定义             |
| 日志系统   | `log.rs`              | 日志记录与诊断               |
| 工具函数   | `util.rs`             | IP 获取等辅助函数            |

### 3. 完整的文档规范

#### [docs/软件需求.md](docs/软件需求.md) ✅

- 项目概述与双机推流方案
- 8 大功能需求模块
- Input Overlay 模块映射
- 素材格式兼容规范
- 第三方预设版权管理

#### [docs/项目架构设计.md](docs/项目架构设计.md) ✅

- 40+ 个文件/目录完整规划
- 6 大核心模块职责分解
- 完整的运行流程图（4 阶段）
- WebSocket 消息设计初稿
- 管理页面 API 端点 (11 个)
- 开发阶段与任务分解

#### [docs/API_PROTOCOL.md](docs/API_PROTOCOL.md) ✅ **新增**

- **混合通信架构说明**：WebSocket (推流) + HTTP REST (管理)
- **WebSocket 协议完整规范**
  - 连接生命周期详述
  - 6 种消息类型定义（连接、按键、配置、心跳等）
  - 消息格式 JSON 示例
- **管理 HTTP API 完整定义**
  - 11 个 REST 端点详解
  - 请求/响应格式示例
  - 错误码定义
- **性能指标与目标**
  - 延迟目标: < 30ms 端到端
  - 吞吐量: 8+ 并发客户端，1000 事件/秒
  - 带宽估算: 0.5-50 KB/秒
- **客户端实现参考** (JavaScript WebSocket 示例)
- **协议版本管理** (v1.0 当前功能，v1.1+ 规划)

---

## 🚀 快速开始

### 验证项目编译

```bash
cd backend
cargo check    # ✅ 已通过
cargo build    # 完整编译
cargo run      # 启动应用
```

### 项目结构验证

```bash
# 查看所有源文件
Get-ChildItem -Recurse -Filter *.rs

# 查看文档文件
ls docs/*.md
```

---

## 📅 下一步工作（优先级）

### Phase 1: 核心框架实现 (1-2 周)

- [ ] 实现 Windows 全局键盘 Hook
- [ ] 实现 WebSocket 服务与事件广播
- [ ] 实现配置 TOML 加载与热重载
- [ ] 实现系统托盘与本地 WebView 宿主
- [ ] 实现基础管理 API 服务

### Phase 2: 预设兼容层 (1-2 周)

- [ ] Input Overlay JSON 预设解析
- [ ] 字段映射与格式转换
- [ ] 严格/宽松兼容模式
- [ ] 预设导入/导出功能

### Phase 3: 前端实现 (2-3 周)

- [ ] 推流显示页面 (`/public/index.html`)
- [ ] 后端管理页面 (`/admin/`)
- [ ] 主题与样式系统
- [ ] 国际化 (中英文)

### Phase 4: 测试与优化 (1 周)

- [ ] 性能测试（延迟、吞吐量）
- [ ] 兼容性测试（预设格式）
- [ ] 压力测试（多客户端）

---

## 📖 文档说明

| 文档                                    | 用途                      | 读者                   |
| --------------------------------------- | ------------------------- | ---------------------- |
| [软件需求.md](docs/软件需求.md)         | 功能规范、需求分析        | 产品、设计、测试       |
| [项目架构设计.md](docs/项目架构设计.md) | 模块设计、代码结构        | 架构师、开发者         |
| [API_PROTOCOL.md](docs/API_PROTOCOL.md) | WebSocket + HTTP API 规范 | 前后端开发者           |
| IMPLEMENTATION.md                       | 技术决策、实现指南        | 开发者（待写）         |
| BUILD.md                                | 编译、部署、环境配置      | DevOps、开发者（待写） |

---

## 🔧 技术栈详情

### 后端 (Rust)

- **Tokio 1.35**: 异步运行时
- **tokio-tungstenite 0.21**: WebSocket
- **Serde 1.0**: 序列化/反序列化
- **toml 0.8**: TOML 配置
- **winit 0.29 + tray-icon 0.1**: 系统托盘
- **WebView2 0.1 (Win32)**: Windows 本地网页宿主
- **tracing 0.1**: 日志系统

### 前端 (纯 Web)

- HTML/CSS/JavaScript (无框架依赖)
- WebSocket 客户端库
- 响应式布局
- 中英文国际化

### 系统支持

- **当前**: Windows 10/11 (优先)
- **预留**: Linux (wry 预留集成)

---

## 📝 许可证

项目采用自主选择的许可证，第三方预设保留原作者的版权声明与许可。

详见 [CREDITS.md](CREDITS.md) (待创建) 和各预设目录下的 CREDITS.md。

---

## 🤝 贡献

项目当前处于初期开发阶段。欢迎反馈、建议和贡献。

---

**最后更新**: 2026-05-04  
**当前版本**: v0.1.0 (框架完成)
