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

## Flatpak 打包

先在宿主机编译二进制，再用 flatpak-builder 打包（不在沙盒内编译 Rust）。

### Arch Linux

安装依赖：

```bash
sudo pacman -S flatpak flatpak-builder appstream-glib
```

添加 Flathub（用户级）：

```bash
flatpak remote-add --if-not-exists --user flathub https://flathub.org/repo/flathub.flatpakrepo
```

编译并打包：

```bash
cargo build --release --manifest-path src-tauri/Cargo.toml

flatpak-builder --user --disable-rofiles-fuse --force-clean \
  --install-deps-from=flathub \
  --repo=repo \
  build-dir flatpak/com.arcs.ble.netcfg.yml

flatpak build-bundle \
  --runtime-repo=https://flathub.org/repo/flathub.flatpakrepo \
  repo arcs-ble-net-cfg.flatpak com.arcs.ble.netcfg
```

安装运行：

```bash
flatpak install --user ./arcs-ble-net-cfg.flatpak
flatpak run com.arcs.ble.netcfg
```

### Ubuntu / Debian

安装依赖：

```bash
sudo apt update
sudo apt install -y flatpak flatpak-builder appstream-util
```

添加 Flathub（用户级）：

```bash
flatpak remote-add --if-not-exists --user flathub https://flathub.org/repo/flathub.flatpakrepo
```

编译并打包：

```bash
cargo build --release --manifest-path src-tauri/Cargo.toml

flatpak-builder --user --disable-rofiles-fuse --force-clean \
  --install-deps-from=flathub \
  --repo=repo \
  build-dir flatpak/com.arcs.ble.netcfg.yml

flatpak build-bundle \
  --runtime-repo=https://flathub.org/repo/flathub.flatpakrepo \
  repo arcs-ble-net-cfg.flatpak com.arcs.ble.netcfg
```

安装运行：

```bash
flatpak install --user ./arcs-ble-net-cfg.flatpak
flatpak run com.arcs.ble.netcfg
```
