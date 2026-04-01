//! Bảng mã theo ứng dụng — GUI webview + foreground detection.
//! Logic portable nằm trong vnkey_engine::app_charset.

use crate::webview::{self, UiEvent};
use std::sync::atomic::Ordering;

// Re-export từ engine để các module khác không cần đổi import path
pub use vnkey_engine::app_charset::{
    get_current_app_charset, to_json, from_json,
    APP_CHARSETS, CS_NAMES, cs_name,
};

use vnkey_engine::app_charset::CS_IDS;

/// Cập nhật charset override khi chuyển app (gọi từ hook khi foreground thay đổi).
pub fn update_app_charset() {
    let exe = crate::blacklist::get_foreground_exe_cached()
        .map(|e| e.to_ascii_lowercase());
    vnkey_engine::app_charset::update_app_charset_for(exe.as_deref());
}

// ── Cửa sổ WebView ─────────────────────────────────────────────────────

static AC_OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn open_app_charset_window() {
    if AC_OPEN.swap(true, Ordering::SeqCst) { return; }
    std::thread::spawn(|| {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_app_charset()));
        if let Err(e) = r { eprintln!("[appcs] panic: {e:?}"); }
        AC_OPEN.store(false, Ordering::Relaxed);
    });
}

fn list_json() -> String {
    let map = APP_CHARSETS.lock().unwrap_or_else(|e| e.into_inner());
    let mut entries: Vec<_> = map.iter().collect();
    entries.sort_by_key(|(k, _)| (*k).clone());
    let items: Vec<String> = entries.iter()
        .map(|(exe, cs)| format!(r#"{{"exe":"{}","cs":{},"name":"{}"}}"#, exe, cs, cs_name(**cs)))
        .collect();
    format!("[{}]", items.join(","))
}

fn cs_options() -> String {
    CS_IDS.iter().zip(CS_NAMES.iter()).map(|(id, name)|
        format!(r#"<option value="{id}">{name}</option>"#)
    ).collect()
}

fn build_html() -> String {
    let body = format!(r##"
<div class="container" style="height:100vh;display:flex;flex-direction:column">
  <div style="display:flex;align-items:center;gap:8px;margin-bottom:2px">
    <div style="width:28px;height:28px;background:#0f7b0f;border-radius:6px;display:flex;align-items:center;justify-content:center;color:#fff;font-size:13px;font-weight:700">A</div>
    <div style="font-weight:600;font-size:14px">Bảng mã theo ứng dụng</div>
  </div>

  <div class="group" style="flex:1;display:flex;flex-direction:column;min-height:0">
    <div class="group-title">Danh sách</div>
    <div class="list-box" id="listbox" style="flex:1;max-height:none"></div>
  </div>

  <div class="group">
    <div class="group-title">Thêm ứng dụng</div>
    <div style="display:flex;gap:8px;align-items:end">
      <div style="flex:1">
        <div style="font-size:11px;color:var(--text-dim);margin-bottom:3px">Ứng dụng</div>
        <div style="display:flex;gap:6px">
          <input type="text" id="inp" placeholder="tên file .exe" style="flex:1"
                 onkeydown="if(event.key==='Enter')addEntry()">
          <button onclick="cmd({{cmd:'pick'}})" title="Chọn bằng chuột (1.5s)" style="padding:6px 10px;font-size:14px">🎯</button>
        </div>
      </div>
      <div style="width:140px">
        <div style="font-size:11px;color:var(--text-dim);margin-bottom:3px">Bảng mã</div>
        <select id="cs">{cs_opts}</select>
      </div>
      <button class="primary" onclick="addEntry()" style="min-width:52px">Thêm</button>
    </div>
  </div>

  <div style="display:flex;align-items:center;gap:8px">
    <button class="danger" onclick="removeSelected()">Xóa mục chọn</button>
    <div id="status" class="status" style="flex:1"></div>
    <button onclick="cmd({{cmd:'close'}})">Đóng</button>
  </div>
</div>
"##, cs_opts = cs_options());

    let script = format!(r#"
var selected = -1;
function renderList(items) {{
    var box = document.getElementById('listbox');
    if (!items.length) {{ box.innerHTML = '<div class="empty">Chưa có ứng dụng nào</div>'; return; }}
    box.innerHTML = items.map(function(entry, i) {{
        return '<div class="item'+(i===selected?' selected':'')+'" onclick="selectItem('+i+')" style="display:flex;justify-content:space-between">'
            + '<span>' + entry.exe + '</span>'
            + '<span style="color:var(--text-dim);font-size:12px">' + entry.name + '</span>'
            + '</div>';
    }}).join('');
}}
function selectItem(i) {{ selected = (selected === i ? -1 : i); renderList(currentList); }}
var currentList = {init};
renderList(currentList);
function addEntry() {{
    var inp = document.getElementById('inp');
    var cs = document.getElementById('cs');
    var name = inp.value.trim();
    if (!name) return;
    inp.value = '';
    cmd({{cmd:'add', name:name, cs:+cs.value}});
}}
function removeSelected() {{
    if (selected < 0) return;
    cmd({{cmd:'remove', idx:selected}});
    selected = -1;
}}
function updateList(items) {{ currentList = items; renderList(items); }}
function setStatus(msg) {{ document.getElementById('status').textContent = msg; }}
function setInput(v) {{ document.getElementById('inp').value = v; }}
"#, init = list_json());

    webview::html(&body, &script)
}

fn handle_ipc(body: &str, proxy: tao::event_loop::EventLoopProxy<UiEvent>) {
    let msg: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v, Err(_) => return,
    };
    let cmd = msg["cmd"].as_str().unwrap_or("");
    match cmd {
        "add" => {
            let name = msg["name"].as_str().unwrap_or("").trim().to_ascii_lowercase();
            let cs = msg["cs"].as_i64().unwrap_or(0) as i32;
            if name.is_empty() { return; }
            {
                let mut map = APP_CHARSETS.lock().unwrap_or_else(|e| e.into_inner());
                map.insert(name, cs);
            }
            crate::config::save();
            let _ = proxy.send_event(UiEvent::Eval(format!("updateList({})", list_json())));
        }
        "remove" => {
            let idx = msg["idx"].as_i64().unwrap_or(-1);
            if idx >= 0 {
                let map_guard = APP_CHARSETS.lock().unwrap_or_else(|e| e.into_inner());
                let mut entries: Vec<_> = map_guard.keys().cloned().collect();
                entries.sort();
                if let Some(key) = entries.get(idx as usize).cloned() {
                    drop(map_guard);
                    let mut map = APP_CHARSETS.lock().unwrap_or_else(|e| e.into_inner());
                    map.remove(&key);
                }
            }
            crate::config::save();
            let _ = proxy.send_event(UiEvent::Eval(format!("updateList({})", list_json())));
        }
        "pick" => {
            let proxy2 = proxy.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(1500));
                match crate::blacklist::get_exe_under_cursor() {
                    Some(name) => {
                        let escaped = name.replace('\'', "\\'");
                        let _ = proxy2.send_event(UiEvent::Eval(
                            format!("setInput('{}');setStatus('Đã chọn: {}')", escaped, escaped)));
                    }
                    None => {
                        let _ = proxy2.send_event(UiEvent::Eval(
                            "setStatus('Không tìm thấy ứng dụng')".into()));
                    }
                }
            });
        }
        "close" => { let _ = proxy.send_event(UiEvent::Close); }
        _ => {}
    }
}

fn run_app_charset() {
    let html = build_html();
    webview::run_webview("Bảng mã theo ứng dụng – VnKey", 460.0, 480.0, &html, handle_ipc);
}
