# VnKey — Bộ gõ tiếng Việt đa nền tảng

[![License: GPL v3](https://img.shields.io/badge/license-GPL--3.0-blue)](LICENSE)

VnKey là bộ gõ tiếng Việt mã nguồn mở, engine viết bằng **Rust**, hỗ trợ **Windows**, **macOS**, **Linux (Fcitx5)** và **Linux (IBus)**.

**Website:** [https://vnkey.app](https://vnkey.app)

## Kiến trúc

```
vnkey-engine/    (Rust)    Core engine + C FFI (staticlib)
vnkey-windows/   (Rust)    Ứng dụng Windows — Win32 + Direct2D
vnkey-macos/     (Obj-C)   Input method cho macOS — Input Method Kit
vnkey-fcitx5/    (C++)     Fcitx5 addon cho Linux
vnkey-ibus/      (C)       IBus engine cho Linux
```

## Tính năng

- **Kiểu gõ:** Telex, Simple Telex, VNI, VIQR (Windows có thêm MS Vietnamese)
- **Bảng mã:** 15 bảng mã — Unicode UTF-8, TCVN3, VNI Windows, VISCII, VPS, VIQR, NCR, CP-1258, …
- **Kiểm tra chính tả** — tự phục hồi ký tự ASCII nếu từ không hợp lệ
- **Bỏ dấu tự do** — đặt dấu thanh ở vị trí tuỳ ý
- **Kiểu mới** (oà, uý) — theo quy tắc chính tả hiện đại
- **Chuyển đổi bảng mã clipboard** — chuyển text giữa Unicode và các bảng mã legacy
- **Cấu hình chung** `config.json` — dùng chung trên mọi nền tảng

## Cài đặt

### Windows

Tải `vnkey.exe` từ [trang phát hành](https://github.com/marixdev/vnkey/releases) — chạy trực tiếp, không cần cài đặt.

Hoặc build từ source:
```powershell
cd vnkey-windows
cargo build --release
# → target\release\vnkey.exe
```

### macOS

Yêu cầu: macOS 11.0+, Xcode Command Line Tools, Rust toolchain, CMake.

```bash
cd vnkey-macos
./build.sh install
```

Sau khi cài:
1. Vào **System Settings → Keyboard → Input Sources → Edit…**
2. Nhấn **+** → tìm "Vietnamese" → chọn **VnKey**
3. Tick "Show Input menu in menu bar"

Hoặc build universal binary (Intel + Apple Silicon):
```bash
./build.sh universal
# → build/VnKey.app
```

### Linux — Fcitx5

```bash
# Debian/Ubuntu
sudo dpkg -i vnkey-fcitx5_1.0.1_amd64.deb

# Fedora/RHEL/Rocky
sudo dnf install ./vnkey-fcitx5-1.0.1-1.x86_64.rpm

# Arch Linux
sudo pacman -U vnkey-fcitx5-1.0.1-1-x86_64.pkg.tar.zst
```

#### NixOS (Fcitx5)

Thêm vào `flake.nix` của hệ thống:

```nix
{
  inputs.vnkey.url = "github:marixdev/vnkey";

  # Trong nixosConfigurations:
  modules = [
    ({ pkgs, ... }: {
      i18n.inputMethod = {
        enable = true;
        type = "fcitx5";
        fcitx5.addons = [
          vnkey.packages.${pkgs.system}.vnkey-fcitx5
        ];
      };
    })
  ];
}
```

Hoặc dùng `nix profile` (không cần NixOS):
```bash
nix profile install github:marixdev/vnkey#vnkey-fcitx5
```

### Linux — IBus

```bash
# Debian/Ubuntu
sudo dpkg -i vnkey-ibus_1.0.1_amd64.deb

# Fedora/RHEL/Rocky
sudo dnf install ./vnkey-ibus-1.0.1-1.x86_64.rpm

# Arch Linux
sudo pacman -U vnkey-ibus-1.0.1-1-x86_64.pkg.tar.zst
```

#### NixOS (IBus)

Thêm vào `flake.nix` của hệ thống:

```nix
{
  inputs.vnkey.url = "github:marixdev/vnkey";

  modules = [
    ({ pkgs, ... }: {
      i18n.inputMethod = {
        enable = true;
        type = "ibus";
        ibus.engines = [
          vnkey.packages.${pkgs.system}.vnkey-ibus
        ];
      };
    })
  ];
}
```

Hoặc dùng `nix profile`:
```bash
nix profile install github:marixdev/vnkey#vnkey-ibus
```

### Build từ source (Linux)

```bash
# Yêu cầu: Rust toolchain, CMake ≥ 3.16, GCC

# Build engine
cd vnkey-engine && cargo build --release && cd ..

# Fcitx5 addon
cd vnkey-fcitx5
bash scripts/build.sh package    # → .deb hoặc .rpm

# IBus engine
cd vnkey-ibus
mkdir build && cd build
cmake .. -DCMAKE_INSTALL_PREFIX=/usr -DCMAKE_BUILD_TYPE=Release
make -j$(nproc)
cpack -G DEB    # hoặc cpack -G RPM
```

## Sử dụng

### Windows

| Thao tác | Cách thực hiện |
|----------|---------------|
| Chuyển Việt/Anh | `Ctrl+Shift` hoặc click trái icon tray |
| Mở menu | Click phải icon tray |
| Mở cấu hình | Double-click icon tray |

### macOS

| Thao tác | Cách thực hiện |
|----------|---------------|
| Chuyển Việt/Anh | Click icon menu bar → chọn VnKey hoặc dùng phím tắt input source |
| Tùy chỉnh | Click icon menu bar → VnKey → "Tùy chỉnh..." |
| Chọn kiểu gõ | Click icon menu bar → VnKey → chọn Telex/VNI/... |

### Linux (Fcitx5 / IBus)

| Thao tác | Cách thực hiện |
|----------|---------------|
| Chuyển Việt/Anh | `Ctrl+Space` |
| Chọn kiểu gõ / bảng mã | Click phải icon tray |

## Cấu trúc dự án

```
vnkey-engine/              Rust engine — core xử lý tiếng Việt
  src/
    engine.rs              Engine chính (process key, backspace, reset)
    input.rs               Định nghĩa kiểu gõ (Telex, VNI, …)
    vnlexi.rs              Xử lý ngữ pháp tiếng Việt (âm, vần, thanh)
    charset/               Chuyển đổi bảng mã
    ffi.rs                 C FFI layer
  tests/integration.rs     Test cases

vnkey-windows/             Ứng dụng Windows
  src/
    main.rs                Entry point, message pump
    hook.rs                WH_KEYBOARD_LL keyboard hook
    send.rs                SendInput (backspace + Unicode text)
    tray.rs                System tray icon + context menu
    gui.rs                 Cửa sổ cấu hình (Win32 + Direct2D)
    osd.rs                 OSD toast notification
    info.rs                Cửa sổ giới thiệu
    config.rs              Đọc/ghi config.json
    converter.rs           Chuyển đổi bảng mã clipboard
    blacklist.rs           Loại trừ ứng dụng
    hotkey.rs              Gán phím tắt
    ui.rs                  Direct2D helpers

vnkey-macos/               Input method macOS
  src/
    main.m                 Entry point, IMKServer
    VnKeyInputController.m IMKInputController — xử lý phím
    VnKeyAppDelegate.m     App delegate, preferences window
    VnKeyPreferences.m     UI cài đặt
  resources/Info.plist     Bundle config (IMKit)
  build.sh                 Build script

vnkey-fcitx5/              Fcitx5 addon (Linux)
  src/vnkey-fcitx5.cpp     Engine implementation

vnkey-ibus/                IBus engine (Linux)
  src/vnkey-ibus.c         Engine implementation
```

## Test

```bash
cd vnkey-engine
cargo test
```

## Giấy phép

Toàn bộ dự án được phát hành theo giấy phép **GPL-3.0-or-later**.

## Lời cảm ơn

Dự án lấy cảm hứng từ [Unikey](https://www.unikey.org/) của Phạm Kim Long và các bộ gõ tiếng Việt khác.
