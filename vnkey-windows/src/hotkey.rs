//! Cài đặt và gán phím tắt — tao + wry.

use crate::webview::{self, UiEvent};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

// ── Cài đặt phím tắt (toàn cục) ────────────────────────────────────────────

/// Loại phím tắt tùy chỉnh: "cs" = bảng mã, "im" = kiểu gõ
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HkKind { Cs, Im }

/// Phím tắt tùy chỉnh: (kind, id, vk, mods)
#[derive(Debug, Clone, Copy)]
pub struct CustomHotkey {
    pub kind: HkKind,
    pub id: i32,
    pub vk: u32,
    pub mods: u32,
}

#[derive(Debug)]
pub struct HotkeySettings {
    pub toggle_vk: u32,
    pub toggle_mods: u32,
    pub conv_vk: u32,
    pub conv_mods: u32,
    pub custom: Vec<CustomHotkey>,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self { toggle_vk: 0, toggle_mods: 0, conv_vk: 0, conv_mods: 0, custom: Vec::new() }
    }
}

pub static HOTKEY_SETTINGS: LazyLock<Mutex<HotkeySettings>> =
    LazyLock::new(|| Mutex::new(HotkeySettings::default()));

pub const HOTKEY_ID_TOGGLE: i32 = 9000;
pub const HOTKEY_ID_CONVERT: i32 = 9001;
pub const HOTKEY_ID_CUSTOM_BASE: i32 = 9100;

/// Trả về custom hotkey nếu hotkey id thuộc custom range
pub fn custom_hotkey_for_id(hk_id: i32) -> Option<CustomHotkey> {
    if hk_id < HOTKEY_ID_CUSTOM_BASE { return None; }
    let idx = (hk_id - HOTKEY_ID_CUSTOM_BASE) as usize;
    let hk = HOTKEY_SETTINGS.lock().ok()?;
    hk.custom.get(idx).copied()
}

// ── Hiển thị ─────────────────────────────────────────────────────────────

pub fn hotkey_display_text(vk: u32, mods: u32) -> String {
    if vk == 0 && mods == 0 { return "(chưa đặt)".into(); }
    let mut s = String::new();
    if mods & 2 != 0 { s.push_str("Ctrl+"); }
    if mods & 1 != 0 { s.push_str("Alt+"); }
    if mods & 4 != 0 { s.push_str("Shift+"); }
    if vk == 0 {
        if s.ends_with('+') { s.truncate(s.len() - 1); }
        return s;
    }
    let key_name = match vk {
        0x41..=0x5A => format!("{}", (vk as u8) as char),
        0x30..=0x39 => format!("{}", (vk as u8 - 0x30)),
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        0x20 => "Space".into(),
        0x1B => "Esc".into(),
        0x0D => "Enter".into(),
        0x09 => "Tab".into(),
        _ => format!("0x{:02X}", vk),
    };
    s.push_str(&key_name);
    s
}

fn toggle_display(hk: &HotkeySettings) -> String {
    if hk.toggle_vk == 0 && hk.toggle_mods == 0 {
        "Ctrl+Shift (mặc định)".into()
    } else {
        hotkey_display_text(hk.toggle_vk, hk.toggle_mods)
    }
}

// ── Đăng ký/hủy đăng ký ─────────────────────────────────────────────────

pub fn register_hotkeys(hwnd: HWND) {
    if let Ok(hk) = HOTKEY_SETTINGS.lock() {
        unsafe {
            if hk.conv_vk != 0 {
                if let Err(e) = RegisterHotKey(hwnd, HOTKEY_ID_CONVERT,
                    HOT_KEY_MODIFIERS(hk.conv_mods), hk.conv_vk) {
                    eprintln!("[hotkey] RegisterHotKey conv FAILED: {e}");
                }
            }
            for (i, ch) in hk.custom.iter().enumerate() {
                if ch.vk != 0 {
                    let id = HOTKEY_ID_CUSTOM_BASE + i as i32;
                    if let Err(e) = RegisterHotKey(hwnd, id,
                        HOT_KEY_MODIFIERS(ch.mods), ch.vk) {
                        eprintln!("[hotkey] RegisterHotKey custom[{i}] FAILED: {e}");
                    }
                }
            }
        }
    }
}

