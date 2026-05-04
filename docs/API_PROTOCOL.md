# WebSocket 协议规范 (API_PROTOCOL)

**版本**: v0.1.0  
**最后更新**: 2026-05-04  
**状态**: 设计完成

---

## 目录

1. [协议概述](#协议概述)
2. [连接模型](#连接模型)
3. [消息格式](#消息格式)
4. [推流显示页协议](#推流显示页协议)
5. [后端管理页面 API](#后端管理页面-api)
6. [错误处理](#错误处理)
7. [性能指标](#性能指标)
8. [协议版本](#协议版本)

---

## 协议概述

WebKeyLayer 采用**混合通信架构**，支持多个通信通道：

| 通道           | 协议      | 源        | 目标                     | 用途             |
| -------------- | --------- | --------- | ------------------------ | ---------------- |
| 推流 WebSocket | WebSocket | Rust 后端 | 局域网客户端             | 实时按键事件广播 |
| 管理 HTTP      | HTTP REST | Rust 后端 | 本地管理网页 (127.0.0.1) | 配置管理与诊断   |

### 架构视图

```
┌─────────────────────────────────────────────────────────┐
│  Rust Backend (0.0.0.0:8080)                            │
│  ├─ WebSocket Server (事件广播)                         │
│  └─ HTTP Server (127.0.0.1:8888, 管理接口)              │
└────┬────────────────────────────────────┬───────────────┘
     │                                    │
     ▼                                    ▼
┌─────────────────────────┐      ┌──────────────────────┐
│ 推流客户端               │      │ 管理网页              │
│ (任意局域网设备)         │      │ (仅本地 127.0.0.1)    │
│ - 推流主机              │      │ - 配置编辑            │
│ - 笔记本                │      │ - 预览与诊断          │
│ - 手机/平板             │      │ - 日志查看            │
└─────────────────────────┘      └──────────────────────┘
```

---

## 连接模型

### 推流客户端连接生命周期

```
客户端                              服务端
   │                                 │
   ├─────── WebSocket CONNECT ──────>│
   │                                 │
   │<───── ConnectionEstablished ────┤
   │      (包含初始配置信息)           │
   │                                 │
   ├─────── Subscribe "key_event" ──>│
   │                                 │
   │                                 │ [用户按下按键]
   │<────── key_pressed ──────────────┤
   │        {keycode, timestamp}      │
   │                                 │
   │                                 │ [用户松开按键]
   │<────── key_released ─────────────┤
   │        {keycode, timestamp}      │
   │                                 │
   │<────── config_updated ──────────┤
   │        [配置变更时广播]           │
   │                                 │
   ├─────── Heartbeat (每 30s) ─────>│
   │                                 │
   │<────── Heartbeat ACK ───────────┤
   │                                 │
   ├─────── WebSocket DISCONNECT ───>│
   │                                 │
   └─────────────────────────────────┘
```

---

## 消息格式

### 通用消息信封 (JSON)

所有 WebSocket 消息都遵循统一的信封格式：

```json
{
  "id": "msg_uuid_or_counter",     // 消息 ID（用于去重）
  "type": "key_pressed",            // 消息类型
  "timestamp": 1714815600000,       // Unix 毫秒时间戳
  "payload": { ... }                // 具体消息内容
}
```

**字段说明**:

- `id`: 唯一标识，客户端可用此去重；大数据量场景下可选
- `type`: 消息类型枚举
- `timestamp`: 事件发生时间（服务端填充）
- `payload`: 根据 `type` 决定内容结构

---

## 推流显示页协议

### WebSocket 连接

**URL**: `ws://<server_ip>:8080/stream`  
**默认绑定**: `0.0.0.0:8080` (所有网卡)  
**超时**: 30 秒无心跳自动断开重连

### 消息类型定义

#### 1. 连接建立

**方向**: 服务端 → 客户端  
**触发**: WebSocket 连接成功

```json
{
  "id": "connection_established",
  "type": "connection_established",
  "timestamp": 1714815600000,
  "payload": {
    "server_version": "0.1.0",
    "protocol_version": "1.0",
    "client_id": "client_uuid",
    "config": {
      "theme": {
        "mode": "light",
        "primary_color": "#333333",
        "highlight_color": "#FF5722"
      },
      "preset": {
        "layout": "wasd-minimal",
        "style": "square"
      },
      "ui": {
        "transparency": 0.9,
        "scale": 1.0
      },
      "i18n": {
        "language": "zh-CN"
      }
    }
  }
}
```

#### 2. 按键按下事件

**方向**: 服务端 → 客户端  
**触发**: 用户按下键盘按键

```json
{
  "id": "key_001",
  "type": "key_pressed",
  "timestamp": 1714815600100,
  "payload": {
    "keycode": 65, // 按键码（A 键）
    "key_name": "A", // 可读的按键名称
    "modifiers": ["Shift"], // 同时按下的修饰键
    "client_id": "client_uuid"
  }
}
```

**常用按键码参考** (Virtual Key Code):

- 字母: 65-90 (A-Z)
- 数字: 48-57 (0-9)
- 方向键: 37-40 (左右上下)
- 功能键: 112-123 (F1-F12)
- 修饰键: 16 (Shift), 17 (Ctrl), 18 (Alt), 91 (Win)
- WASD: 87, 65, 83, 68

#### 3. 按键松开事件

**方向**: 服务端 → 客户端  
**触发**: 用户松开键盘按键

```json
{
  "id": "key_002",
  "type": "key_released",
  "timestamp": 1714815600150,
  "payload": {
    "keycode": 65,
    "key_name": "A",
    "modifiers": []
  }
}
```

#### 2b. 鼠标按键按下事件

**方向**: 服务端 → 客户端  
 **触发**: 用户按下鼠标按键

```json
{
  "id": "mouse_btn_001",
  "type": "mouse_button_pressed",
  "timestamp": 1714815600110,
  "payload": {
    "button": 1, // 1=left, 2=right, 3=middle
    "pressed": true,
    "x": 1024, // 全局或相对屏幕坐标（按需约定）
    "y": 768,
    "modifiers": ["Shift"]
  }
}
```

#### 2c. 鼠标按键松开事件

**方向**: 服务端 → 客户端  
 **触发**: 用户松开鼠标按键

```json
{
  "id": "mouse_btn_002",
  "type": "mouse_button_released",
  "timestamp": 1714815600120,
  "payload": {
    "button": 1,
    "pressed": false,
    "x": 1025,
    "y": 768,
    "modifiers": []
  }
}
```

#### 2d. 鼠标方向变化事件（相对位移）

**方向**: 服务端 → 客户端  
**触发**: 相对于上一次采样，鼠标方向发生变化

注意：为降低带宽与渲染压力，鼠标移动使用**相对位移**（dx, dy）而非绝对坐标，dx/dy 表示相对于上一次采样的位移。

```json
{
  "id": "mouse_move_001",
  "type": "mouse_move_direction_changed",
  "timestamp": 1714815600130,
  "payload": {
    "dx": 24,
    "dy": -5,
    "direction": "up_right"
  }
}
```

方向枚举：`up` / `down` / `left` / `right` / `up_left` / `up_right` / `down_left` / `down_right`

#### 2e. 鼠标静止事件

**方向**: 服务端 → 客户端  
**触发**: 相对于上一次采样，鼠标无位移（进入静止状态时发送一次）

```json
{
  "id": "mouse_idle_001",
  "type": "mouse_idle",
  "timestamp": 1714815600135,
  "payload": {
    "state": "idle"
  }
}
```

#### 2f. 鼠标滚轮事件

**方向**: 服务端 → 客户端  
 **触发**: 鼠标滚轮滚动

```json
{
  "id": "mouse_wheel_001",
  "type": "mouse_wheel",
  "timestamp": 1714815600140,
  "payload": {
    "delta": -120,
    "x": 1200,
    "y": 400
  }
}
```

#### 4. 配置更新事件

**方向**: 服务端 → 客户端  
**触发**: 管理页面修改配置后

```json
{
  "id": "config_update_001",
  "type": "config_updated",
  "timestamp": 1714815600200,
  "payload": {
    "changes": {
      "theme.mode": "dark",
      "preset.layout": "qwerty-full",
      "preset.style": "circle"
    },
    "full_config": {
      "theme": { ... },
      "preset": { ... },
      "ui": { ... },
      "i18n": { ... }
    }
  }
}
```

#### 5. 心跳消息 (Heartbeat)

**方向**: 双向  
**触发**: 每 30 秒无通信自动发送

**客户端 → 服务端**:

```json
{
  "id": "heartbeat_client_001",
  "type": "heartbeat",
  "timestamp": 1714815630000,
  "payload": {
    "client_id": "client_uuid"
  }
}
```

**服务端 → 客户端**:

```json
{
  "id": "heartbeat_server_001",
  "type": "heartbeat_ack",
  "timestamp": 1714815630000,
  "payload": {
    "server_time": 1714815630000,
    "connected_clients": 5,
    "latency_ms": 12
  }
}
```

#### 6. 连接关闭

**方向**: 双向  
**触发**: 任意一方主动关闭连接

```json
{
  "id": "disconnect_001",
  "type": "disconnect",
  "timestamp": 1714815700000,
  "payload": {
    "reason": "client_closed", // 或 "server_shutdown", "timeout", "error"
    "message": "User closed the window"
  }
}
```

---

## 后端管理页面 API

### HTTP 服务配置

**基础 URL**: `http://127.0.0.1:8888`  
**内容类型**: `application/json`  
**认证**: 无（仅本地访问受保护）

### API 端点

#### 1. 获取当前配置

```
GET /api/config
```

**响应**:

```json
{
  "code": 0,
  "message": "success",
  "data": {
    "theme": {
      "mode": "light",
      "primary_color": "#333333",
      "highlight_color": "#FF5722"
    },
    "preset": {
      "layout": "wasd-minimal",
      "style": "square"
    },
    "ui": {
      "transparency": 0.9,
      "scale": 1.0
    },
    "network": {
      "port": 8080,
      "bind_address": "0.0.0.0"
    },
    "i18n": {
      "language": "zh-CN"
    }
  }
}
```

#### 2. 保存配置

```
POST /api/config
Content-Type: application/json

{
  "theme": { ... },
  "preset": { ... },
  "ui": { ... },
  "i18n": { ... }
}
```

**响应**:

```json
{
  "code": 0,
  "message": "Configuration saved successfully"
}
```

#### 3. 热重载配置

```
POST /api/config/reload
```

**响应**:

```json
{
  "code": 0,
  "message": "Configuration reloaded"
}
```

#### 4. 获取预设列表

```
GET /api/preset/list
```

**响应**:

```json
{
  "code": 0,
  "data": [
    {
      "name": "qwerty-full",
      "version": "1.0",
      "description": "Complete QWERTY keyboard",
      "width": 1920,
      "height": 640,
      "elements_count": 127
    },
    {
      "name": "wasd-minimal",
      "version": "1.0",
      "description": "Minimal WASD + Essential keys",
      "width": 400,
      "height": 300,
      "elements_count": 24
    }
  ]
}
```

#### 5. 导入预设

```
POST /api/preset/import
Content-Type: multipart/form-data

{
  "preset_file": <JSON file>,
  "texture_file": <PNG/JPG file>,
  "mode": "strict"  // 或 "lenient"
}
```

**响应**:

```json
{
  "code": 0,
  "message": "Preset imported successfully",
  "data": {
    "preset_name": "qwerty-full",
    "warnings": ["Element 'ET_GAMEPAD_BUTTON' type not supported in this phase"]
  }
}
```

#### 6. 获取实时预览数据

```
GET /api/preview
```

**响应**:

```json
{
  "code": 0,
  "data": {
    "current_layout": "wasd-minimal",
    "pressed_keys": [65, 83], // A, S 按键码
    "all_elements": [
      {
        "id": "key_w",
        "keycode": 87,
        "pressed": false,
        "position": { "x": 50, "y": 50 },
        "texture": { "x": 0, "y": 0, "width": 40, "height": 40 }
      }
    ]
  }
}
```

#### 7. 获取日志

```
GET /api/logs?level=debug&limit=100&offset=0
```

**查询参数**:

- `level`: 日志级别 (debug, info, warn, error)
- `limit`: 返回条数 (默认 100)
- `offset`: 偏移量 (默认 0)

**响应**:

```json
{
  "code": 0,
  "data": [
    {
      "timestamp": 1714815600000,
      "level": "info",
      "message": "Keyboard hook started",
      "module": "keyboard_hook"
    }
  ]
}
```

#### 8. 获取运行状态

```
GET /api/status
```

**响应**:

```json
{
  "code": 0,
  "data": {
    "service_running": true,
    "keyboard_hook_active": true,
    "connected_clients": 5,
    "websocket_server_uptime_ms": 3600000,
    "latency_stats": {
      "min_ms": 8,
      "max_ms": 42,
      "avg_ms": 15
    },
    "errors_count": 0,
    "warnings_count": 2
  }
}
```

#### 9. 启动服务

```
POST /api/service/start
```

**响应**:

```json
{
  "code": 0,
  "message": "Service started successfully"
}
```

#### 10. 停止服务

```
POST /api/service/stop
```

**响应**:

```json
{
  "code": 0,
  "message": "Service stopped successfully"
}
```

#### 11. 获取本机网络信息

```
GET /api/network/ip
```

**响应**:

```json
{
  "code": 0,
  "data": {
    "local_ips": ["192.168.1.100", "10.0.0.50"],
    "websocket_port": 8080,
    "admin_port": 8888,
    "connection_url": "ws://192.168.1.100:8080/stream",
    "admin_url": "http://127.0.0.1:8888"
  }
}
```

---

## 错误处理

### WebSocket 错误响应

```json
{
  "id": "error_001",
  "type": "error",
  "timestamp": 1714815600000,
  "payload": {
    "error_code": "INVALID_MESSAGE_FORMAT",
    "message": "Failed to parse message",
    "details": "Expected 'type' field in payload"
  }
}
```

**错误码定义**:

- `INVALID_MESSAGE_FORMAT`: 消息格式不正确
- `UNKNOWN_MESSAGE_TYPE`: 未知的消息类型
- `SERVER_OVERLOAD`: 服务器过载
- `INTERNAL_ERROR`: 内部服务器错误
- `CONNECTION_TIMEOUT`: 连接超时

### HTTP 错误响应

```json
{
  "code": -1,
  "message": "Error description",
  "error_code": "ERROR_CODE"
}
```

**HTTP 状态码**:

- `200 OK`: 成功
- `400 Bad Request`: 请求参数错误
- `404 Not Found`: 资源不存在
- `500 Internal Server Error`: 服务器错误
- `503 Service Unavailable`: 服务不可用

---

## 性能指标

### 延迟目标

| 指标                   | 目标   | 说明                   |
| ---------------------- | ------ | ---------------------- |
| 按键事件端到端延迟     | < 30ms | 从物理按键到客户端显示 |
| WebSocket 消息往返时间 | < 20ms | 在同一局域网内         |
| 心跳响应时间           | < 5ms  | 连接保活检测           |

### 吞吐量目标

- **最大并发客户端数**: 8+
- **按键事件处理速率**: 1000 事件/秒（每秒最多 1000 个按键事件）
- **心跳间隔**: 30 秒
- **消息序列化大小**: < 200 字节（单个按键事件）

### 带宽估算

- **正常情况**: ~0.5-1 KB/秒（按键每秒平均 5 次）
- **高强度使用**: ~50 KB/秒（按键每秒 100 次）
- **多客户端**: 带宽随客户端数线性增长

---

## 协议版本

### v1.0 (当前)

**阶段特性**:

- ✅ 按键事件（字母、数字、方向键、功能键、WASD）
- ✅ 配置更新广播
- ✅ 心跳与连接管理
- ✅ 管理 HTTP API (基础操作)
- ✅ 第一阶段预设元素类型（ET_KEYBOARD_KEY, ET_MOUSE_BUTTON, ET_TEXTURE）

**更新**: v1.0 同时支持鼠标事件（鼠标按键、移动与滚轮）——这是当前版本的功能要求。

### 未来规划 (v1.1+)

- 🔄 鼠标事件广播
- 🔄 手柄事件支持
- 🔄 第二阶段预设类型（ET_WHEEL, ET_MOUSE_MOVEMENT, etc.)
- 🔄 消息压缩 (gzip)
- 🔄 二进制消息格式 (MessagePack/Protocol Buffers)
- 🔄 公网穿透支持（隧道代理）

---

## 附录：客户端实现示例

### JavaScript 客户端连接

```javascript
const serverUrl = "ws://192.168.1.100:8080/stream";
const ws = new WebSocket(serverUrl);

ws.onopen = (event) => {
  console.log("Connected to server");
};

ws.onmessage = (event) => {
  const message = JSON.parse(event.data);

  if (message.type === "key_pressed") {
    console.log(`Key pressed: ${message.payload.key_name}`);
    // 更新 UI 显示按键被按下
  } else if (message.type === "key_released") {
    console.log(`Key released: ${message.payload.key_name}`);
    // 更新 UI 显示按键被松开
  } else if (message.type === "config_updated") {
    console.log("Configuration updated");
    // 重新应用新配置
  }
};

ws.onerror = (error) => {
  console.error("WebSocket error:", error);
};

ws.onclose = () => {
  console.log("Disconnected from server");
};

// 发送心跳
setInterval(() => {
  if (ws.readyState === WebSocket.OPEN) {
    ws.send(
      JSON.stringify({
        id: Date.now(),
        type: "heartbeat",
        timestamp: Date.now(),
        payload: { client_id: "my_client" },
      }),
    );
  }
}, 30000);
```

---

**协议审查清单**:

- [x] 消息格式设计
- [x] 连接生命周期
- [x] 错误处理
- [x] 性能指标
- [x] 管理 API
- [ ] 二进制格式（未来版本）
- [ ] 认证机制（未来版本）
