use std::time::Duration;

use btleplug::api::{Central, CharPropFlags, Manager as _, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::StreamExt;
use serde::Serialize;
use tauri::{AppHandle, Manager as _, State};
use tokio::sync::Mutex;
use uuid::Uuid;

const SERVICE_UUID: Uuid = Uuid::from_u128(0x0000e402_0000_1000_8000_00805f9b34fb);
const WRITE_UUID: Uuid = Uuid::from_u128(0x0000e403_0000_1000_8000_00805f9b34fb);
const STATUS_UUID: Uuid = Uuid::from_u128(0x0000e404_0000_1000_8000_00805f9b34fb);

const PREFIX_ID: u16 = 0x03e4;
const FIRST_PACKET_DATA_MAX: usize = 11; // 20 - 3 - 6
const NEXT_PACKET_DATA_MAX: usize = 17; // 20 - 3

#[derive(Default)]
struct AppState {
    adapter: Mutex<Option<Adapter>>,
    peripheral: Mutex<Option<Peripheral>>,
}

#[derive(Serialize)]
struct DeviceInfo {
    id: String,
    name: String,
    rssi: Option<i16>,
    matched: bool,
}

#[derive(Clone, Serialize)]
struct StatusEvent {
    code: u16,
    name: String,
    hex: String,
}

fn status_name(code: u16) -> &'static str {
    match code {
        0x0100 => "READY",
        0x0101 => "START",
        0x0102 => "INPROCESS",
        0x0103 => "CERT_READY",
        0x0104 => "SUCCESS",
        0x0105 => "REBOOTING",
        0x0106 => "IDLE",
        0x0107 => "SSID",
        0x0108 => "PWD",
        0x0109 => "CERT_ERR",
        0x010A => "ERROR",
        _ => "UNKNOWN",
    }
}

fn contains_marker(data: &[u8]) -> bool {
    data.windows(2).any(|w| w == [0xAB, 0x0A])
}

fn matches_device_name(name: &str) -> bool {
    name.to_ascii_lowercase().contains("netcfg")
}

async fn ensure_adapter(state: &AppState) -> Result<Adapter, String> {
    if let Some(adapter) = state.adapter.lock().await.clone() {
        return Ok(adapter);
    }

    let manager = Manager::new().await.map_err(|e| e.to_string())?;
    let adapters = manager.adapters().await.map_err(|e| e.to_string())?;
    let adapter = adapters
        .into_iter()
        .next()
        .ok_or_else(|| "No BLE adapters found".to_string())?;

    *state.adapter.lock().await = Some(adapter.clone());
    Ok(adapter)
}

async fn get_connected_peripheral(state: &AppState) -> Result<Peripheral, String> {
    let peripheral = state
        .peripheral
        .lock()
        .await
        .clone()
        .ok_or_else(|| "No device connected".to_string())?;

    if peripheral.is_connected().await.map_err(|e| e.to_string())? {
        Ok(peripheral)
    } else {
        Err("Device is not connected".to_string())
    }
}

fn split_payload(data: &[u8]) -> Vec<Vec<u8>> {
    if data.is_empty() {
        return vec![Vec::new()];
    }

    let mut packets = Vec::new();
    if data.len() <= FIRST_PACKET_DATA_MAX {
        packets.push(data.to_vec());
        return packets;
    }

    packets.push(data[..FIRST_PACKET_DATA_MAX].to_vec());
    let mut offset = FIRST_PACKET_DATA_MAX;
    while offset < data.len() {
        let end = (offset + NEXT_PACKET_DATA_MAX).min(data.len());
        packets.push(data[offset..end].to_vec());
        offset = end;
    }

    packets
}

fn build_packets(opcode: u16, data: &[u8]) -> Vec<Vec<u8>> {
    let data_len = data.len() as u16;
    let chunks = split_payload(data);
    let raw_count = chunks.len() as u8;

    chunks
        .into_iter()
        .enumerate()
        .map(|(idx, chunk)| {
            let mut packet = Vec::new();
            let raw_index = (idx + 1) as u8;
            if idx == 0 {
                let raw_length = (6 + chunk.len()) as u8;
                packet.extend([raw_index, raw_count, raw_length]);
                packet.extend(PREFIX_ID.to_le_bytes());
                packet.extend(opcode.to_le_bytes());
                packet.extend(data_len.to_le_bytes());
                packet.extend(chunk);
            } else {
                let raw_length = chunk.len() as u8;
                packet.extend([raw_index, raw_count, raw_length]);
                packet.extend(chunk);
            }
            packet
        })
        .collect()
}