pub fn unregister_hotkeys(hwnd: HWND) {
    unsafe {
        let _ = UnregisterHotKey(hwnd, HOTKEY_ID_TOGGLE);
        let _ = UnregisterHotKey(hwnd, HOTKEY_ID_CONVERT);
        for i in 0..30 {
            let _ = UnregisterHotKey(hwnd, HOTKEY_ID_CUSTOM_BASE + i);
        }
    }
}

fn reregister_hotkeys() {
    unsafe {
        match FindWindowW(w!("VnKeyHiddenWindow"), w!("VnKey")) {
            Ok(mw) => { let _ = PostMessageW(mw, crate::WM_VNKEY_REREGISTER_HOTKEYS, WPARAM(0), LPARAM(0)); }
            Err(e) => { eprintln!("[hotkey] FindWindowW FAILED: {e}"); }
        }
    }
}

pub fn is_toggle_builtin() -> bool {
    match HOTKEY_SETTINGS.lock() {
        Ok(hk) => hk.toggle_vk == 0 && hk.toggle_mods == 0,
        Err(_) => true,
    }
}

// ── JS keyCode → VK code ────────────────────────────────────────────────

fn js_code_to_vk(code: &str) -> u32 {
    match code {
        "KeyA" => 0x41, "KeyB" => 0x42, "KeyC" => 0x43, "KeyD" => 0x44,
        "KeyE" => 0x45, "KeyF" => 0x46, "KeyG" => 0x47, "KeyH" => 0x48,
        "KeyI" => 0x49, "KeyJ" => 0x4A, "KeyK" => 0x4B, "KeyL" => 0x4C,
        "KeyM" => 0x4D, "KeyN" => 0x4E, "KeyO" => 0x4F, "KeyP" => 0x50,
        "KeyQ" => 0x51, "KeyR" => 0x52, "KeyS" => 0x53, "KeyT" => 0x54,
        "KeyU" => 0x55, "KeyV" => 0x56, "KeyW" => 0x57, "KeyX" => 0x58,
        "KeyY" => 0x59, "KeyZ" => 0x5A,
        "Digit0" => 0x30, "Digit1" => 0x31, "Digit2" => 0x32, "Digit3" => 0x33,
        "Digit4" => 0x34, "Digit5" => 0x35, "Digit6" => 0x36, "Digit7" => 0x37,
        "Digit8" => 0x38, "Digit9" => 0x39,
        "F1" => 0x70, "F2" => 0x71, "F3" => 0x72, "F4" => 0x73,
        "F5" => 0x74, "F6" => 0x75, "F7" => 0x76, "F8" => 0x77,
        "F9" => 0x78, "F10" => 0x79, "F11" => 0x7A, "F12" => 0x7B,
        "Space" => 0x20, "Tab" => 0x09, "Enter" => 0x0D,
        "Escape" => 0x1B, "Backspace" => 0x08, "Delete" => 0x2E,
        "Insert" => 0x2D, "Home" => 0x24, "End" => 0x23,
        "PageUp" => 0x21, "PageDown" => 0x22,
        "ArrowUp" => 0x26, "ArrowDown" => 0x28,
        "ArrowLeft" => 0x25, "ArrowRight" => 0x27,
        _ => 0,
    }
}

// ── Tên bảng mã / kiểu gõ ────────────────────────────────────────────────

const CS_IDS: [i32; 11] = [0, 1, 2, 3, 5, 10, 20, 21, 22, 40, 43];
const CS_NAMES: [&str; 11] = ["Unicode","UTF-8","NCR Decimal","NCR Hex","CP-1258",
    "VIQR","TCVN3 (ABC)","VPS","VISCII","VNI Windows","VNI Mac"];
const IM_NAMES: [&str; 5] = ["Telex","Simple Telex","VNI","VIQR","MS Vietnamese"];

