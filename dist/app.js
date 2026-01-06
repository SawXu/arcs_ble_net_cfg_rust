const tauriApi = window.__TAURI__ || {};
const invoke = tauriApi.tauri?.invoke || tauriApi.invoke;
const listen = tauriApi.event?.listen;

const deviceList = document.getElementById("device-list");
const deviceStatus = document.getElementById("device-status");
const logEl = document.getElementById("log");
const statusEl = document.getElementById("status");

const log = (message) => {
  const ts = new Date().toLocaleTimeString();
  logEl.textContent += `[${ts}] ${message}\n`;
  logEl.scrollTop = logEl.scrollHeight;
};

const setStatus = (message) => {
  statusEl.textContent = message;
};

const setConnected = (name) => {
  deviceStatus.textContent = name ? `已连接: ${name}` : "未连接";
};

const loadDevices = (devices) => {
  deviceList.innerHTML = "";
  if (!devices.length) {
    const option = document.createElement("option");
    option.value = "";
    option.textContent = "未发现设备";
    deviceList.appendChild(option);
    return;
  }
  devices.forEach((device) => {
    const option = document.createElement("option");
    option.value = device.id;
    const tag = device.matched ? "*" : "";
    const rssi = device.rssi === null ? "" : ` RSSI:${device.rssi}`;
    option.textContent = `${tag}${device.name}${rssi}`;
    deviceList.appendChild(option);
  });
};

const scanDevices = async () => {
  log("开始扫描...");
  try {
    if (!invoke) {
      log("Tauri API 未就绪，无法调用扫描");
      return;
    }
    const devices = await invoke("scan_devices", { timeout_ms: 3000 });
    loadDevices(devices);
    log(`扫描完成，发现 ${devices.length} 个设备`);
  } catch (err) {
    log(`扫描失败: ${err}`);
  }
};

const connectDevice = async () => {
  const id = deviceList.value;
  if (!id) {
    log("请选择设备");
    return;
  }
  try {
    if (!invoke) {
      log("Tauri API 未就绪，无法连接设备");
      return;
    }
    await invoke("connect_device", { id });
    setConnected(deviceList.options[deviceList.selectedIndex].textContent);
    log("连接成功");
  } catch (err) {
    log(`连接失败: ${err}`);
  }
};

const disconnectDevice = async () => {
  try {
    if (!invoke) {
      log("Tauri API 未就绪，无法断开设备");
      return;
    }
    await invoke("disconnect_device");
    setConnected("");
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
    if (!invoke) {
      log("Tauri API 未就绪，无法发送配网");
      return;
    }
    await invoke("configure_wifi", { ssid, password });
    log("配网指令已发送");
  } catch (err) {
    log(`配网失败: ${err}`);
  }
};

const sendStart = async () => {
  try {
    if (!invoke) {
      log("Tauri API 未就绪，无法发送 START");
      return;
    }
    await invoke("send_start");
    log("START 已发送");
  } catch (err) {
    log(`START 发送失败: ${err}`);
  }
};

const sendSsid = async () => {
  const ssid = document.getElementById("ssid").value.trim();
  if (!ssid) {
    log("请输入 SSID");
    return;
  }
  try {
    if (!invoke) {
      log("Tauri API 未就绪，无法发送 SSID");
      return;
    }
    await invoke("send_ssid", { ssid });
    log("SSID 已发送");
  } catch (err) {
    log(`SSID 发送失败: ${err}`);
  }
};

const sendPwd = async () => {
  const password = document.getElementById("password").value;
  try {
    if (!invoke) {
      log("Tauri API 未就绪，无法发送 PWD");
      return;
    }
    await invoke("send_password", { password });
    log("PWD 已发送");
  } catch (err) {
    log(`PWD 发送失败: ${err}`);
  }
};

const sendDone = async () => {
  try {
    if (!invoke) {
      log("Tauri API 未就绪，无法发送 DONE");
      return;
    }
    await invoke("send_done");
    log("DONE 已发送");
  } catch (err) {
    log(`DONE 发送失败: ${err}`);
  }
};

const sendReboot = async () => {
  try {
    if (!invoke) {
      log("Tauri API 未就绪，无法发送 REBOOT");
      return;
    }
    await invoke("send_reboot");
    log("REBOOT 已发送");
  } catch (err) {
    log(`REBOOT 发送失败: ${err}`);
  }
};

if (listen) {
  listen("netcfg_status", (event) => {
    const { code, name, hex } = event.payload;
    setStatus(`设备状态: ${name} (${hex})`);
    log(`状态通知: ${name} (${hex})`);
  });
} else {
  log("Tauri 事件系统未就绪，无法接收状态通知");
}

window.addEventListener("DOMContentLoaded", () => {
  if (!invoke) {
    log("未检测到 Tauri 环境，请不要用浏览器直接打开 dist/index.html");
  }
  document.getElementById("scan").addEventListener("click", scanDevices);
  document.getElementById("connect").addEventListener("click", connectDevice);
  document.getElementById("disconnect").addEventListener("click", disconnectDevice);
  document.getElementById("configure").addEventListener("click", configureWifi);
  document.getElementById("start").addEventListener("click", sendStart);
  document.getElementById("send-ssid").addEventListener("click", sendSsid);
  document.getElementById("send-pwd").addEventListener("click", sendPwd);
  document.getElementById("done").addEventListener("click", sendDone);
  document.getElementById("reboot").addEventListener("click", sendReboot);
  setConnected("");
});
