#!/usr/bin/env python3
"""
Firefox Multi-Version Vietnamese Input Test
============================================
Tải và test gõ tiếng Việt (VnKey) trên nhiều phiên bản Firefox khác nhau.

Yêu cầu:
    pip install selenium
    (geckodriver sẽ được tải tự động nếu dùng selenium >= 4.6)

Cách dùng:
    python firefox_test.py                    # Test Firefox mặc định trên máy
    python firefox_test.py --versions 115 128 136  # Tải và test các phiên bản cụ thể
    python firefox_test.py --list             # Liệt kê phiên bản có sẵn
    python firefox_test.py --download-only    # Chỉ tải, không test
"""

import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import time
import zipfile
from pathlib import Path
from urllib.request import urlopen, urlretrieve
from urllib.error import URLError

# Thư mục lưu trữ Firefox đã tải
CACHE_DIR = Path(__file__).parent / ".firefox_cache"

# Các test case gõ tiếng Việt (Telex)
# Mỗi tuple: (phím gõ, kết quả mong đợi, mô tả)
TEST_CASES_TELEX = [
    ("Vieejt Nam", "Việt Nam", "Dấu mũ + dấu nặng"),
    ("ddawngr caaps", "đẳng cấp", "dd → đ, aw → ă, dấu hỏi/sắc"),
    ("Xin chaof", "Xin chào", "Dấu huyền"),
    ("thuowjng", "thường", "ow → ươ, dấu nặng"),
    ("nguowif", "người", "oi → ời, dấu huyền"),
    ("hoaf binh", "hoà bình", "Dấu huyền trên a, dấu huyền trên i"),
    ("quoocs gia", "quốc gia", "oo → ô, dấu sắc"),
    ("truwowngf", "trường", "uw → ư, ow → ơ, dấu huyền"),
    ("anh ays", "anh ấy", "Dấu sắc cuối từ"),
    ("gias tri", "giá trị", "Dấu sắc, dấu nặng"),
]

# ====================== Firefox Download ======================

def get_firefox_versions():
    """Lấy danh sách phiên bản Firefox có sẵn từ Mozilla archive."""
    url = "https://product-details.mozilla.org/1.0/firefox_versions.json"
    try:
        with urlopen(url, timeout=15) as resp:
            data = json.loads(resp.read())
        return {
            "release": data.get("LATEST_FIREFOX_VERSION", ""),
            "esr": data.get("FIREFOX_ESR", ""),
            "beta": data.get("LATEST_FIREFOX_DEVEL_VERSION", ""),
            "nightly": data.get("FIREFOX_NIGHTLY", ""),
        }
    except URLError as e:
        print(f"  Không thể lấy danh sách phiên bản: {e}")
        return {}


def get_download_url(version):
    """Tạo URL tải Firefox cho Windows 64-bit."""
    base = "https://archive.mozilla.org/pub/firefox/releases"
    return f"{base}/{version}/win64/en-US/Firefox Setup {version}.exe"


def get_portable_url(version):
    """URL bản portable (dùng 7zip extract thay vì cài đặt)."""
    base = "https://archive.mozilla.org/pub/firefox/releases"
    # Tải file .exe và extract bằng 7z
    return f"{base}/{version}/win64/en-US/Firefox Setup {version}.exe"