fn custom_label(ch: &CustomHotkey) -> String {
    match ch.kind {
        HkKind::Cs => {
            CS_IDS.iter().zip(CS_NAMES.iter())
                .find(|(&id, _)| id == ch.id)
                .map(|(_, &n)| format!("Bảng mã: {n}"))
                .unwrap_or_else(|| format!("Bảng mã #{}", ch.id))
        }
        HkKind::Im => {
            IM_NAMES.get(ch.id as usize)
                .map(|n| format!("Kiểu gõ: {n}"))
                .unwrap_or_else(|| format!("Kiểu gõ #{}", ch.id))
        }
    }
}

/// JSON cho danh sách custom hotkey hiện tại
fn custom_list_json() -> String {
    let hk = HOTKEY_SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
    let items: Vec<String> = hk.custom.iter().enumerate().map(|(i, ch)| {
        let kind = match ch.kind { HkKind::Cs => "cs", HkKind::Im => "im" };
        let label = custom_label(ch);
        let keys = hotkey_display_text(ch.vk, ch.mods);
        format!(r#"{{"idx":{i},"kind":"{kind}","id":{},"label":"{}","keys":"{}"}}"#, ch.id, label, keys)
    }).collect();
    format!("[{}]", items.join(","))
}

// ── Cửa sổ WebView ──────────────────────────────────────────────────────

static HK_OPEN: AtomicBool = AtomicBool::new(false);

pub fn open_hotkey_window() {
    if HK_OPEN.swap(true, Ordering::SeqCst) { return; }
    std::thread::spawn(|| {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_hotkey()));
        if let Err(e) = r { eprintln!("[hotkey] panic: {e:?}"); }
        HK_OPEN.store(false, Ordering::Relaxed);
    });
}

