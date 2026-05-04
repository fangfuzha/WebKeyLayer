# WebKeyLayer 前端开发指南

## 项目结构

```
frontend/
├── public/                                  # 推流显示页面
│   ├── index.html                          # 推流页面主文件
│   ├── css/
│   │   ├── themes.css                      # 主题样式 (浅色/深色/高对比度/系统跟随)
│   │   ├── styles.css                      # 按键样式模板 (方形/圆形/扁平/毛玻璃)
│   │   └── layout.css                      # 布局响应式样式
│   └── assets/
│       └── (预设贴图资源)
│
├── admin/                                  # 后端管理页面 (仅本地 127.0.0.1)
│   ├── index.html                          # 管理界面主页
│   ├── css/
│   │   ├── admin.css                       # 管理页面样式
│   │   └── theme-switcher.css              # 主题切换器样式
│   └── js/
│       ├── main.js                         # 管理页面主逻辑
│       ├── config-editor.js                # 配置编辑模块
│       ├── preview.js                      # 实时预览模块
│       ├── import-export.js                # 导入/导出功能
│       ├── logs.js                         # 日志查看模块
│       ├── i18n.js                         # 国际化管理
│       └── api-client.js                   # 后端 API 调用封装
│
├── src/                                    # 推流显示页 JS 模块
│   ├── render.js                           # 渲染引擎 (DOM 生成与更新)
│   ├── websocket-client.js                 # WebSocket 客户端 (连接、订阅、重连)
│   ├── config-manager.js                   # 配置管理 (后端配置下发、本地缓存)
│   ├── themes.js                           # 主题系统 (4 种主题)
│   ├── styles.js                           # 按键样式模板 (4 种风格)
│   ├── layout.js                           # 键盘布局管理 (全键盘/WASD/自定义)
│   ├── i18n/
│   │   ├── zh-CN.json                      # 中文国际化
│   │   └── en-US.json                      # 英文国际化
│   └── util.js                             # 工具函数
│
└── README.md (当前文件)
```

## 两个网页说明

### 1. 推流显示页面 (`/public/index.html`)

**用途**：在推流机上打开，通过 WebSocket 连接接收键盘事件，实时显示按键映射。

**特性**：

- 完全透明背景 (OBS/其他推流软件可直接捕获)
- 任意局域网设备可访问 (推流主机、笔记本、手机等)
- 实时按键高亮 (按下时显示高亮样式，松开后恢复)
- 配置由后端下发 (无需本地配置，纯客户端实现)
- 支持多种主题与样式切换

**访问地址**：`http://<服务器IP>:8080/public/`

**渲染流程**：

```
WebSocket 连接建立
    ↓
接收初始配置 (主题、布局、样式)
    ↓
生成 DOM 元素（键盘按键映射）
    ↓
[等待按键事件]
    ↓
接收 key_pressed 事件
    ↓
更新对应按键的 CSS 类 (高亮状态)
    ↓
接收 key_released 事件
    ↓
恢复按键默认样式
```

### 2. 后端管理页面 (`/admin/index.html`)

**用途**：在本地管理后端配置、预设、日志和诊断信息。

**特性**：

- 仅本地访问 (127.0.0.1，通过托盘打开)
- 配置编辑与实时预览
- 预设导入/导出
- 日志查看与诊断
- 服务启停控制
- 连接状态展示

**访问地址**：`http://127.0.0.1:8888/admin/`

**主要功能模块**：

1. **配置编辑** (Config Editor)
   - 主题选择 (浅色/深色/高对比度/系统跟随)
   - 按键样式模板 (方形/圆形/扁平/毛玻璃)
   - 布局选择 (全键盘/WASD/自定义)
   - 颜色自定义
   - 国际化设置 (中文/英文)

2. **实时预览** (Live Preview)
   - 显示当前配置的键盘映射预览
   - 实时响应配置变更
   - 模拟按键显示效果

3. **预设导入** (Import Presets)
   - 上传 Input Overlay JSON + 贴图文件
   - 选择兼容模式 (严格/宽松)
   - 显示导入结果与警告

