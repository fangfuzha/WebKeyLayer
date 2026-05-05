(() => {
  const keyElements = new Map();
  const keyAliases = new Map();
  for (const element of document.querySelectorAll("[data-keycodes]")) {
    const aliases = element.dataset.keycodes
      .split(",")
      .map((keycode) => Number(keycode.trim()))
      .filter(Number.isFinite);
    keyAliases.set(element, aliases);
    for (const keycode of aliases) {
      keyElements.set(keycode, element);
    }
  }
  const mouseButtons = new Map(
    [...document.querySelectorAll("[data-mouse-button]")].map((element) => [
      Number(element.dataset.mouseButton),
      element,
    ]),
  );
  const statusElement = document.querySelector("[data-status]");
  const directionElement = document.querySelector("[data-direction]");
  const wheelElement = document.querySelector("[data-wheel]");

  const directionGlyphs = {
    up: "↑",
    down: "↓",
    left: "←",
    right: "→",
    up_left: "↖",
    up_right: "↗",
    down_left: "↙",
    down_right: "↘",
    idle: "•",
  };

  let socket = null;
  let reconnectTimer = 0;
  let heartbeatTimer = 0;
  let wheelResetTimer = 0;
  const activeKeycodes = new Set();

  function resolveWebSocketUrl() {
    const params = new URLSearchParams(window.location.search);
    const override = params.get("ws");
    if (override) {
      return override;
    }

    if (window.location.protocol === "file:") {
      return "ws://127.0.0.1:8080/stream";
    }

    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const hostname = window.location.hostname || "127.0.0.1";
    const port = window.location.port || "8080";
    return `${protocol}//${hostname}:${port}/stream`;
  }

  function setStatus(status) {
    statusElement.dataset.status = status;
    statusElement.textContent = status.toUpperCase();
  }

  function connect() {
    clearTimeout(reconnectTimer);
    clearInterval(heartbeatTimer);
    setStatus("connecting");

    socket = new WebSocket(resolveWebSocketUrl());

    socket.addEventListener("open", () => {
      setStatus("connected");
      heartbeatTimer = window.setInterval(sendHeartbeat, 30000);
    });

    socket.addEventListener("message", (event) => {
      const message = parseMessage(event.data);
      if (message) {
        handleMessage(message);
      }
    });

    socket.addEventListener("close", () => {
      clearInterval(heartbeatTimer);
      setStatus("disconnected");
      reconnectTimer = window.setTimeout(connect, 1200);
    });

    socket.addEventListener("error", () => {
      setStatus("error");
    });
  }

  function parseMessage(raw) {
    try {
      return JSON.parse(raw);
    } catch {
      return null;
    }
  }

  function sendHeartbeat() {
    if (!socket || socket.readyState !== WebSocket.OPEN) {
      return;
    }

    socket.send(
      JSON.stringify({
        id: `heartbeat_${Date.now()}`,
        type: "heartbeat",
        timestamp: Date.now(),
        payload: {},
      }),
    );
  }

  function handleMessage(message) {
    const payload = message.payload || {};
    switch (message.type) {
      case "connection_established":
      case "heartbeat_ack":
        break;
      case "key_pressed":
      case "key_released":
        updateKey(payload.keycode, message.type === "key_pressed");
        break;
      case "mouse_button_pressed":
      case "mouse_button_released":
        updateMouseButton(payload.button, message.type === "mouse_button_pressed");
        break;
      case "mouse_move_direction_changed":
        updateDirection(payload.direction);
        break;
      case "mouse_idle":
        updateDirection("idle");
        break;
      case "mouse_wheel":
        updateWheel(payload.delta);
        break;
      default:
        break;
    }
  }

  function updateKey(keycode, pressed) {
    const numericKeycode = Number(keycode);
    const element = keyElements.get(numericKeycode);
    if (!element) {
      return;
    }

    if (pressed) {
      activeKeycodes.add(numericKeycode);
    } else {
      activeKeycodes.delete(numericKeycode);
    }

    const aliases = keyAliases.get(element) || [numericKeycode];
    element.classList.toggle(
      "is-active",
      aliases.some((alias) => activeKeycodes.has(alias)),
    );
  }

  function updateMouseButton(button, pressed) {
    const element = mouseButtons.get(Number(button));
    if (!element) {
      return;
    }
    element.classList.toggle("is-active", pressed);
  }

  function updateDirection(direction) {
    const glyph = directionGlyphs[direction] || directionGlyphs.idle;
    directionElement.textContent = glyph;
    directionElement.classList.toggle("is-active", direction !== "idle");
  }

  function updateWheel(delta) {
    const value = Number(delta) || 0;
    wheelElement.classList.remove("is-wheel-up", "is-wheel-down");
    if (value === 0) {
      return;
    }

    wheelElement.classList.add(value > 0 ? "is-wheel-up" : "is-wheel-down");
    clearTimeout(wheelResetTimer);
    wheelResetTimer = window.setTimeout(() => {
      wheelElement.classList.remove("is-wheel-up", "is-wheel-down");
    }, 420);
  }

  connect();
})();
