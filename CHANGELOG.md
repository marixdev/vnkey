# Changelog

## 1.0.2 — 2026-04-01

### vnkey-engine
- Thêm module `app_charset`: hỗ trợ cấu hình bảng mã riêng theo từng ứng dụng (per-app charset override)
- Thêm FFI: `vnkey_app_charset_from_json`, `vnkey_app_charset_to_json`, `vnkey_app_charset_update`, `vnkey_app_charset_get_current`
- Thêm `backspaces_bytes` trong `ProcessResult` — tính số backspace theo byte cho bảng mã đa byte (VNI-Win, VNI-Mac, BKHCM2, VietWare-X)
- Thêm `snapshot_output()` + `get_backspaces_for_multi_byte()` để tính chính xác số byte cần xoá trong bảng mã 2-byte
- Sửa `auto_non_vn_restore` không restore khi từ chỉ toàn phụ âm (ví dụ: "đc", "đk" là viết tắt phổ biến, không nên revert thành "ddc", "ddk")
- Sửa thứ tự `VS_UY` trong `VC_PAIR_LIST` (vnlexi)
- Sửa test `test_telex_w_standalone` fail trên NixOS — cập nhật kỳ vọng: `w` → `ư`, `ww` → `w` đúng theo Telex chuẩn (#14)
- Cải thiện `process_telex_w`: khi đã có nguyên âm, luôn trả kết quả `process_hook` thay vì fall-through tạo `ư` sai (#14)
- Sửa `process_telex_w`: thêm NonVn guard — sau khi `auto_non_vn_restore` revert từ về NonVn, phím `w` tiếp theo không tạo `ư` mà pass-through (ví dụ: "windows" không còn bị thành "windoư")
- Cập nhật FFI: `vnkey_process`, `vnkey_backspace`, `vnkey_engine_process`, `vnkey_engine_backspace` thêm tham số `backspaces_bytes`

### vnkey-windows
- Thiết kế lại toàn bộ giao diện cấu hình: chuyển từ win32 + direct2d sang tao + wry (WebView2), phong cách WinUI với inline SVG icons
- Cửa sổ chính: bố cục 2 cột cân đối, icon SVG chuyên nghiệp cho các nút công cụ (loại trừ, chuyển mã, phím tắt, bảng mã ứng dụng)
- Cửa sổ phím tắt: thiết kế lại hoàn toàn — hỗ trợ gán phím tắt cho cả **bảng mã** và **kiểu gõ**, giao diện thêm mới bằng dropdown thay vì liệt kê tất cả
- Cửa sổ loại trừ ứng dụng: thiết kế lại với icon SVG, nút Đóng inline
- Cửa sổ chuyển đổi bảng mã: thiết kế lại, nút Đóng inline cùng hàng nút hành động
- Cửa sổ bảng mã theo ứng dụng: chuyển hoàn toàn từ WinAPI sang WebView2
- Cửa sổ giới thiệu: thêm nút Website (vnkey.app) và GitHub (marixdev/vnkey)
- Sửa hiện tượng flash trắng khi mở cửa sổ — ẩn window cho đến khi WebView2 render xong (hidden-until-ready)
- Tắt nút maximize trên tất cả cửa sổ cấu hình
- Hiển thị icon ứng dụng trên thanh tiêu đề (WM_SETICON từ Win32 resource)
- Tạo mới v.ico (chữ V nền xanh) và e.ico (chữ E nền đỏ) chuyên nghiệp
- Dọn dẹp file không sử dụng (file Slint cũ, backup, SVG rời)
- Sửa Ctrl+Shift+N/V kích hoạt chuyển ngôn ngữ — defer toggle đến keyup, huỷ nếu có phím khác xen giữa (#15)
- Sửa xung đột với Kanata (keyboard remapping) — phím không xử lý dùng `CallNextHookEx` thay vì block + reinject qua `SendInput` (#13)
- Sửa gõ tiếng Việt trên thanh tìm kiếm/địa chỉ của LibreWolf, Waterfox, Mullvad Browser — thêm vào danh sách VK_BACK (Shift+Left không hoạt động trên các trình duyệt Firefox-based)

### vnkey-fcitx5
- **Chuyển sang chế độ commit trực tiếp** (không dùng preedit/gạch chân) — sửa lỗi hiển thị sai trên nhiều ứng dụng GTK/Qt
- Dùng `deleteSurroundingText` thay vì xoá ký tự từ preedit buffer khi backspace
- Thêm `directCommit()`: commit trực tiếp với chuyển mã bảng mã đích + per-app charset override
- Thiết kế lại menu tray: tách thành submenu riêng cho Kiểu gõ, Bảng mã, toggle Spell/Free/Modern, Clipboard — thay vì một menu phẳng
- Thêm visual bullet (●) cho radio items trong submenu (workaround GNOME Shell AppIndicator)
- Thêm `syncActiveIC()` + `settingsGen_` để đồng bộ cài đặt ngay lập tức khi đổi từ menu (không cần đợi focus_in)
- Hỗ trợ per-app charset: tự phát hiện app đang focus, áp dụng bảng mã riêng cho từng ứng dụng
- Lưu/nạp `app_charsets` trong config JSON
- Thêm `Enabled=True`, `OnDemand=True` trong addon.conf
- postinst: tự động restart fcitx5 sau cài đặt (`fcitx5-remote -r`)
- Thêm glibc compat stubs: `pidfd_getpid`, `pidfd_spawnp` (GLIBC_2.39) cho tương thích glibc cũ
- Cập nhật icon: dùng icon mới thống nhất với Windows (chữ V nền xanh #0067C0)

### vnkey-ibus
- **Chuyển sang chế độ commit trực tiếp** (không dùng preedit/gạch chân) — sửa lỗi hiển thị sai trên nhiều ứng dụng GTK
- Dùng `ibus_engine_delete_surrounding_text` thay vì xoá ký tự từ preedit buffer khi backspace
- Thêm `direct_commit()`: commit trực tiếp với chuyển mã bảng mã đích + per-app charset override
- Hỗ trợ per-app charset: phát hiện app bằng `xdotool getactivewindow getwindowpid` + `/proc/PID/exe`, áp dụng bảng mã riêng
- Lưu/nạp `app_charsets` trong config JSON
- Cập nhật icon: dùng icon mới thống nhất với Windows (chữ V nền xanh #0067C0)
- Cập nhật FFI header: thêm tham số `backspaces_bytes` (truyền NULL — macOS dùng UTF-8)
- Thêm icon VnKey.png + tự động tạo VnKey.icns khi build (thiết kế thống nhất với Windows)

---

## 1.0.1 — 2026-03-29

### vnkey-engine
- Sửa lỗi UB (undefined behavior) do transmute không kiểm tra biên — thay bằng `from_u8`/`from_i16` an toàn
- Sửa sentinel -1 trong buffer — thêm `debug_assert` kiểm tra bounds
- Implement `auto_non_vn_restore`: tự khôi phục phím gốc khi từ không phải tiếng Việt (vd: gõ "services" không còn bị thành "sẻvices")
- Thêm `soft_reset()` public + FFI: lưu trạng thái để backspace sau dấu cách có thể khôi phục dấu
- Xóa method `buf_mut` không sử dụng

### vnkey-windows
- Sửa blocking mutex: `ENGINE.lock()` → `ENGINE.try_lock()` trong keyboard hook
- Sửa hardcoded bàn phím US — dùng `ToUnicode` API hỗ trợ mọi layout
- Sửa phím tắt Win+D không hoạt động — thêm xử lý VK_LWIN/VK_RWIN
- Sửa Facebook chat/comment không nhận dấu khi click lần đầu — thêm `GetGUIThreadInfo` theo dõi focus
- Sửa WPS Office hiện ký tự đôi ("chàoao") — thêm phương thức backspace riêng cho ứng dụng không hỗ trợ Shift+Left
- Trích xuất `build_backspace_inputs()` helper giảm code trùng lặp
- Space dùng `soft_reset` thay vì `reset` để hỗ trợ backspace khôi phục dấu
- Sửa phím tắt tùy chỉnh (Alt+Z, ...) gây mất focus khi đang soạn thảo — xử lý toggle trực tiếp trong LL hook thay vì RegisterHotKey
- Thêm thông báo OSD (Tiếng Việt / English) khi chuyển chế độ bằng Ctrl+Shift mặc định
- Cài đặt lại keyboard hook định kỳ (5s) phòng trường hợp Windows tự gỡ hook
- Sửa lỗi gõ "đc" (viết tắt "được") thành "ddc" — debounce focus element change trong cùng cửa sổ khi đang gõ, tránh engine bị reset sai bởi autocomplete popup

### vnkey-fcitx5
- Sửa `saveConfig` dùng `std::system("mkdir -p")` — thay bằng `std::filesystem::create_directories()`
- Space dùng `vnkey_engine_soft_reset` thay vì `vnkey_engine_reset`

### vnkey-ibus
- Space dùng `vnkey_engine_soft_reset` thay vì `vnkey_engine_reset`

### Chung
- Thêm `flake.nix` hỗ trợ NixOS (Fcitx5 & IBus)
- Cập nhật README hướng dẫn cài đặt NixOS chi tiết

### vnkey-macos (MỚI)
- Phiên bản macOS đầu tiên — sử dụng Input Method Kit (IMKit)
- Hỗ trợ macOS 11.0+ (Big Sur trở lên), cả Intel và Apple Silicon
- Preedit với gạch chân, commit tại ranh giới từ
- Menu bar: chuyển Việt/Anh, chọn kiểu gõ, mở cài đặt
- Cửa sổ Preferences (kiểu gõ, kiểm tra chính tả, bỏ dấu tự do, kiểu mới)
- Cài đặt lưu qua NSUserDefaults
- Build script hỗ trợ universal binary (lipo)

---

## 1.0.0 — 2026-03-29

Phiên bản đầu tiên phát hành công khai.

### vnkey-engine (Rust core)
- 4 kiểu gõ: Telex, Simple Telex, VNI, VIQR
- 15 bảng mã: Unicode UTF-8, TCVN3, VNI Windows, VISCII, VPS, VIQR, NCR, CP-1258, …
- Kiểm tra chính tả tự động
- Bỏ dấu tự do / theo quy tắc
- Kiểu mới (oà, uý)
- Chuyển đổi bảng mã (charset_from_utf8 / charset_to_utf8)
- C FFI (staticlib) cho tích hợp đa ngôn ngữ

### vnkey-windows
- Giao diện Win32 native + Direct2D
- System tray icon với menu đầy đủ
- OSD toast khi chuyển Việt/Anh
- Cửa sổ cấu hình (kiểu gõ, bảng mã, tuỳ chọn)
- Công cụ chuyển đổi bảng mã clipboard
- Loại trừ ứng dụng (blacklist)
- Gán phím tắt tuỳ chỉnh
- Khởi động cùng Windows
- Cửa sổ giới thiệu với link clickable

### vnkey-fcitx5
- Fcitx5 input method addon cho Linux
- Preedit với underline
- Menu chuột phải: kiểu gõ, bảng mã, tuỳ chọn
- Chuyển đổi bảng mã clipboard (wl-paste/xclip/xsel)
- Gói .deb (Debian 12+, Ubuntu 22.04+), .rpm (Fedora 41+), .pkg.tar.zst (Arch Linux), .tar.gz (NixOS)
- GLIBC compat shims cho tương thích rộng (glibc 2.34+)

### vnkey-ibus
- IBus input method engine cho Linux
- Preedit với underline
- Property menu: kiểu gõ, bảng mã, tuỳ chọn
- Chuyển đổi bảng mã clipboard
- Gói .deb (Debian 12+, Ubuntu 22.04+), .rpm (Fedora 41+), .pkg.tar.zst (Arch Linux), .tar.gz (NixOS)
- GLIBC compat shims cho tương thích rộng (glibc 2.34+)
