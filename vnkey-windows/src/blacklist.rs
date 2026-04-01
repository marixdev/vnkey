//! Cửa sổ loại trừ ứng dụng — tao + wry.

use crate::webview::{self, UiEvent};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};

use windows::Win32::Foundation::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::PWSTR;

pub static BLACKLIST: LazyLock<Mutex<Vec<String>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

static BL_OPEN: AtomicBool = AtomicBool::new(false);

pub fn is_foreground_blacklisted() -> bool {
    let exe = match get_foreground_exe() { Some(e) => e, None => return false };
    let list = BLACKLIST.lock().unwrap();
    list.iter().any(|b| b.eq_ignore_ascii_case(&exe))
}

fn get_foreground_exe() -> Option<String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() { return None; }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 { return None; }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).ok()?;
        let _ = CloseHandle(process);
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit('\\').next().map(|s| s.to_string())
    }
}

pub fn get_foreground_exe_cached() -> Option<String> { get_foreground_exe() }

pub fn get_exe_under_cursor() -> Option<String> {
    unsafe {
        let mut pt = POINT::default();
        GetCursorPos(&mut pt).ok()?;
        let hwnd = WindowFromPoint(pt);
        if hwnd.0.is_null() { return None; }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 { return None; }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).ok()?;
        let _ = CloseHandle(process);
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit('\\').next().map(|s| s.to_string())
    }
}

// ── Cửa sổ WebView ──────────────────────────────────────────────────────

pub fn open_blacklist_window() {
    if BL_OPEN.swap(true, Ordering::SeqCst) { return; }
    std::thread::spawn(|| {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_blacklist()));
        if let Err(e) = r { eprintln!("[blacklist] panic: {e:?}"); }
        BL_OPEN.store(false, Ordering::Relaxed);
    });
}

fn list_json() -> String {
    let list = BLACKLIST.lock().unwrap_or_else(|e| e.into_inner());
    serde_json::to_string(&*list).unwrap_or_else(|_| "[]".into())
}

fn build_html() -> String {
    let body = r##"
<div class="container" style="height:100vh;display:flex;flex-direction:column">
  <div style="display:flex;align-items:center;gap:8px;margin-bottom:2px">
    <div style="width:28px;height:28px;background:var(--danger);border-radius:6px;display:flex;align-items:center;justify-content:center">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="4.93" y1="4.93" x2="19.07" y2="19.07"/></svg>
    </div>
    <div style="font-weight:600;font-size:14px">Loại trừ ứng dụng</div>
  </div>

  <div class="group" style="flex:1;display:flex;flex-direction:column;min-height:0">
    <div class="group-title">Danh sách</div>
    <div class="list-box" id="listbox" style="flex:1;max-height:none;overflow-y:auto"></div>
  </div>

  <div style="display:flex;gap:8px;align-items:center">
    <input type="text" id="inp" placeholder="Tên file .exe (vd: notepad.exe)" style="flex:1"
           onkeydown="if(event.key==='Enter')addApp()">
    <button onclick="addApp()" style="min-width:60px">Thêm</button>
  </div>

  <div style="display:flex;gap:8px">
    <button class="full" onclick="cmd({cmd:'pick'})" style="gap:6px">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><circle cx="12" cy="12" r="8"/><line x1="12" y1="2" x2="12" y2="4"/><line x1="12" y1="20" x2="12" y2="22"/><line x1="2" y1="12" x2="4" y2="12"/><line x1="20" y1="12" x2="22" y2="12"/></svg> Chọn bằng chuột
    </button>
    <button class="full danger" onclick="removeSelected()">Xóa mục chọn</button>
    <button onclick="cmd({cmd:'close'})">Đóng</button>
  </div>

  <div id="status" class="status"></div>
</div>
"##;

    let script = format!(r#"
var selected = -1;
function renderList(items) {{
    var box = document.getElementById('listbox');
    if (!items.length) {{ box.innerHTML = '<div class="empty">Chưa có ứng dụng nào</div>'; return; }}
    box.innerHTML = items.map(function(name, i) {{
        return '<div class="item'+(i===selected?' selected':'')+'" onclick="selectItem('+i+')">'+name+'</div>';
    }}).join('');
}}
function selectItem(i) {{ selected = (selected === i ? -1 : i); renderList(currentList); }}
var currentList = {init};
renderList(currentList);
function addApp() {{
    var inp = document.getElementById('inp');
    var name = inp.value.trim();
    if (!name) return;
    inp.value = ''; cmd({{cmd:'add', name:name}});
}}
function removeSelected() {{
    if (selected < 0) return; cmd({{cmd:'remove', idx:selected}});
    selected = -1;
}}
function updateList(items) {{ currentList = items; renderList(items); }}
function setStatus(msg) {{ document.getElementById('status').textContent = msg; }}
"#, init = list_json());

    webview::html(body, &script)
}

fn handle_ipc(body: &str, proxy: tao::event_loop::EventLoopProxy<UiEvent>) {
    let msg: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v, Err(_) => return,
    };
    let cmd = msg["cmd"].as_str().unwrap_or("");
    match cmd {
        "add" => {
            let name = msg["name"].as_str().unwrap_or("").trim().to_lowercase();
            if name.is_empty() { return; }
            {
                let mut list = BLACKLIST.lock().unwrap_or_else(|e| e.into_inner());
                if !list.iter().any(|b| b.eq_ignore_ascii_case(&name)) {
                    list.push(name);
                }
            }
            crate::config::save();
            let _ = proxy.send_event(UiEvent::Eval(format!("updateList({})", list_json())));
        }
        "remove" => {
            let idx = msg["idx"].as_i64().unwrap_or(-1);
            if idx >= 0 {
                let mut list = BLACKLIST.lock().unwrap_or_else(|e| e.into_inner());
                let i = idx as usize;
                if i < list.len() { list.remove(i); }
            }
            crate::config::save();
            let _ = proxy.send_event(UiEvent::Eval(format!("updateList({})", list_json())));
        }
        "pick" => {
            let proxy2 = proxy.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(1500));
                let exe = get_exe_under_cursor();
                match exe {
                    Some(name) => {
                        let lower = name.to_lowercase();
                        {
                            let mut list = BLACKLIST.lock().unwrap_or_else(|e| e.into_inner());
                            if !list.iter().any(|b| b.eq_ignore_ascii_case(&lower)) {
                                list.push(lower.clone());
                            }
                        }
                        crate::config::save();
                        let _ = proxy2.send_event(UiEvent::Eval(format!(
                            "updateList({});setStatus('Đã thêm: {}')", list_json(), lower)));
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

fn run_blacklist() {
    let html = build_html();
    webview::run_webview("Loại trừ ứng dụng – VnKey", 420.0, 440.0, &html, handle_ipc);
}
