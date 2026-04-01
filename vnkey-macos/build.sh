#!/bin/bash
# Build VnKey.app cho macOS
# Yêu cầu: Xcode Command Line Tools, Rust toolchain, CMake
#
# Sử dụng:
#   ./build.sh              # Build cho architecture hiện tại
#   ./build.sh universal    # Build universal binary (Intel + Apple Silicon)
#   ./build.sh install      # Build và cài vào ~/Library/Input Methods/
#   ./build.sh clean        # Xóa thư mục build

set -euo pipefail
cd "$(dirname "$0")"

ENGINE_DIR="../vnkey-engine"
BUILD_DIR="build"

clean() {
    echo "==> Dọn dẹp..."
    rm -rf "$BUILD_DIR"
    (cd "$ENGINE_DIR" && cargo clean 2>/dev/null || true)
}

build_engine() {
    local target="$1"
    echo "==> Build vnkey-engine ($target)..."
    (cd "$ENGINE_DIR" && cargo build --release --target "$target")
}

build_icon() {
    local icns="resources/VnKey.icns"
    local png="resources/VnKey.png"
    if [[ -f "$icns" ]]; then
        return
    fi
    if ! command -v sips &>/dev/null || ! command -v iconutil &>/dev/null; then
        echo "WARN: sips/iconutil not available, skipping .icns generation"
        return
    fi
    echo "==> Generate VnKey.icns from PNG..."
    local iconset="$BUILD_DIR/VnKey.iconset"
    mkdir -p "$iconset"
    for size in 16 32 128 256 512; do
        sips -z $size $size "$png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
        local double=$((size * 2))
        if [[ $double -le 1024 ]]; then
            sips -z $double $double "$png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
        fi
    done
    iconutil -c icns -o "$icns" "$iconset"
    rm -rf "$iconset"
}

build_app() {
    echo "==> Build VnKey.app..."
    build_icon
    cmake -B "$BUILD_DIR" -DCMAKE_BUILD_TYPE=Release
    cmake --build "$BUILD_DIR" --config Release
}

build_universal() {
    echo "==> Build universal binary..."
    # Build cho cả 2 architecture
    build_engine "aarch64-apple-darwin"
    build_engine "x86_64-apple-darwin"

    # Tạo universal static lib
    echo "==> Tạo universal libvnkey_engine.a..."
    mkdir -p "$ENGINE_DIR/target/release"
    lipo -create \
        "$ENGINE_DIR/target/aarch64-apple-darwin/release/libvnkey_engine.a" \
        "$ENGINE_DIR/target/x86_64-apple-darwin/release/libvnkey_engine.a" \
        -output "$ENGINE_DIR/target/release/libvnkey_engine.a"

    build_app
}

install_app() {
    local dest="$HOME/Library/Input Methods"
    echo "==> Cài đặt vào $dest..."
    mkdir -p "$dest"
    rm -rf "$dest/VnKey.app"
    cp -R "$BUILD_DIR/VnKey.app" "$dest/"
    echo "==> Đã cài đặt. Vào System Preferences > Keyboard > Input Sources để thêm VnKey."
    echo "    Nếu không thấy, thử đăng xuất và đăng nhập lại."
}

case "${1:-}" in
    clean)
        clean
        ;;
    universal)
        build_universal
        echo "==> Hoàn tất: $BUILD_DIR/VnKey.app (universal)"
        ;;
    install)
        if [[ "$(uname -m)" == "arm64" ]]; then
            build_engine "aarch64-apple-darwin"
        else
            build_engine "x86_64-apple-darwin"
        fi
        # Symlink hoặc copy cho CMake tìm
        ARCH_TARGET=""
        if [[ "$(uname -m)" == "arm64" ]]; then
            ARCH_TARGET="aarch64-apple-darwin"
        else
            ARCH_TARGET="x86_64-apple-darwin"
        fi
        mkdir -p "$ENGINE_DIR/target/release"
        cp "$ENGINE_DIR/target/$ARCH_TARGET/release/libvnkey_engine.a" \
           "$ENGINE_DIR/target/release/libvnkey_engine.a"
        build_app
        install_app
        ;;
    *)
        # Build cho architecture hiện tại
        (cd "$ENGINE_DIR" && cargo build --release)
        build_app
        echo "==> Hoàn tất: $BUILD_DIR/VnKey.app"
        ;;
esac