fn build_html() -> String {
    let (tog_label, conv_label) = {
        let hk = HOTKEY_SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
        (toggle_display(&hk), hotkey_display_text(hk.conv_vk, hk.conv_mods))
    };

    // Build <option> list for add dropdown
    let mut add_opts = String::new();
    add_opts.push_str(r#"<optgroup label="Bảng mã">"#);
    for (&id, &name) in CS_IDS.iter().zip(CS_NAMES.iter()) {
        add_opts.push_str(&format!(r#"<option value="cs:{id}">{name}</option>"#));
    }
    add_opts.push_str("</optgroup>");
    add_opts.push_str(r#"<optgroup label="Kiểu gõ">"#);
    for (i, &name) in IM_NAMES.iter().enumerate() {
        add_opts.push_str(&format!(r#"<option value="im:{i}">{name}</option>"#));
    }
    add_opts.push_str("</optgroup>");

    let init_json = custom_list_json();

    let body = format!(r##"
<div class="container" style="height:100vh;display:flex;flex-direction:column">
  <div style="display:flex;align-items:center;gap:8px;margin-bottom:2px">
    <div style="width:28px;height:28px;background:var(--accent);border-radius:6px;display:flex;align-items:center;justify-content:center">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="4" width="20" height="16" rx="2"/><path d="M6 8h.01M10 8h.01M14 8h.01M18 8h.01M6 12h.01M10 12h.01M14 12h.01M18 12h.01M8 16h8"/></svg>
    </div>
    <div style="font-weight:600;font-size:14px">Gán phím tắt</div>
  </div>

  <div class="group">
    <div class="group-title">Chung</div>
    <div class="hk-row">
      <span class="hk-label">Chuyển Việt/Anh</span>
      <div class="hotkey-box" id="tog-box">{tog}</div>
      <button onclick="startCapture('toggle')">Gán</button>
      <button onclick="clearHotkey('toggle')">Xóa</button>
    </div>
    <div class="hk-row">
      <span class="hk-label">Chuyển mã Clipboard</span>
      <div class="hotkey-box" id="conv-box">{conv}</div>
      <button onclick="startCapture('convert')">Gán</button>
      <button onclick="clearHotkey('convert')">Xóa</button>
    </div>
  </div>

  <div class="group" style="flex:1;display:flex;flex-direction:column;min-height:0">
    <div class="group-title">Phím tắt chuyển nhanh</div>
    <div id="custom-list" style="flex:1;overflow-y:auto"></div>
  </div>

  <div style="display:flex;gap:6px;align-items:center">
    <select id="add-sel" style="flex:1"><option value="">— Chọn bảng mã / kiểu gõ —</option>{add_opts}</select>
    <button onclick="addCustom()">Thêm</button>
  </div>

  <div style="display:flex;align-items:center;gap:8px">
    <div id="hint" class="status" style="flex:1"></div>
    <button onclick="cmd({{cmd:'close'}})">Đóng</button>
  </div>
</div>
"##, tog = tog_label, conv = conv_label, add_opts = add_opts);

    let script = format!(r#"
var capturing = null;
var capIdx = -1;
var items = {init};

function renderList() {{
    var el = document.getElementById('custom-list');
    if (!items.length) {{ el.innerHTML = '<div class="empty">Chưa có phím tắt nào. Chọn bảng mã hoặc kiểu gõ rồi nhấn Thêm.</div>'; return; }}
    el.innerHTML = items.map(function(it, i) {{
        return '<div class="hk-row">'
          + '<span class="hk-label">' + it.label + '</span>'
          + '<div class="hotkey-box" id="cust-' + i + '">' + it.keys + '</div>'
          + '<button onclick="startCapture(\'custom\',' + i + ')">Gán</button>'
          + '<button class="danger" style="padding:3px 8px" onclick="removeCustom(' + i + ')">✕</button>'
          + '</div>';
    }}).join('');
}}

function addCustom() {{
    var sel = document.getElementById('add-sel');
    var v = sel.value;
    if (!v) return;
    cmd({{cmd:'add', val:v}});
    sel.value = '';
}}

function removeCustom(idx) {{
    cmd({{cmd:'remove', idx:idx}});
}}

function startCapture(target, idx) {{
    capturing = target;
    capIdx = idx;
    var box;
    if (target === 'toggle') box = document.getElementById('tog-box');
    else if (target === 'convert') box = document.getElementById('conv-box');
    else box = document.getElementById('cust-' + idx);
    if (box) {{ box.textContent = 'Nhấn phím…'; box.classList.add('capturing'); }}
    document.getElementById('hint').textContent = 'Nhấn tổ hợp phím. Esc để hủy.';
}}

function clearHotkey(target) {{
    cmd({{cmd:'clear', target:target}});
}}

function endCapture() {{
    capturing = null; capIdx = -1;
    document.getElementById('hint').textContent = '';
    document.querySelectorAll('.hotkey-box').forEach(function(b) {{ b.classList.remove('capturing'); }});
}}

function updateCustom(list) {{
    items = list; renderList();
}}

function setBox(id, text) {{
    var el = document.getElementById(id);
    if (el) {{ el.textContent = text; el.classList.remove('capturing'); }}
}}

document.addEventListener('keydown', function(e) {{
    if (!capturing) return;
    e.preventDefault(); e.stopPropagation();
    if (['Control','Alt','Shift','Meta'].indexOf(e.key) >= 0) return;
    cmd({{cmd:'key', target:capturing, idx:capIdx, code:e.code, ctrl:e.ctrlKey, alt:e.altKey, shift:e.shiftKey}});
}});

renderList();
"#, init = init_json);

    webview::html(&body, &script)
}

fn handle_ipc(body: &str, proxy: tao::event_loop::EventLoopProxy<UiEvent>) {
    let msg: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v, Err(_) => return,
    };
    let cmd = msg["cmd"].as_str().unwrap_or("");
    match cmd {
        "add" => {
            // val = "cs:0" or "im:2"
            let val = msg["val"].as_str().unwrap_or("");
            let parts: Vec<&str> = val.splitn(2, ':').collect();
            if parts.len() != 2 { return; }
            let kind = match parts[0] { "cs" => HkKind::Cs, "im" => HkKind::Im, _ => return };
            let id: i32 = parts[1].parse().unwrap_or(-1);
            if id < 0 { return; }
            // Check duplicate
            {
                let hk = HOTKEY_SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
                if hk.custom.iter().any(|c| c.kind == kind && c.id == id) {
                    let _ = proxy.send_event(UiEvent::Eval(
                        "document.getElementById('hint').textContent='Đã có trong danh sách'".into()));
                    return;
                }
            }
            {
                let mut hk = HOTKEY_SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
                hk.custom.push(CustomHotkey { kind, id, vk: 0, mods: 0 });
            }
            crate::config::save();
            let _ = proxy.send_event(UiEvent::Eval(format!("updateCustom({})", custom_list_json())));
        }
        "remove" => {
            let idx = msg["idx"].as_i64().unwrap_or(-1);
            if idx >= 0 {
                let mut hk = HOTKEY_SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
                let i = idx as usize;
                if i < hk.custom.len() { hk.custom.remove(i); }
            }
            reregister_hotkeys();
            crate::config::save();
            let _ = proxy.send_event(UiEvent::Eval(format!("updateCustom({})", custom_list_json())));
        }
        "key" => {
            let target = msg["target"].as_str().unwrap_or("");
            let code = msg["code"].as_str().unwrap_or("");
            let ctrl = msg["ctrl"].as_bool().unwrap_or(false);
            let alt = msg["alt"].as_bool().unwrap_or(false);
            let shift = msg["shift"].as_bool().unwrap_or(false);
            let idx = msg["idx"].as_i64().unwrap_or(-1) as i32;

            let vk = js_code_to_vk(code);
            if vk == 0 { return; }

            if vk == 0x1B {
                let _ = proxy.send_event(UiEvent::Eval("endCapture()".into()));
                return;
            }

            let mut mods = 0u32;
            if ctrl { mods |= 2; }
            if alt { mods |= 1; }
            if shift { mods |= 4; }
            let display = hotkey_display_text(vk, mods);

            match target {
                "toggle" => {
                    if let Ok(mut hk) = HOTKEY_SETTINGS.lock() {
                        hk.toggle_vk = vk; hk.toggle_mods = mods;
                    }
                    let _ = proxy.send_event(UiEvent::Eval(format!(
                        "setBox('tog-box','{}');endCapture()", display)));
                }
                "convert" => {
                    if let Ok(mut hk) = HOTKEY_SETTINGS.lock() {
                        hk.conv_vk = vk; hk.conv_mods = mods;
                    }
                    if let Ok(mut cs) = crate::converter::CONV_SETTINGS.lock() {
                        cs.hotkey_vk = vk; cs.hotkey_modifiers = mods;
                    }
                    let _ = proxy.send_event(UiEvent::Eval(format!(
                        "setBox('conv-box','{}');endCapture()", display)));
                }
                "custom" => {
                    if idx >= 0 {
                        if let Ok(mut hk) = HOTKEY_SETTINGS.lock() {
                            if let Some(ch) = hk.custom.get_mut(idx as usize) {
                                ch.vk = vk; ch.mods = mods;
                            }
                        }
                    }
                    let _ = proxy.send_event(UiEvent::Eval(format!(
                        "updateCustom({});endCapture()", custom_list_json())));
                }
                _ => {}
            }
            reregister_hotkeys();
            crate::config::save();
        }
        "clear" => {
            let target = msg["target"].as_str().unwrap_or("");
            match target {
                "toggle" => {
                    if let Ok(mut hk) = HOTKEY_SETTINGS.lock() {
                        hk.toggle_vk = 0; hk.toggle_mods = 0;
                    }
                    let _ = proxy.send_event(UiEvent::Eval(
                        "setBox('tog-box','Ctrl+Shift (mặc định)')".into()));
                }
                "convert" => {
                    if let Ok(mut hk) = HOTKEY_SETTINGS.lock() {
                        hk.conv_vk = 0; hk.conv_mods = 0;
                    }
                    if let Ok(mut cs) = crate::converter::CONV_SETTINGS.lock() {
                        cs.hotkey_vk = 0; cs.hotkey_modifiers = 0;
                    }
                    let _ = proxy.send_event(UiEvent::Eval(
                        "setBox('conv-box','(chưa đặt)')".into()));
                }
                _ => {}
            }
            reregister_hotkeys();
            crate::config::save();
        }
        "close" => { let _ = proxy.send_event(UiEvent::Close); }
        _ => {}
    }
}

fn run_hotkey() {
    let html = build_html();
    webview::run_webview("Gán phím – VnKey", 480.0, 480.0, &html, handle_ipc);
}
