//! Lưu/tải cấu hình vào %APPDATA%\VnKey\config.json

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

/// Bảo vệ debounce: khi đã lên lịch lưu, bỏ qua các lần trùng.
static SAVE_PENDING: Mutex<bool> = Mutex::new(false);

fn config_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("VnKey"))
}

fn config_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("config.json"))
}

/// Tải cài đặt đã lưu và áp dụng vào EngineState + ConvSettings.
/// Gọi một lần khi khởi động, sau khi ENGINE đã khởi tạo.
pub fn load() {
    let path = match config_path() {
        Some(p) => p,
        None => return,
    };
    let data = match fs::read_to_string(&path) {
        Ok(d) => d,
        Err(_) => return, // lần chạy đầu — chưa có tệp
    };

    // Parser JSON tối giản (không dùng serde) — cặp key:value phẳng
    let vals = parse_json(&data);

    // Áp dụng vào EngineState
    if let Ok(mut guard) = crate::ENGINE.lock() {
        if let Some(state) = guard.as_mut() {
            if let Some(v) = get_i32(&vals, "input_method") {
                state.set_input_method(v);
            }
            if let Some(v) = get_i32(&vals, "output_charset") {
                state.output_charset = v;
            }
            if let Some(v) = get_bool(&vals, "viet_mode") {
                state.viet_mode = v;
                state.engine.set_viet_mode(v);
            }
            if let Some(v) = get_bool(&vals, "spell_check") {
                state.spell_check = v;
            }
            if let Some(v) = get_bool(&vals, "free_marking") {
                state.free_marking = v;
            }
            if let Some(v) = get_bool(&vals, "modern_style") {
                state.modern_style = v;
            }
            state.sync_options();
        }
    }

    // Cài đặt chạy quyền Admin
    if let Some(v) = get_bool(&vals, "run_as_admin") {
        crate::tray::set_run_as_admin(v);
    }

    // Áp dụng vào ConvSettings
    if let Ok(mut conv) = crate::converter::CONV_SETTINGS.lock() {
        if let Some(v) = get_i32(&vals, "conv_from") {
            conv.from_charset = v as usize;
        }
        if let Some(v) = get_i32(&vals, "conv_to") {
            conv.to_charset = v as usize;
        }
        // Trường cũ — cũng tải vào cài đặt hotkey mới
        if let Some(v) = get_u32(&vals, "conv_hotkey_vk") {
            conv.hotkey_vk = v;
        }
        if let Some(v) = get_u32(&vals, "conv_hotkey_mod") {
            conv.hotkey_modifiers = v;
        }
    }

    // Áp dụng cài đặt hotkey
    if let Ok(mut hk) = crate::hotkey::HOTKEY_SETTINGS.lock() {
        if let Some(v) = get_u32(&vals, "toggle_vk") {
            hk.toggle_vk = v;
        }
        if let Some(v) = get_u32(&vals, "toggle_mods") {
            hk.toggle_mods = v;
        }
        // Hotkey chuyển mã: ưu tiên trường mới, fallback trường cũ
        if let Some(v) = get_u32(&vals, "hk_conv_vk") {
            hk.conv_vk = v;
        } else if let Some(v) = get_u32(&vals, "conv_hotkey_vk") {
            hk.conv_vk = v;
        }
        if let Some(v) = get_u32(&vals, "hk_conv_mods") {
            hk.conv_mods = v;
        } else if let Some(v) = get_u32(&vals, "conv_hotkey_mod") {
            hk.conv_mods = v;
        }
    }

    // Áp dụng custom hotkeys (mới) hoặc cs_hotkeys (cũ)
    for (k, v) in &vals {
        if k == "custom_hotkeys" || k == "cs_hotkeys" {
            use crate::hotkey::{CustomHotkey, HkKind};
            if let Ok(mut hk) = crate::hotkey::HOTKEY_SETTINGS.lock() {
                hk.custom.clear();
                for chunk in v.split('{').skip(1) {
                    if let Some(end) = chunk.find('}') {
                        let inner = &chunk[..end];
                        let mut kind_str = "";
                        let mut id = 0i32;
                        let mut vk = 0u32;
                        let mut mods = 0u32;
                        for part in inner.split(',') {
                            let part = part.trim();
                            if let Some(colon) = part.find(':') {
                                let key = part[..colon].trim().trim_matches('"');
                                let val_s = part[colon+1..].trim().trim_matches('"');
                                match key {
                                    "k" => { kind_str = if val_s == "im" { "im" } else { "cs" }; }
                                    "cs" | "id" => { id = val_s.parse().unwrap_or(0); }
                                    "vk" => { vk = val_s.parse().unwrap_or(0); }
                                    "mods" => { mods = val_s.parse().unwrap_or(0); }
                                    _ => {}
                                }
                            }
                        }
                        let kind = if kind_str == "im" { HkKind::Im } else { HkKind::Cs };
                        // Old format (cs_hotkeys) only had entries with vk!=0; new format keeps all
                        hk.custom.push(CustomHotkey { kind, id, vk, mods });
                    }
                }
            }
            break; // prefer first match (custom_hotkeys over cs_hotkeys)
        }
    }

    // Áp dụng danh sách đen
    if let Ok(mut list) = crate::blacklist::BLACKLIST.lock() {
        list.clear();
        for (k, v) in &vals {
            if k == "blacklist" {
                // value là chuỗi mảng JSON kiểu ["app1.exe","app2.exe"]
                for item in parse_string_array(v) {
                    if !item.is_empty() {
                        list.push(item);
                    }
                }
            }
        }
    }

    // Áp dụng bảng mã theo ứng dụng
    for (k, v) in &vals {
        if k == "app_charsets" {
            crate::app_charset::from_json(v);
        }
    }
}

