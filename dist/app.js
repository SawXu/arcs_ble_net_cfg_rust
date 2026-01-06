const getTauriApi = () => {
  const tauriApi = window.__TAURI__ || {};
  const invokeFn = tauriApi.core?.invoke || tauriApi.tauri?.invoke || tauriApi.invoke;
  const listenFn = tauriApi.event?.listen;
  return { invoke: invokeFn, listen: listenFn };
};

const deviceList = document.getElementById("device-list");
const pageScan = document.getElementById("page-scan");
const pageConfig = document.getElementById("page-config");
const scanHint = document.getElementById("scan-hint");
const deviceStatus = document.getElementById("device-status");
const logEl = document.getElementById("log");
const statusEl = document.getElementById("status");
const scanButton = document.getElementById("scan");
const connectOverlay = document.getElementById("connect-overlay");
const connectTarget = document.getElementById("connect-target");

let scanning = false;
let connectingId = null;

const log = (message) => {
  const ts = new Date().toLocaleTimeString();
  logEl.textContent += `[${ts}] ${message}\n`;
  logEl.scrollTop = logEl.scrollHeight;
};

const setStatusState = (message, state) => {
  statusEl.textContent = message;
  statusEl.classList.remove("success", "error", "info");
  if (state) {
    statusEl.classList.add(state);
  }
};

const setConnected = (name) => {
  deviceStatus.textContent = name ? `已连接: ${name}` : "未连接";
};

const showScanPage = () => {
  pageScan.classList.remove("hidden");
  pageConfig.classList.add("hidden");
};

const showConfigPage = () => {
  pageScan.classList.add("hidden");
  pageConfig.classList.remove("hidden");
};

const loadDevices = (devices) => {
  deviceList.innerHTML = "";
  if (!devices.length) {
    const empty = document.createElement("li");
    empty.className = "device-item";
    empty.textContent = "未发现设备";
    deviceList.appendChild(empty);
    return;
  }
  devices.forEach((device) => {
    const item = document.createElement("li");
    item.className = "device-item";
    item.dataset.id = device.id;

    const left = document.createElement("div");
    const title = document.createElement("div");
    const tag = device.matched ? " *" : "";
    title.textContent = `${device.name}${tag}`;
    const meta = document.createElement("div");
    meta.className = "device-meta";
    const rssi = device.rssi === null ? "RSSI: -" : `RSSI: ${device.rssi}`;
    const mac = device.id ? `MAC: ${device.id}` : "MAC: -";
    meta.textContent = `${mac} · ${rssi}`;
    left.appendChild(title);
    left.appendChild(meta);

    const action = document.createElement("span");
    action.className = "device-meta";
    action.textContent = "点击连接";

    item.appendChild(left);
    item.appendChild(action);
    item.addEventListener("click", () => connectDevice(device.id, device.name, item, action));
    deviceList.appendChild(item);
  });
};

const scanDevices = async () => {
  if (scanning) {
    log("扫描进行中，请稍候...");
    return;
  }
  scanning = true;
  scanButton.disabled = true;
  log("开始扫描...");
  try {
    const { invoke } = getTauriApi();
    if (!invoke) {
      log("Tauri API 未就绪，无法调用扫描");
      return;
    }
    scanHint.textContent = "正在扫描...";
    const devices = await invoke("scan_devices", { timeout_ms: 3000 });
    const filtered = devices.filter((device) => device.name && device.name !== "Unknown");
    loadDevices(filtered);
    scanHint.textContent = `扫描完成，发现 ${filtered.length} 个设备`;
    log(`扫描完成，发现 ${filtered.length} 个设备`);
  } catch (err) {
    log(`扫描失败: ${err}`);
    scanHint.textContent = "扫描失败，请重试";
  } finally {
    scanning = false;
    scanButton.disabled = false;
  }
};

const connectDevice = async (id, name, item, actionEl) => {
  if (connectingId) {
    log("正在连接设备，请稍候...");
    return;
  }
  try {
    const { invoke } = getTauriApi();
    if (!invoke) {
      log("Tauri API 未就绪，无法连接设备");
      return;
    }
    connectingId = id;
    if (connectTarget) {
      connectTarget.textContent = name || id || "设备";
    }
    if (connectOverlay) {
      connectOverlay.classList.remove("hidden");
    }
    if (item) {
      item.classList.add("disabled");
    }
    if (actionEl) {
      actionEl.textContent = "连接中...";
    }
    log(`开始连接: ${name || id}`);
    await invoke("connect_device", { id });
    setConnected(name);
    showConfigPage();
    log("连接成功");
  } catch (err) {
    log(`连接失败: ${err}`);
  } finally {
    if (connectingId === id) {
      connectingId = null;
      if (connectOverlay) {
        connectOverlay.classList.add("hidden");
      }
      if (item) {
        item.classList.remove("disabled");
      }
      if (actionEl) {
        actionEl.textContent = "点击连接";
      }
    }
  }
};

const disconnectDevice = async () => {
  try {
    const { invoke } = getTauriApi();
    if (!invoke) {
      log("Tauri API 未就绪，无法断开设备");
      return;
    }
    await invoke("disconnect_device");
    setConnected("");
    showScanPage();
    log("已断开连接");
  } catch (err) {
    log(`断开失败: ${err}`);
  }
};

const configureWifi = async () => {
  const ssid = document.getElementById("ssid").value.trim();
  const password = document.getElementById("password").value;
  if (!ssid) {
    log("请输入 SSID");
    return;
  }
  try {
    const { invoke } = getTauriApi();
    if (!invoke) {
      log("Tauri API 未就绪，无法发送配网");
      return;
    }
    setStatusState("配网中...", "info");
    await invoke("configure_wifi", { ssid, password });
    log("配网指令已发送");
  } catch (err) {
    log(`配网失败: ${err}`);
    setStatusState("配网失败", "error");
  }
};

const attachListener = async () => {
  const { listen } = getTauriApi();
  if (!listen) {
    log("Tauri 事件系统未就绪，无法接收状态通知");
    return false;
  }
  try {
    await listen("netcfg_status", (event) => {
      const { code, name, hex, raw_hex: rawHex } = event.payload;
      if (code === 0x0104) {
        setStatusState("配网成功", "success");
      } else if (code === 0x010A) {
        setStatusState("配网失败", "error");
      } else if (rawHex) {
        setStatusState(`设备状态: ${name} (${hex}) RAW[${rawHex}]`, "info");
      } else {
        setStatusState(`设备状态: ${name} (${hex})`, "info");
      }

      if (rawHex) {
        log(`状态通知: ${name} (${hex}) RAW[${rawHex}]`);
      } else {
        log(`状态通知: ${name} (${hex})`);
      }
    });
  } catch (err) {
    log(`事件监听失败: ${err}`);
    return false;
  }
  log("事件监听已注册");
  return true;
};

window.addEventListener("DOMContentLoaded", () => {
  const { invoke } = getTauriApi();
  if (!invoke) {
    log("未检测到 Tauri 环境，请不要用浏览器直接打开 dist/index.html");
  }
  attachListener();
  document.getElementById("scan").addEventListener("click", scanDevices);
  document.getElementById("disconnect").addEventListener("click", disconnectDevice);
  document.getElementById("back").addEventListener("click", () => {
    showScanPage();
  });
  document.getElementById("configure").addEventListener("click", configureWifi);
  setConnected("");
  showScanPage();
  scanDevices();
});
