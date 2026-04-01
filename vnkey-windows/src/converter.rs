//! Cửa sổ chuyển đổi bảng mã — tao + wry.
//! Chức năng chuyển mã clipboard (gọi từ WM_HOTKEY) vẫn giữ nguyên.

use crate::webview::{self, UiEvent};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};

use vnkey_engine::charset::Charset;
use windows::Win32::Foundation::*;
use windows::Win32::System::DataExchange::*;
use windows::Win32::System::Memory::*;

#[derive(Debug)]
pub struct ConvSettings {
    pub from_charset: usize,
    pub to_charset: usize,
    pub hotkey_vk: u32,
    pub hotkey_modifiers: u32,
}

impl Default for ConvSettings {
    fn default() -> Self {
        Self { from_charset: 0, to_charset: 6, hotkey_vk: 0, hotkey_modifiers: 0 }
    }
}

pub static CONV_SETTINGS: LazyLock<Mutex<ConvSettings>> =
    LazyLock::new(|| Mutex::new(ConvSettings::default()));

static CONV_OPEN: AtomicBool = AtomicBool::new(false);

const CS_IDS: [i32; 11] = [0, 1, 2, 3, 5, 10, 20, 21, 22, 40, 43];
const CS_LABELS: [&str; 11] = [
    "Unicode", "UTF-8", "NCR Decimal", "NCR Hex", "CP-1258",
    "VIQR", "TCVN3 (ABC)", "VPS", "VISCII", "VNI Windows", "VNI Mac",
];

fn charset_from_id(id: i32) -> Charset {
    match id {
        0 => Charset::Unicode, 1 => Charset::Utf8,
        2 => Charset::NcrDec, 3 => Charset::NcrHex,
        5 => Charset::WinCP1258, 10 => Charset::Viqr,
        20 => Charset::Tcvn3, 21 => Charset::Vps,
        22 => Charset::Viscii, 40 => Charset::VniWin,
        43 => Charset::VniMac, _ => Charset::Unicode,
    }
}

// ── Chuyển mã clipboard (gọi từ WM_HOTKEY) ──────────────────────────────

pub fn convert_clipboard() {
    let (from_idx, to_idx) = {
        let s = match CONV_SETTINGS.lock() { Ok(s) => s, Err(_) => return };
        (s.from_charset, s.to_charset)
    };
    let from_id = CS_IDS.get(from_idx).copied().unwrap_or(0);
    let to_id = CS_IDS.get(to_idx).copied().unwrap_or(0);
    if from_id == to_id { return; }
    let from_cs = charset_from_id(from_id);
    let to_cs = charset_from_id(to_id);
    let from_name = CS_LABELS.get(from_idx).unwrap_or(&"?");
    let to_name = CS_LABELS.get(to_idx).unwrap_or(&"?");
    if let Some(text) = get_clipboard_text() {
        if let Some(converted) = do_convert(&text, from_cs, to_cs) {
            set_clipboard_text(&converted);
            crate::osd::show(&format!("✔ Clipboard: {from_name} → {to_name}"));
        } else {
            crate::osd::show(&format!("✘ Lỗi chuyển mã {from_name} → {to_name}"));
        }
    }
}

fn do_convert(text: &str, from: Charset, to: Charset) -> Option<String> {
    let (src_bytes, actual_from) = match from {
        Charset::Unicode | Charset::Utf8 => (text.as_bytes().to_vec(), Charset::Utf8),
        Charset::NcrDec | Charset::NcrHex | Charset::Viqr => (text.as_bytes().to_vec(), from),
        _ => (text.chars().map(|c| (c as u32) as u8).collect(), from),
    };
    let result = vnkey_engine::charset::convert(&src_bytes, actual_from, to).ok()?;
    match to {
        Charset::Unicode => {
            let mut u16buf = Vec::with_capacity(result.len() / 2);
            let mut i = 0;
            while i + 1 < result.len() {
                u16buf.push(u16::from_le_bytes([result[i], result[i + 1]]));
                i += 2;
            }
            Some(String::from_utf16_lossy(&u16buf))
        }
        Charset::Utf8 | Charset::NcrDec | Charset::NcrHex | Charset::Viqr =>
            String::from_utf8(result).ok(),
        _ => Some(result.iter().map(|&b| b as char).collect()),
    }
}