4. **日志查看** (Logs Viewer)
   - 按日志级别过滤 (debug/info/warn/error)
   - 搜索日志内容
   - 导出日志文件

5. **诊断信息** (Status Dashboard)
   - 服务运行状态
   - 连接客户端数
   - 网络延迟统计
   - 错误与警告统计

---

## 核心技术模块

### 1. render.js (渲染引擎)

**职责**：DOM 元素生成与更新

```javascript
class KeyboardRenderer {
  // 从预设生成 DOM 元素
  renderKeyboard(preset, config)

  // 更新单个按键状态 (按下/松开)
  updateKeyState(keycode, pressed)

  // 应用主题和样式
  applyTheme(theme)
  applyStyleTemplate(style)
}
```

**性能注意**：

- 使用事件委托减少监听器数量
- 按键状态通过 CSS 类切换（避免重排）
- 缓存 DOM 元素引用

### 2. websocket-client.js (WebSocket 客户端)

**职责**：连接管理、事件订阅、自动重连

```javascript
class WebSocketClient {
  // 连接到服务器
  connect(url)

  // 订阅事件类型
  subscribe(eventType, handler)

  // 自动重连机制
  enableAutoReconnect(interval = 3000, maxAttempts = 10)

  // 发送心跳
  sendHeartbeat()
}
```

**消息处理**：

- `connection_established`: 初始化配置
- `key_pressed`: 高亮对应按键
- `key_released`: 恢复按键样式
- `config_updated`: 重新应用配置

### 3. config-manager.js (配置管理)

**职责**：处理后端配置下发、本地缓存

```javascript
class ConfigManager {
  // 获取当前配置
  getConfig()

  // 更新配置（通常由后端推送）
  updateConfig(config)

  // 本地持久化（sessionStorage）
  saveLocalCache()
  loadLocalCache()
}
```

### 4. themes.js (主题系统)

**职责**：支持 4 种主题的动态切换

**主题定义**：

```javascript
themes = {
  light: {
    keyColor: "#f0f0f0",
    textColor: "#333",
    highlightColor: "#FF5722",
  },
  dark: {
    keyColor: "#1a1a1a",
    textColor: "#fff",
    highlightColor: "#FF5722",
  },
  high_contrast: {
    keyColor: "#000",
    textColor: "#fff",
    highlightColor: "#FFFF00",
  },
  system: {
    // 自动跟随系统亮暗主题
  },
};
```

### 5. styles.js (按键样式)

**职责**：支持 4 种按键样式模板

**样式模板**：

- `square`: 直角、边框、常规风格
- `circle`: 圆角、柔和边界、现代感
- `flat`: 极简设计、阴影最小
- `glassmorphism`: 模糊背景、半透明、高级感

### 6. layout.js (布局管理)

**职责**：支持多种键盘布局

**布局类型**：

- `full-keyboard`: 完整 QWERTY 键盘（127 个按键）
- `wasd-minimal`: 精简 WASD + 必要功能键（24 个按键）
- `custom`: 用户自定义布局

### 7. i18n/ (国际化)

**职责**：多语言支持

**支持语言**：

- `zh-CN.json`: 中文
- `en-US.json`: 英文

```javascript
class I18n {
  // 初始化语言
  init(language = 'zh-CN')

  // 获取翻译字符串
  t(key)

  // 切换语言
  switchLanguage(language)
}
```

---

## 开发步骤

### Step 1: 推流显示页面基础

1. 创建 `public/index.html` 框架
2. 实现 WebSocket 连接 (websocket-client.js)
3. 实现基础渲染引擎 (render.js)
4. 测试从服务器接收配置

### Step 2: 主题与样式系统

1. 实现 4 种主题样式 (themes.css)
2. 实现 4 种按键风格 (styles.css)
3. 实现动态主题切换 (themes.js, styles.js)
4. 测试主题切换效果

### Step 3: 布局管理