async fn write_packets(
    peripheral: &Peripheral,
    write_char: &btleplug::api::Characteristic,
    write_type: WriteType,
    packets: Vec<Vec<u8>>,
) -> Result<(), String> {
    for packet in packets {
        let mut last_err = None;
        for _ in 0..=2 {
            match peripheral
                .write(write_char, &packet, write_type)
                .await
            {
                Ok(()) => {
                    last_err = None;
                    break;
                }
                Err(err) => {
                    last_err = Some(err.to_string());
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
        if let Some(err) = last_err {
            return Err(err);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    Ok(())
}

async fn find_characteristics(
    peripheral: &Peripheral,
) -> Result<(btleplug::api::Characteristic, btleplug::api::Characteristic), String> {
    let chars = peripheral.characteristics();
    let write_char = chars
        .iter()
        .find(|c| {
            c.uuid == WRITE_UUID
                && (c.properties.contains(CharPropFlags::WRITE)
                    || c.properties.contains(CharPropFlags::WRITE_WITHOUT_RESPONSE))
        })
        .cloned()
        .ok_or_else(|| "Write characteristic not found".to_string())?;

    let status_char = chars
        .iter()
        .find(|c| {
            c.uuid == STATUS_UUID
                && (c.properties.contains(CharPropFlags::NOTIFY)
                    || c.properties.contains(CharPropFlags::INDICATE)
                    || c.properties.contains(CharPropFlags::READ))
        })
        .cloned()
        .ok_or_else(|| "Status characteristic not found".to_string())?;

    Ok((write_char, status_char))
}

async fn listen_status_notifications(app: AppHandle, peripheral: Peripheral) {
    let mut stream = match peripheral.notifications().await {
        Ok(stream) => stream,
        Err(err) => {
            let _ = app.emit_all(
                "netcfg_status",
                StatusEvent {
                    code: 0,
                    name: "NOTIFY_ERROR".to_string(),
                    hex: err.to_string(),
                },
            );
            return;
        }
    };

    while let Some(notification) = stream.next().await {
        if notification.uuid != STATUS_UUID {
            continue;
        }
        if notification.value.len() < 2 {
            continue;
        }
        let code = u16::from_le_bytes([notification.value[0], notification.value[1]]);
        let event = StatusEvent {
            code,
            name: status_name(code).to_string(),
            hex: format!("0x{code:04X}"),
        };
        let _ = app.emit_all("netcfg_status", event);
    }
}

#[tauri::command]
async fn scan_devices(
    state: State<'_, AppState>,
    timeout_ms: Option<u64>,
) -> Result<Vec<DeviceInfo>, String> {
    let adapter = ensure_adapter(&state).await?;
    adapter
        .start_scan(ScanFilter::default())
        .await
        .map_err(|e| e.to_string())?;

    let wait_ms = timeout_ms.unwrap_or(3000);
    tokio::time::sleep(Duration::from_millis(wait_ms)).await;

    let peripherals = adapter.peripherals().await.map_err(|e| e.to_string())?;
    let mut devices = Vec::new();

    for peripheral in peripherals {
        let props = peripheral.properties().await.map_err(|e| e.to_string())?;
        if let Some(props) = props {
            let name = props.local_name.unwrap_or_else(|| "Unknown".to_string());
            let mut matched = matches_device_name(&name);

            if !matched {
                if props
                    .manufacturer_data
                    .values()
                    .any(|data| contains_marker(data))
                {
                    matched = true;
                }
            }

            if !matched {
                if props
                    .service_data
                    .values()
                    .any(|data| contains_marker(data))
                {
                    matched = true;
                }
            }

            devices.push(DeviceInfo {
                id: peripheral.id().to_string(),
                name,
                rssi: props.rssi,
                matched,
            });
        }
    }

    let _ = adapter.stop_scan().await;
    devices.sort_by(|a, b| b.rssi.cmp(&a.rssi));
    Ok(devices)
}

#[tauri::command]
async fn connect_device(state: State<'_, AppState>, app: AppHandle, id: String) -> Result<(), String> {
    let adapter = ensure_adapter(&state).await?;
    let peripherals = adapter.peripherals().await.map_err(|e| e.to_string())?;
    let peripheral = peripherals
        .into_iter()
        .find(|p| p.id().to_string() == id)
        .ok_or_else(|| "Device not found".to_string())?;

    if !peripheral.is_connected().await.map_err(|e| e.to_string())? {
        let mut last_err = None;
        for _ in 0..3 {
            match peripheral.connect().await {
                Ok(()) => {
                    last_err = None;
                    break;
                }
                Err(err) => {
                    last_err = Some(err.to_string());
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
        if let Some(err) = last_err {
            return Err(err);
        }
    }
    peripheral
        .discover_services()
        .await
        .map_err(|e| e.to_string())?;
    tokio::time::sleep(Duration::from_millis(200)).await;

    if !peripheral
        .services()
        .iter()
        .any(|service| service.uuid == SERVICE_UUID)
    {
        return Err("NETCFG_BLE service not found".to_string());
    }

    let (_write_char, status_char) = find_characteristics(&peripheral).await?;
    if status_char
        .properties
        .contains(CharPropFlags::NOTIFY)
        || status_char.properties.contains(CharPropFlags::INDICATE)
    {
        peripheral
            .subscribe(&status_char)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        let _ = app.emit_all(
            "netcfg_status",
            StatusEvent {
                code: 0,
                name: "STATUS_CHAR_NO_NOTIFY".to_string(),
                hex: "0x0000".to_string(),
            },
        );
    }

    *state.peripheral.lock().await = Some(peripheral.clone());
    tauri::async_runtime::spawn(listen_status_notifications(app, peripheral));

    Ok(())
}

#[tauri::command]
async fn disconnect_device(state: State<'_, AppState>) -> Result<(), String> {
    let peripheral = get_connected_peripheral(&state).await?;
    peripheral
        .disconnect()
        .await
        .map_err(|e| e.to_string())?;
    *state.peripheral.lock().await = None;
    Ok(())
}

#[tauri::command]
async fn send_start(state: State<'_, AppState>) -> Result<(), String> {
    send_opcode(state.inner(), 0xA001, &[]).await
}

#[tauri::command]
async fn send_ssid(state: State<'_, AppState>, ssid: String) -> Result<(), String> {
    let bytes = ssid.as_bytes();
    if bytes.len() > 36 {
        return Err("SSID length exceeds 36 bytes".to_string());
    }
    send_opcode(state.inner(), 0xA002, bytes).await
}

#[tauri::command]
async fn send_password(state: State<'_, AppState>, password: String) -> Result<(), String> {
    let bytes = password.as_bytes();
    if bytes.len() > 64 {
        return Err("Password length exceeds 64 bytes".to_string());
    }
    send_opcode(state.inner(), 0xA003, bytes).await
}

#[tauri::command]
async fn send_done(state: State<'_, AppState>) -> Result<(), String> {
    send_opcode(state.inner(), 0xA010, &[]).await
}

#[tauri::command]
async fn send_reboot(state: State<'_, AppState>) -> Result<(), String> {
    send_opcode(state.inner(), 0xA011, &[]).await
}

#[tauri::command]
async fn configure_wifi(
    state: State<'_, AppState>,
    ssid: String,
    password: String,
) -> Result<(), String> {
    let ssid_bytes = ssid.as_bytes();
    if ssid_bytes.len() > 36 {
        return Err("SSID length exceeds 36 bytes".to_string());
    }
    let pwd_bytes = password.as_bytes();
    if pwd_bytes.len() > 64 {
        return Err("Password length exceeds 64 bytes".to_string());
    }

    let state_ref = state.inner();
    send_opcode(state_ref, 0xA001, &[]).await?;
    send_opcode(state_ref, 0xA002, ssid_bytes).await?;
    send_opcode(state_ref, 0xA003, pwd_bytes).await?;
    send_opcode(state_ref, 0xA010, &[]).await?;
    Ok(())
}

async fn send_opcode(state: &AppState, opcode: u16, data: &[u8]) -> Result<(), String> {
    let peripheral = get_connected_peripheral(state).await?;
    let (write_char, _status_char) = find_characteristics(&peripheral).await?;
    let packets = build_packets(opcode, data);
    let write_type = if write_char
        .properties
        .contains(CharPropFlags::WRITE_WITHOUT_RESPONSE)
    {
        WriteType::WithoutResponse
    } else {
        WriteType::WithResponse
    };
    write_packets(&peripheral, &write_char, write_type, packets).await
}

fn main() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            scan_devices,
            connect_device,
            disconnect_device,
            configure_wifi,
            send_start,
            send_ssid,
            send_password,
            send_done,
            send_reboot
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