def download_firefox(version, cache_dir=CACHE_DIR):
    """Tải Firefox nếu chưa có trong cache. Trả path tới thư mục Firefox."""
    cache_dir.mkdir(parents=True, exist_ok=True)
    firefox_dir = cache_dir / f"firefox-{version}"
    firefox_exe = firefox_dir / "firefox.exe"

    if firefox_exe.exists():
        print(f"  Firefox {version}: đã có trong cache")
        return firefox_dir

    print(f"  Tải Firefox {version}...")
    url = get_download_url(version)
    installer_path = cache_dir / f"firefox-{version}-setup.exe"

    try:
        urlretrieve(url, installer_path)
    except URLError as e:
        print(f"  Lỗi tải Firefox {version}: {e}")
        return None

    # Extract bằng 7z (nếu có) hoặc chạy installer silent
    seven_zip = shutil.which("7z") or r"C:\Program Files\7-Zip\7z.exe"
    if Path(seven_zip).exists():
        print(f"  Extract bằng 7z...")
        firefox_dir.mkdir(parents=True, exist_ok=True)
        # Firefox .exe installer thực chất là 7z archive
        subprocess.run(
            [seven_zip, "x", str(installer_path), f"-o{firefox_dir}", "-y"],
            capture_output=True
        )
        # Tìm firefox.exe trong core/
        core_dir = firefox_dir / "core"
        if core_dir.exists():
            # Move core/* lên một cấp
            for item in core_dir.iterdir():
                dest = firefox_dir / item.name
                if not dest.exists():
                    shutil.move(str(item), str(dest))
            shutil.rmtree(core_dir, ignore_errors=True)
    else:
        # Sử dụng silent install vào thư mục tùy chọn
        print(f"  Cài đặt silent vào {firefox_dir}...")
        ini_content = f"""[Install]
InstallDirectoryPath={firefox_dir}
QuickLaunchShortcut=false
DesktopShortcut=false
StartMenuShortcuts=false
MaintenanceService=false
"""
        ini_path = cache_dir / f"firefox-{version}-install.ini"
        ini_path.write_text(ini_content)
        subprocess.run(
            [str(installer_path), "/INI=" + str(ini_path)],
            capture_output=True,
            timeout=120
        )
        ini_path.unlink(missing_ok=True)

    installer_path.unlink(missing_ok=True)

    if firefox_exe.exists():
        print(f"  Firefox {version}: OK ({firefox_dir})")
        return firefox_dir
    else:
        print(f"  Firefox {version}: extract/install thất bại")
        return None


# ====================== Selenium Test ======================

def create_profile(profile_dir):
    """Tạo Firefox profile tối giản (tắt update, telemetry)."""
    profile_dir.mkdir(parents=True, exist_ok=True)
    prefs = """
user_pref("app.update.enabled", false);
user_pref("app.update.auto", false);
user_pref("browser.shell.checkDefaultBrowser", false);
user_pref("browser.startup.homepage_override.mstone", "ignore");
user_pref("browser.tabs.warnOnClose", false);
user_pref("datareporting.policy.dataSubmissionEnabled", false);
user_pref("toolkit.telemetry.reportingpolicy.firstRun", false);
user_pref("browser.aboutwelcome.enabled", false);
user_pref("browser.newtabpage.activity-stream.showSponsoredTopSites", false);
"""
    (profile_dir / "prefs.js").write_text(prefs)


