# WebKeyLayer TODO

最后更新：2026-05-05

## 当前目标

把后端核心输入链路做成端到端可运行：Windows 全局输入 Hook 捕获键盘/鼠标事件，经状态模型去重/判定后，通过 WebSocket 广播给局域网网页客户端。

## 已完成

- [x] 编写需求文档：[软件需求.md](软件需求.md)
- [x] 编写架构设计：[项目架构设计.md](项目架构设计.md)
- [x] 编写 WebSocket 协议：[API_PROTOCOL.md](API_PROTOCOL.md)
- [x] 初始化 Rust 后端工程：[../backend/Cargo.toml](../backend/Cargo.toml)
- [x] 建立后端模块骨架：config / state / keyboard_hook / mouse_hook / websocket_server / ui / preset
- [x] 实现配置文件 `load_or_create`，默认路径为 `%APPDATA%/WebKeyLayer/config.toml`
- [x] 实现 WebSocket 多客户端 fanout 广播基础能力
- [x] 实现统一 WebSocket 消息信封与消息 ID
- [x] 实现连接建立消息与 heartbeat_ack
- [x] 将键盘状态统一到 `state::KeyboardState`
- [x] 实现 Windows 全局键盘 Hook（WH_KEYBOARD_LL）
- [x] 接入键盘事件到 WebSocket `key_pressed` / `key_released`
- [x] 实现键盘长按重复事件过滤
- [x] 实现鼠标方向状态机：方向变化事件 + 静止事件
- [x] 实现 Windows 全局鼠标 Hook（WH_MOUSE_LL）
- [x] 接入鼠标移动方向变化广播 `mouse_move_direction_changed`
- [x] 接入鼠标静止广播 `mouse_idle`
- [x] 接入鼠标按键广播 `mouse_button_pressed` / `mouse_button_released`
- [x] 接入鼠标滚轮广播 `mouse_wheel`
- [x] 主程序启动流程接入配置加载、WebSocket、键盘 Hook、鼠标 Hook
- [x] 实现本机局域网 IP 获取与端口可用性检测工具函数
- [x] 创建前端推流页最小可用版：[../frontend/public/index.html](../frontend/public/index.html)
- [x] 实现推流页 WebSocket 客户端：[../frontend/public/js/stream-client.js](../frontend/public/js/stream-client.js)
- [x] 实现推流页透明 Overlay 样式：[../frontend/public/css/stream.css](../frontend/public/css/stream.css)
- [x] 后端 8080 端口支持 `/public/` 静态资源访问与 `/stream` WebSocket 连接
- [x] 新增根目录 Cargo workspace，可从仓库根目录运行后端
- [x] 实现 `127.0.0.1:8888` 本地管理 HTTP API 服务
- [x] 实现 `GET /api/status`
- [x] 实现 `GET /api/config`
- [x] 实现 `POST /api/config`
- [x] 实现 `POST /api/config/reload`
- [x] 实现 `GET /api/network/ip`
- [x] 实现 `GET /api/preview`
- [x] 实现 `GET /api/logs` / `DELETE /api/logs` 占位接口
- [x] 实现 `GET /api/preset/list` 占位接口
- [x] 实现 `POST /api/service/start` / `POST /api/service/stop` 控制键盘/鼠标 Hook 生命周期
- [x] 为键盘状态与鼠标方向状态机添加最小自动化测试
- [x] 实现 Input Overlay JSON 顶层字段解析
- [x] 支持导入 `ET_TEXTURE`
- [x] 支持导入 `ET_KEYBOARD_KEY`
- [x] 支持导入 `ET_MOUSE_BUTTON`
- [x] 输出内部统一预设模型
- [x] 收集并返回不兼容项告警
- [x] 实现 `POST /api/preset/import`
- [x] `GET /api/preset/list` 返回已导入预设摘要
- [x] 管理页支持查看状态、服务启停、复制连接地址、保存配置、导入预设
- [x] 后端 8888 端口支持 `/admin` 管理页静态资源访问
- [x] 导入预设持久化到 `%APPDATA%/WebKeyLayer/presets.json`
- [x] 启动管理服务时自动加载已导入预设