/// Lưu cài đặt hiện tại ra đĩa. Có debounce — tạo thread ngắn
/// để thay đổi nhanh (nhiều click menu) chỉ ghi một lần.
pub fn save() {
    {
        let mut pending = match SAVE_PENDING.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
        if *pending {
            return;
        }
        *pending = true;
    }

    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(200));
        {
            if let Ok(mut p) = SAVE_PENDING.lock() { *p = false; }
        }
        do_save();
    });
}

/// Lưu đồng bộ ngay lập tức (không debounce). Dùng trước khi relaunch.
pub fn save_now() {
    do_save();
}

fn do_save() {
    let path = match config_path() {
        Some(p) => p,
        None => return,
    };

    // Đọc trạng thái engine hiện tại
    let (im, cs, vm, spell, free, modern) = {
        match crate::ENGINE.lock() {
            Ok(guard) => match guard.as_ref() {
                Some(s) => (
                    s.input_method,
                    s.output_charset,
                    s.viet_mode,
                    s.spell_check,
                    s.free_marking,
                    s.modern_style,
                ),
                None => return,
            },
            Err(_) => return,
        }
    };

    // Đọc cài đặt chạy quyền Admin
    let run_as_admin = crate::tray::get_run_as_admin();

    // Đọc cài đặt chuyển mã
    let (conv_from, conv_to) = {
        match crate::converter::CONV_SETTINGS.lock() {
            Ok(c) => (c.from_charset, c.to_charset),
            Err(_) => (20, 1),
        }
    };

    // Đọc cài đặt hotkey
    let (toggle_vk, toggle_mods, hk_conv_vk, hk_conv_mods) = {
        match crate::hotkey::HOTKEY_SETTINGS.lock() {
            Ok(hk) => (hk.toggle_vk, hk.toggle_mods, hk.conv_vk, hk.conv_mods),
            Err(_) => (0u32, 0u32, 0u32, 0u32),
        }
    };

    // Đọc custom hotkeys
    let custom_hk_json = {
        match crate::hotkey::HOTKEY_SETTINGS.lock() {
            Ok(hk) => {
                use crate::hotkey::HkKind;
                let items: Vec<String> = hk.custom.iter()
                    .map(|ch| {
                        let kind = match ch.kind { HkKind::Cs => "cs", HkKind::Im => "im" };
                        format!("{{\"k\":\"{kind}\",\"id\":{},\"vk\":{},\"mods\":{}}}", ch.id, ch.vk, ch.mods)
                    })
                    .collect();
                format!("[{}]", items.join(","))
            }
            Err(_) => "[]".to_string(),
        }
    };

    // Đọc danh sách đen
    let blacklist_json = {
        match crate::blacklist::BLACKLIST.lock() {
            Ok(list) => {
                let items: Vec<String> = list.iter().map(|s| format!("\"{s}\"")).collect();
                format!("[{}]", items.join(", "))
            }
            Err(_) => "[]".to_string(),
        }
    };
    // Đọc bảng mã theo ứng dụng
    let app_charsets_json = crate::app_charset::to_json();
    let json = format!(
        "{{\n\
         \x20 \"input_method\": {im},\n\
         \x20 \"output_charset\": {cs},\n\
         \x20 \"viet_mode\": {vm},\n\
         \x20 \"spell_check\": {spell},\n\
         \x20 \"free_marking\": {free},\n\
         \x20 \"modern_style\": {modern},\n\
         \x20 \"run_as_admin\": {run_as_admin},\n\
         \x20 \"conv_from\": {conv_from},\n\
         \x20 \"conv_to\": {conv_to},\n\
         \x20 \"toggle_vk\": {toggle_vk},\n\
         \x20 \"toggle_mods\": {toggle_mods},\n\
         \x20 \"hk_conv_vk\": {hk_conv_vk},\n\
         \x20 \"hk_conv_mods\": {hk_conv_mods},\n\
         \x20 \"custom_hotkeys\": {custom_hk_json},\n\
         \x20 \"blacklist\": {blacklist_json},\n\
         \x20 \"app_charsets\": {app_charsets_json}\n\
         }}\n"
    );

    if let Some(dir) = config_dir() {
        let _ = fs::create_dir_all(&dir);
    }
    let _ = fs::write(&path, json);
}

// ── Trợ giúp JSON tối giản (không cần serde) ──

fn parse_json(s: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for line in s.lines() {
        let line = line.trim().trim_end_matches(',');
        if let Some(colon) = line.find(':') {
            let key = line[..colon].trim().trim_matches('"');
            let val = line[colon + 1..].trim().trim_matches('"');
            if !key.is_empty() {
                result.push((key.to_string(), val.to_string()));
            }
        }
    }
    result
}

fn get_i32(vals: &[(String, String)], key: &str) -> Option<i32> {
    vals.iter()
        .find(|(k, _)| k == key)
        .and_then(|(_, v)| v.parse().ok())
}

fn get_u32(vals: &[(String, String)], key: &str) -> Option<u32> {
    vals.iter()
        .find(|(k, _)| k == key)
        .and_then(|(_, v)| v.parse().ok())
}

fn get_bool(vals: &[(String, String)], key: &str) -> Option<bool> {
    vals.iter()
        .find(|(k, _)| k == key)
        .and_then(|(_, v)| v.parse().ok())
}

/// Phân tích mảng JSON chuỗi kiểu `["a.exe", "b.exe"]`
fn parse_string_array(s: &str) -> Vec<String> {
    let s = s.trim();
    let s = s.strip_prefix('[').unwrap_or(s);
    let s = s.strip_suffix(']').unwrap_or(s);
    s.split(',')
        .map(|item| item.trim().trim_matches('"').to_string())
        .filter(|item| !item.is_empty())
        .collect()
}