def run_selenium_test(firefox_path, test_cases, timeout=30):
    """Chạy test bằng Selenium WebDriver."""
    try:
        from selenium import webdriver
        from selenium.webdriver.firefox.options import Options
        from selenium.webdriver.firefox.service import Service
        from selenium.webdriver.common.by import By
        from selenium.webdriver.common.keys import Keys
        from selenium.webdriver.support.ui import WebDriverWait
        from selenium.webdriver.support import expected_conditions as EC
    except ImportError:
        print("  Cần cài selenium: pip install selenium")
        return None

    # Tạo profile tạm
    profile_dir = Path(tempfile.mkdtemp(prefix="vnkey_fx_"))
    create_profile(profile_dir)

    options = Options()
    options.binary_location = str(firefox_path)
    options.set_preference("app.update.enabled", False)
    options.set_preference("browser.shell.checkDefaultBrowser", False)
    # Profile
    options.profile = str(profile_dir)

    results = []
    driver = None

    try:
        service = Service()
        driver = webdriver.Firefox(service=service, options=options)
        driver.set_page_load_timeout(timeout)

        # Mở trang test đơn giản
        html = """<!DOCTYPE html>
<html><body>
<h3>VnKey Firefox Test</h3>
<div id="tests"></div>
<script>
var container = document.getElementById('tests');
for (var i = 0; i < %d; i++) {
    var input = document.createElement('input');
    input.type = 'text';
    input.id = 'test_' + i;
    input.style.width = '400px';
    input.style.marginBottom = '8px';
    input.style.display = 'block';
    input.style.fontSize = '16px';
    container.appendChild(input);
}
</script>
</body></html>""" % len(test_cases)

        # Lưu HTML tạm
        html_path = profile_dir / "test.html"
        html_path.write_text(html, encoding="utf-8")
        driver.get(f"file:///{html_path.as_posix()}")

        time.sleep(1)  # Chờ VnKey hook bắt foreground

        for i, (keys, expected, desc) in enumerate(test_cases):
            input_el = driver.find_element(By.ID, f"test_{i}")
            input_el.click()
            time.sleep(0.2)

            # Gõ từng phím (mô phỏng người thật)
            for ch in keys:
                input_el.send_keys(ch)
                time.sleep(0.03)  # 30ms giữa các phím

            time.sleep(0.3)  # Chờ VnKey xử lý xong

            actual = input_el.get_attribute("value")
            passed = actual == expected

            results.append({
                "desc": desc,
                "keys": keys,
                "expected": expected,
                "actual": actual,
                "passed": passed,
            })

            status = "PASS" if passed else "FAIL"
            if not passed:
                print(f"    [{status}] {desc}: '{keys}' → '{actual}' (expected '{expected}')")
            else:
                print(f"    [{status}] {desc}")

    except Exception as e:
        print(f"  Lỗi Selenium: {e}")
        results.append({"desc": "SETUP", "error": str(e), "passed": False})
    finally:
        if driver:
            try:
                driver.quit()
            except Exception:
                pass
        # Cleanup profile
        shutil.rmtree(profile_dir, ignore_errors=True)

    return results


# ====================== SendKeys Test (không cần Selenium) ======================

def run_sendkeys_test(firefox_path, test_cases):
    """Test bằng pyautogui hoặc pywinauto (fallback nếu không có Selenium)."""
    try:
        import pyautogui
    except ImportError:
        print("  Cần cài pyautogui hoặc selenium để chạy test")
        print("  pip install selenium  HOẶC  pip install pyautogui")
        return None

    # Mở Firefox với URL
    profile_dir = Path(tempfile.mkdtemp(prefix="vnkey_fx_"))
    create_profile(profile_dir)

    html = """<!DOCTYPE html><html><body>
<input id="test" type="text" style="width:500px;font-size:20px" autofocus>
</body></html>"""
    html_path = profile_dir / "test.html"
    html_path.write_text(html, encoding="utf-8")

    proc = subprocess.Popen([
        str(firefox_path),
        "--profile", str(profile_dir),
        "--no-remote",
        f"file:///{html_path.as_posix()}"
    ])

    time.sleep(5)  # Chờ Firefox khởi động

    results = []
    for keys, expected, desc in test_cases:
        # Clear input
        pyautogui.hotkey("ctrl", "a")
        time.sleep(0.1)
        pyautogui.press("delete")
        time.sleep(0.2)

        # Gõ
        for ch in keys:
            pyautogui.press(ch) if len(ch) == 1 and ch != " " else pyautogui.press("space")
            time.sleep(0.03)
        time.sleep(0.3)

        # Đọc kết quả bằng clipboard
        pyautogui.hotkey("ctrl", "a")
        time.sleep(0.1)
        pyautogui.hotkey("ctrl", "c")
        time.sleep(0.1)

        try:
            import win32clipboard
            win32clipboard.OpenClipboard()
            actual = win32clipboard.GetClipboardData(win32clipboard.CF_UNICODETEXT)
            win32clipboard.CloseClipboard()
        except Exception:
            actual = "???"

        passed = actual == expected
        results.append({
            "desc": desc,
            "keys": keys,
            "expected": expected,
            "actual": actual,
            "passed": passed,
        })
        status = "PASS" if passed else "FAIL"
        print(f"    [{status}] {desc}: '{actual}' {'==' if passed else '!='} '{expected}'")

    proc.terminate()
    shutil.rmtree(profile_dir, ignore_errors=True)
    return results


# ====================== Main ======================

