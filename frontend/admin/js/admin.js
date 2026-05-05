(() => {
  const state = {
    config: null,
    network: null,
    dirty: false,
  };

  const nodes = {
    servicePill: document.querySelector("[data-service-state]"),
    serviceLabel: document.querySelector("[data-service-label]"),
    clients: document.querySelector("[data-clients]"),
    keyboard: document.querySelector("[data-keyboard]"),
    mouse: document.querySelector("[data-mouse]"),
    errors: document.querySelector("[data-errors]"),
    warnings: document.querySelector("[data-warnings]"),
    configPath: document.querySelector("[data-config-path]"),
    configState: document.querySelector("[data-config-state]"),
    streamUrl: document.querySelector("[data-stream-url]"),
    wsUrl: document.querySelector("[data-ws-url]"),
    preview: document.querySelector("[data-preview]"),
    previewState: document.querySelector("[data-preview-state]"),
    presetFile: document.querySelector("[data-preset-file]"),
    importMode: document.querySelector("[data-import-mode]"),
    presetList: document.querySelector("[data-preset-list]"),
    presetCount: document.querySelector("[data-preset-count]"),
    toast: document.querySelector("[data-toast]"),
  };

  document.querySelectorAll("[data-action]").forEach((button) => {
    button.addEventListener("click", () => handleAction(button.dataset.action));
  });

  document.querySelectorAll("[data-copy]").forEach((button) => {
    button.addEventListener("click", () => copyValue(button.dataset.copy));
  });

  document.querySelectorAll("[data-field]").forEach((field) => {
    field.addEventListener("input", () => {
      state.dirty = true;
      nodes.configState.textContent = "已修改";
    });
  });

  async function boot() {
    await Promise.all([loadStatus(), loadNetwork(), loadConfig(), loadPresets()]);
    window.setInterval(loadStatus, 2500);
  }

  async function handleAction(action) {
    try {
      switch (action) {
        case "start":
          await apiPost("/api/service/start", {});
          await loadStatus();
          showToast("服务已启动");
          break;
        case "stop":
          await apiPost("/api/service/stop", {});
          await loadStatus();
          showToast("服务已停止");
          break;
        case "refresh":
          await Promise.all([loadStatus(), loadNetwork(), loadPresets()]);
          showToast("状态已刷新");
          break;
        case "save-config":
          await saveConfig();
          break;
        case "reload-config":
          await loadConfig();
          showToast("配置已重载");
          break;
        case "import-preset":
          await importPreset();
          break;
        default:
          break;
      }
    } catch (error) {
      showToast(error.message || String(error));
    }
  }

  async function loadStatus() {
    const response = await apiGet("/api/status");
    const data = response.data;
    const running = Boolean(data.service_running);
    nodes.servicePill.dataset.serviceState = running ? "running" : "stopped";
    nodes.serviceLabel.textContent = running ? "运行中" : "已停止";
    nodes.clients.textContent = `${data.connected_clients || 0} 客户端`;
    nodes.keyboard.textContent = data.keyboard_hook_active ? "启用" : "关闭";
    nodes.mouse.textContent = data.mouse_hook_active ? "启用" : "关闭";
    nodes.errors.textContent = data.errors_count || 0;
    nodes.warnings.textContent = data.warnings_count || 0;
    nodes.configPath.textContent = data.config_path || "";
  }

  async function loadNetwork() {
    const response = await apiGet("/api/network/ip");
    state.network = response.data;
    nodes.streamUrl.value = state.network.stream_url || "";
    nodes.wsUrl.value = state.network.connection_url || "";
    if (nodes.preview.src !== nodes.streamUrl.value) {
      nodes.preview.src = nodes.streamUrl.value;
      nodes.previewState.textContent = "已加载";
    }
  }

  async function loadConfig() {
    const response = await apiGet("/api/config");
    state.config = response.data;
    setField("theme.mode", state.config.theme.mode);
    setField("preset.style", state.config.preset.style);
    setField("ui.transparency", state.config.ui.transparency);
    setField("ui.scale", state.config.ui.scale);
    state.dirty = false;
    nodes.configState.textContent = "未修改";
  }

  async function loadPresets() {
    const response = await apiGet("/api/preset/list");
    const presets = response.data || [];
    nodes.presetCount.textContent = `${presets.length} 个`;
    nodes.presetList.innerHTML = "";

    if (!presets.length) {
      nodes.presetList.innerHTML = '<tr><td colspan="3">暂无预设</td></tr>';
      return;
    }

    for (const preset of presets) {
      const row = document.createElement("tr");
      row.innerHTML = `
        <td>${escapeHtml(preset.name)}</td>
        <td>${preset.width} x ${preset.height}</td>
        <td>${preset.elements_count}</td>
      `;
      nodes.presetList.appendChild(row);
    }
  }

  async function saveConfig() {
    if (!state.config) {
      return;
    }

    const patch = structuredClone(state.config);
    patch.theme.mode = getField("theme.mode");
    patch.preset.style = getField("preset.style");
    patch.ui.transparency = Number(getField("ui.transparency"));
    patch.ui.scale = Number(getField("ui.scale"));

    const response = await apiPost("/api/config", patch);
    state.config = response.data.config;
    state.dirty = false;
    nodes.configState.textContent = response.data.requires_restart ? "需重启推流服务" : "已保存";
    showToast("配置已保存");
  }

  async function importPreset() {
    const file = nodes.presetFile.files[0];
    if (!file) {
      showToast("请选择 JSON 预设文件");
      return;
    }

    const content = await file.text();
    const response = await apiPost("/api/preset/import", {
      file_name: file.name,
      mode: nodes.importMode.value,
      content,
    });
    await loadPresets();
    const warnings = response.data.warnings || [];
    showToast(warnings.length ? `导入完成，${warnings.length} 个告警` : "预设已导入");
  }

  async function copyValue(kind) {
    const value = kind === "stream" ? nodes.streamUrl.value : nodes.wsUrl.value;
    if (!value) {
      return;
    }
    await navigator.clipboard.writeText(value);
    showToast("已复制");
  }

  function setField(path, value) {
    const field = document.querySelector(`[data-field="${path}"]`);
    if (field) {
      field.value = value;
    }
  }

  function getField(path) {
    return document.querySelector(`[data-field="${path}"]`)?.value;
  }

  async function apiGet(path) {
    const response = await fetch(path);
    return parseApiResponse(response);
  }

  async function apiPost(path, body) {
    const response = await fetch(path, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    return parseApiResponse(response);
  }

  async function parseApiResponse(response) {
    const payload = await response.json();
    if (!response.ok || payload.code !== 0) {
      throw new Error(payload.message || `HTTP ${response.status}`);
    }
    return payload;
  }

  function showToast(message) {
    nodes.toast.textContent = message;
    nodes.toast.hidden = false;
    window.clearTimeout(showToast.timer);
    showToast.timer = window.setTimeout(() => {
      nodes.toast.hidden = true;
    }, 2600);
  }

  function escapeHtml(value) {
    return String(value).replace(/[&<>"']/g, (char) => {
      const entities = {
        "&": "&amp;",
        "<": "&lt;",
        ">": "&gt;",
        '"': "&quot;",
        "'": "&#39;",
      };
      return entities[char];
    });
  }

  boot().catch((error) => showToast(error.message || String(error)));
})();
