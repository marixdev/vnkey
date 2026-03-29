# Changelog

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