def print_summary(all_results):
    """In tổng kết toàn bộ test."""
    print("\n" + "=" * 60)
    print("TỔNG KẾT")
    print("=" * 60)
    total_pass = 0
    total_fail = 0
    for version, results in all_results.items():
        if results is None:
            print(f"  Firefox {version}: SKIP (không test được)")
            continue
        passed = sum(1 for r in results if r.get("passed"))
        failed = len(results) - passed
        total_pass += passed
        total_fail += failed
        status = "OK" if failed == 0 else "CÓ LỖI"
        print(f"  Firefox {version}: {passed}/{len(results)} passed [{status}]")
        if failed > 0:
            for r in results:
                if not r.get("passed"):
                    if "error" in r:
                        print(f"    - {r['desc']}: {r['error']}")
                    else:
                        print(f"    - {r['desc']}: '{r['actual']}' != '{r['expected']}'")
    print(f"\nTổng: {total_pass} passed, {total_fail} failed")
    return total_fail == 0


def main():
    parser = argparse.ArgumentParser(
        description="Test gõ tiếng Việt (VnKey) trên nhiều phiên bản Firefox"
    )
    parser.add_argument(
        "--versions", nargs="+", default=[],
        help="Danh sách phiên bản Firefox cần test (vd: 115.0 128.0 136.0)"
    )
    parser.add_argument(
        "--list", action="store_true",
        help="Liệt kê phiên bản Firefox hiện tại"
    )
    parser.add_argument(
        "--download-only", action="store_true",
        help="Chỉ tải Firefox, không chạy test"
    )
    parser.add_argument(
        "--local", action="store_true",
        help="Test Firefox đã cài trên máy (không tải)"
    )
    parser.add_argument(
        "--driver", choices=["selenium", "sendkeys"], default="selenium",
        help="Driver test: selenium (mặc định) hoặc sendkeys (pyautogui)"
    )

    args = parser.parse_args()

    if args.list:
        print("Phiên bản Firefox hiện tại:")
        versions = get_firefox_versions()
        for channel, ver in versions.items():
            print(f"  {channel}: {ver}")
        return

    all_results = {}

    # Test Firefox local
    if args.local or not args.versions:
        local_firefox = shutil.which("firefox")
        if not local_firefox:
            # Tìm trong đường dẫn mặc định
            for p in [
                r"C:\Program Files\Mozilla Firefox\firefox.exe",
                r"C:\Program Files (x86)\Mozilla Firefox\firefox.exe",
            ]:
                if Path(p).exists():
                    local_firefox = p
                    break

        if local_firefox:
            # Lấy phiên bản
            try:
                out = subprocess.check_output(
                    [local_firefox, "--version"], text=True, timeout=10
                ).strip()
                ver = out.split()[-1] if out else "local"
            except Exception:
                ver = "local"

            print(f"\n{'='*60}")
            print(f"Test Firefox {ver} (local: {local_firefox})")
            print(f"{'='*60}")

            if not args.download_only:
                if args.driver == "selenium":
                    results = run_selenium_test(
                        local_firefox, TEST_CASES_TELEX
                    )
                else:
                    results = run_sendkeys_test(
                        local_firefox, TEST_CASES_TELEX
                    )
                all_results[ver] = results
        else:
            print("Firefox chưa cài trên máy. Dùng --versions để tải phiên bản cụ thể.")

    # Test phiên bản chỉ định
    for version in args.versions:
        print(f"\n{'='*60}")
        print(f"Firefox {version}")
        print(f"{'='*60}")

        firefox_dir = download_firefox(version)
        if not firefox_dir:
            all_results[version] = None
            continue

        firefox_exe = firefox_dir / "firefox.exe"
        if not firefox_exe.exists():
            print(f"  Không tìm thấy firefox.exe trong {firefox_dir}")
            all_results[version] = None
            continue

        if args.download_only:
            print(f"  Đã tải: {firefox_dir}")
            continue

        print(f"  Chạy test...")
        if args.driver == "selenium":
            results = run_selenium_test(str(firefox_exe), TEST_CASES_TELEX)
        else:
            results = run_sendkeys_test(str(firefox_exe), TEST_CASES_TELEX)
        all_results[version] = results

    if all_results:
        ok = print_summary(all_results)
        sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
