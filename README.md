# arcs_ble_net_cfg_rust

基于 Tauri v2 的 BLE 配网工具，前端使用 HTML/JavaScript。

## 运行

```bash
cd src-tauri
cargo tauri dev
```

## 说明

- 使用 `btleplug` 扫描/连接 BLE 设备。
- 启动后自动扫描，选择设备连接后进入配网页面。
- 状态区显示配网结果（SUCCESS 视为成功）。