## 进行中

- [ ] 托盘菜单与 WebView 宿主

## 下一步优先级

1. **前端推流页最小可用版**
   - [x] 创建 `/public` 静态页面
   - [x] 建立 WebSocket 客户端连接 `ws://<server_ip>:8080/stream`
   - [x] 渲染最小 WASD + 鼠标状态视图
   - [x] 响应 `key_pressed` / `key_released`
   - [x] 响应 `mouse_button_*`、`mouse_move_direction_changed`、`mouse_idle`、`mouse_wheel`
   - [x] 用浏览器实际联调 Overlay 显示效果

2. **本地管理服务**
   - [x] 启动 `127.0.0.1:8888` HTTP 服务
   - [x] 实现 `GET /api/status`
   - [x] 实现 `GET /api/config`
   - [x] 实现 `POST /api/config`
   - [x] 实现 `GET /api/network/ip`
   - [x] 实现服务启动/停止控制接口
   - [x] 提供 `/admin` 本地管理页面
   - [x] 管理页面接入状态、配置、网络、预设导入接口

3. **Input Overlay 预设兼容导入**
   - [x] 解析 Input Overlay JSON 顶层字段
   - [x] 支持 `ET_TEXTURE`
   - [x] 支持 `ET_KEYBOARD_KEY`
   - [x] 支持 `ET_MOUSE_BUTTON`
   - [x] 输出内部统一预设模型
   - [x] 收集并返回不兼容项告警
   - [x] 接入 `POST /api/preset/import`
   - [x] 管理页面提供预设导入入口
   - [x] 导入后保存预设并写回当前布局配置
   - [x] 重启后自动恢复已导入预设列表

4. **UI 与托盘**
   - [ ] 托盘菜单：打开管理面板
   - [ ] 托盘菜单：启动/停止服务
   - [ ] 托盘菜单：显示/复制连接地址
   - [ ] 托盘菜单：查看日志
   - [ ] 托盘菜单：退出程序

## 验证记录

- [x] `cargo fmt`
- [x] `cargo check`：通过，当前仅剩未实现骨架模块产生的 warning
- [x] `cargo check --manifest-path E:\my_project\WebKeyLayer\Cargo.toml`：根目录 workspace 编译通过
- [x] `cargo run --manifest-path backend/Cargo.toml`：已验证 WebSocket 与键盘 Hook 可启动
- [x] `cargo run --manifest-path backend/Cargo.toml`：已验证 WebSocket、键盘 Hook、鼠标 Hook 同时启动，并捕获到键盘/鼠标事件
- [x] `cargo run --manifest-path backend/Cargo.toml` + `http://127.0.0.1:8080/public/`：已验证浏览器推流页加载并连接 WebSocket
- [x] `cargo check --manifest-path Cargo.toml`：接入管理 API 后编译通过
- [x] `cargo run --manifest-path backend/Cargo.toml`：已验证管理 HTTP 服务启动于 `http://127.0.0.1:8888`
- [x] `Invoke-RestMethod http://127.0.0.1:8888/api/status`：已验证运行状态接口返回 Hook 与客户端状态
- [x] `Invoke-RestMethod http://127.0.0.1:8888/api/network/ip`：已验证连接地址接口返回推流页 URL
- [x] `POST /api/service/stop` + `POST /api/service/start`：已验证可停止并重新启动键盘/鼠标 Hook
- [x] `cargo test --manifest-path Cargo.toml`：新增 11 个测试并通过，覆盖键盘状态、鼠标方向状态机、Input Overlay 导入器、管理页静态资源、预设导入 API 和预设持久化
- [x] `cargo fmt --manifest-path Cargo.toml -- --check`：通过
- [x] `POST /api/preset/import` + 重启后 `GET /api/preset/list`：已验证 `gpro-wasd` 预设持久化并自动恢复

## 当前已知限制

- 托盘菜单和 WebView 宿主仍为骨架，当前通过浏览器访问 `http://127.0.0.1:8888/admin`
- 配置文件热重载监听尚未实现，当前通过管理页“重载”按钮手动触发