1. 创建多种键盘布局定义 (layout.js)
2. 实现布局切换渲染
3. 支持响应式布局
4. 测试不同屏幕尺寸

### Step 4: 管理页面基础

1. 创建 `admin/index.html` 框架
2. 实现配置编辑表单
3. 实现实时预览 (preview.js)
4. 集成后端 API (api-client.js)

### Step 5: 管理页面高级功能

1. 预设导入/导出 (import-export.js)
2. 日志查看 (logs.js)
3. 诊断信息展示
4. 国际化界面 (i18n.js)

### Step 6: 测试与优化

1. 跨浏览器兼容性测试
2. 响应式布局测试
3. 性能优化（渲染速度）
4. 无障碍访问测试

---

## CSS 类名约定

### 推流页面

```html
<!-- 键盘容器 -->
<div class="keyboard-container">
  <!-- 单个按键 -->
  <div class="key-element" data-keycode="65" data-key-name="A">
    <span class="key-label">A</span>
  </div>
</div>

<!-- 按键状态 -->
<div class="key-element pressed"></div>
<!-- 按下状态 -->
<div class="key-element released"></div>
<!-- 松开状态 -->

<!-- 主题类 -->
<body class="theme-light"></body>
<body class="theme-dark"></body>

<!-- 样式类 -->
<body class="style-square"></body>
<body class="style-circle"></body>
```

### 管理页面

```html
<!-- 标签页 -->
<div class="tab-config active"></div>
<div class="tab-preview"></div>
<div class="tab-import"></div>

<!-- 表单 -->
<form class="config-form">
  <input class="config-input" type="text" />
  <select class="config-select"></select>
</form>
```

---

## 调试技巧

### 本地测试推流页面

```javascript
// 在浏览器控制台测试 WebSocket 连接
const ws = new WebSocket("ws://localhost:8080/stream");
ws.onmessage = (event) => {
  console.log("Received:", JSON.parse(event.data));
};
```

### 模拟按键事件

```javascript
// 在管理页面测试按键变更
fetch("/api/preview")
  .then((r) => r.json())
  .then((data) => {
    console.log(data);
  });
```

### 启用浏览器开发者工具

- F12 打开开发者工具
- Network 标签页查看 WebSocket 通信
- Console 查看日志和错误
- Performance 分析渲染性能

---

## 性能优化指标

| 指标     | 目标   | 实现方式                       |
| -------- | ------ | ------------------------------ |
| 首屏加载 | < 1s   | 懒加载资源，减少初始 HTML      |
| 帧率     | 60 FPS | CSS 类切换，避免 JS 重排       |
| 内存占用 | < 50MB | 及时清理事件监听，使用事件委托 |
| 网络延迟 | < 30ms | WebSocket 低延迟，消息最小化   |

---

## 浏览器兼容性

| 浏览器     | 支持版本 |
| ---------- | -------- |
| Chrome     | 90+      |
| Edge       | 90+      |
| Firefox    | 88+      |
| Safari     | 14+      |
| 手机浏览器 | 现代版本 |

---

## 开发工具建议

- **编辑器**: VS Code
- **浏览器**: Chrome DevTools
- **版本控制**: Git
- **API 测试**: Postman / Insomnia

---

## 常见问题

### Q: 推流页面为什么看不到按键？

A: 检查以下几点：

1. 确认后端 WebSocket 服务正在运行 (ws://IP:8080/stream)
2. 查看浏览器控制台是否有连接错误
3. 确认后端是否发送了 `connection_established` 消息

### Q: 按键延迟太高怎么办？

A:

1. 检查网络延迟 (在浏览器 Network 标签页查看)
2. 减少 DOM 操作数量 (使用事件委托)
3. 启用硬件加速 (浏览器设置)
4. 减小页面 CSS 复杂度

### Q: 支持手机吗？

A: 是的。推流页面使用响应式设计，支持手机浏览器访问。

---

**文档更新**: 2026-05-04  
**当前状态**: 目录结构完成，等待开发