// ── Clipboard helpers ────────────────────────────────────────────────────

const CF_UNICODETEXT: u32 = 13;

fn get_clipboard_text() -> Option<String> {
    unsafe {
        OpenClipboard(HWND::default()).ok()?;
        let result = (|| {
            let handle = GetClipboardData(CF_UNICODETEXT).ok()?;
            let hmem: HGLOBAL = HGLOBAL(handle.0 as _);
            let ptr = GlobalLock(hmem) as *const u16;
            if ptr.is_null() { return None; }
            let mut len = 0usize;
            while *ptr.add(len) != 0 { len += 1; }
            let slice = std::slice::from_raw_parts(ptr, len);
            let s = String::from_utf16_lossy(slice);
            let _ = GlobalUnlock(hmem);
            Some(s)
        })();
        let _ = CloseClipboard();
        result
    }
}

fn set_clipboard_text(text: &str) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let byte_len = wide.len() * 2;
    unsafe {
        if OpenClipboard(HWND::default()).is_err() { return; }
        let _ = EmptyClipboard();
        if let Ok(hmem) = GlobalAlloc(GMEM_MOVEABLE, byte_len) {
            let ptr = GlobalLock(hmem) as *mut u16;
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
                let _ = GlobalUnlock(hmem);
                let _ = SetClipboardData(CF_UNICODETEXT, HANDLE(hmem.0 as _));
            }
        }
        let _ = CloseClipboard();
    }
}

// ── Cửa sổ WebView ──────────────────────────────────────────────────────

pub fn open_converter_window() {
    if CONV_OPEN.swap(true, Ordering::SeqCst) { return; }
    std::thread::spawn(|| {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_converter()));
        if let Err(e) = r { eprintln!("[converter] panic: {e:?}"); }
        CONV_OPEN.store(false, Ordering::Relaxed);
    });
}

fn build_html() -> String {
    let (from_idx, to_idx) = {
        let s = CONV_SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
        (s.from_charset, s.to_charset)
    };

    let opts = |sel: usize| -> String {
        CS_LABELS.iter().enumerate().map(|(i, n)|
            if i == sel { format!(r#"<option value="{i}" selected>{n}</option>"#) }
            else { format!(r#"<option value="{i}">{n}</option>"#) }
        ).collect()
    };

    let body = format!(r##"
<div class="container" style="height:100vh;display:flex;flex-direction:column">
  <div style="display:flex;align-items:center;gap:8px;margin-bottom:2px">
    <div style="width:28px;height:28px;background:var(--accent);border-radius:6px;display:flex;align-items:center;justify-content:center;color:#fff;font-size:14px">⇄</div>
    <div style="font-weight:600;font-size:14px">Chuyển mã</div>
  </div>

  <div style="display:flex;gap:8px;align-items:end">
    <div class="group" style="flex:1">
      <div class="group-title">Nguồn</div>
      <select id="from" onchange="cmd({{cmd:'from',v:+this.value}})">{from_opts}</select>
    </div>
    <button onclick="cmd({{cmd:'swap'}})" title="Đổi chiều" style="margin-bottom:10px;font-size:16px;padding:6px 10px">⇄</button>
    <div class="group" style="flex:1">
      <div class="group-title">Đích</div>
      <select id="to" onchange="cmd({{cmd:'to',v:+this.value}})">{to_opts}</select>
    </div>
  </div>

  <div class="group" style="flex:1;display:flex;flex-direction:column">
    <div class="group-title">Văn bản</div>
    <textarea id="text" style="flex:1;min-height:0;resize:none" placeholder="Nhập hoặc dán văn bản cần chuyển mã…"></textarea>
  </div>

  <div style="display:flex;gap:8px">
    <button class="primary full" onclick="cmd({{cmd:'conv_text',text:document.getElementById('text').value}})">Chuyển văn bản</button>
    <button class="full" onclick="cmd({{cmd:'conv_clip'}})">Chuyển clipboard</button>
    <button onclick="cmd({{cmd:'close'}})">Đóng</button>
  </div>

  <div id="status" class="status"></div>
</div>
"##,
        from_opts = opts(from_idx),
        to_opts = opts(to_idx),
    );

    webview::html(&body, "
function setStatus(msg, ok) {
    var el = document.getElementById('status');
    el.textContent = msg;
    el.className = 'status ' + (ok ? 'ok' : 'err');
}
function setText(t) { document.getElementById('text').value = t; }
function setFrom(i) { document.getElementById('from').value = i; }
function setTo(i) { document.getElementById('to').value = i; }
")
}

fn handle_ipc(body: &str, proxy: tao::event_loop::EventLoopProxy<UiEvent>) {
    let msg: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v, Err(_) => return,
    };
    let cmd = msg["cmd"].as_str().unwrap_or("");
    match cmd {
        "from" => {
            let v = msg["v"].as_i64().unwrap_or(0) as usize;
            if let Ok(mut s) = CONV_SETTINGS.lock() { s.from_charset = v; }
            crate::config::save();
        }
        "to" => {
            let v = msg["v"].as_i64().unwrap_or(0) as usize;
            if let Ok(mut s) = CONV_SETTINGS.lock() { s.to_charset = v; }
            crate::config::save();
        }
        "swap" => {
            let (f, t) = {
                let mut s = CONV_SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
                let f = s.from_charset;
                let t = s.to_charset;
                s.from_charset = t;
                s.to_charset = f;
                (t, f)
            };
            crate::config::save();
            let _ = proxy.send_event(UiEvent::Eval(
                format!("setFrom({});setTo({})", f, t)));
        }
        "conv_text" => {
            let text = msg["text"].as_str().unwrap_or("");
            if text.is_empty() {
                let _ = proxy.send_event(UiEvent::Eval("setStatus('Chưa nhập văn bản',false)".into()));
                return;
            }
            let (from_idx, to_idx) = {
                let s = CONV_SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
                (s.from_charset, s.to_charset)
            };
            let from_id = CS_IDS.get(from_idx).copied().unwrap_or(0);
            let to_id = CS_IDS.get(to_idx).copied().unwrap_or(0);
            if from_id == to_id {
                let _ = proxy.send_event(UiEvent::Eval("setStatus('Bảng mã nguồn và đích giống nhau',false)".into()));
                return;
            }
            match do_convert(text, charset_from_id(from_id), charset_from_id(to_id)) {
                Some(result) => {
                    let escaped = result.replace('\\', "\\\\").replace('\'', "\\'").replace('\n', "\\n").replace('\r', "");
                    let from_n = CS_LABELS.get(from_idx).unwrap_or(&"?");
                    let to_n = CS_LABELS.get(to_idx).unwrap_or(&"?");
                    let _ = proxy.send_event(UiEvent::Eval(format!(
                        "setText('{}');setStatus('✔ Đã chuyển {} → {}',true)", escaped, from_n, to_n)));
                }
                None => {
                    let _ = proxy.send_event(UiEvent::Eval("setStatus('✘ Lỗi chuyển mã',false)".into()));
                }
            }
        }
        "conv_clip" => {
            let (from_idx, to_idx) = {
                let s = CONV_SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
                (s.from_charset, s.to_charset)
            };
            let from_id = CS_IDS.get(from_idx).copied().unwrap_or(0);
            let to_id = CS_IDS.get(to_idx).copied().unwrap_or(0);
            if from_id == to_id {
                let _ = proxy.send_event(UiEvent::Eval("setStatus('Bảng mã nguồn và đích giống nhau',false)".into()));
                return;
            }
            match get_clipboard_text() {
                Some(text) => {
                    match do_convert(&text, charset_from_id(from_id), charset_from_id(to_id)) {
                        Some(result) => {
                            set_clipboard_text(&result);
                            let f = CS_LABELS.get(from_idx).unwrap_or(&"?");
                            let t = CS_LABELS.get(to_idx).unwrap_or(&"?");
                            let _ = proxy.send_event(UiEvent::Eval(format!(
                                "setStatus('✔ Clipboard: {} → {}',true)", f, t)));
                        }
                        None => { let _ = proxy.send_event(UiEvent::Eval("setStatus('✘ Lỗi chuyển mã clipboard',false)".into())); }
                    }
                }
                None => { let _ = proxy.send_event(UiEvent::Eval("setStatus('✘ Clipboard trống',false)".into())); }
            }
        }
        "close" => { let _ = proxy.send_event(UiEvent::Close); }
        _ => {}
    }
}

fn run_converter() {
    let html = build_html();
    webview::run_webview("Chuyển mã – VnKey", 500.0, 440.0, &html, handle_ipc);
}
