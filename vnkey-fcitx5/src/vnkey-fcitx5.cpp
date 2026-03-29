/*
 * vnkey-fcitx5 — Bộ gõ tiếng Việt cho Fcitx5
 * Sử dụng vnkey-engine (Rust) qua FFI
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#include "vnkey-fcitx5.h"

#include <fcitx/inputcontext.h>
#include <fcitx/inputpanel.h>
#include <fcitx-utils/utf8.h>

#include <cstring>
#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <sstream>

namespace fcitx {

// ==================== Dữ liệu menu ====================

enum ItemKind { KIND_IM, KIND_CS, KIND_OPT_SPELL, KIND_OPT_FREE, KIND_OPT_MODERN,
                KIND_CLIP_TO_UNI, KIND_CLIP_FROM_UNI };

struct MenuItem {
    ItemKind kind;
    int      value;   /* ID kiểu gõ hoặc ID bảng mã, bỏ qua cho tùy chọn */
    const char *label;
};

static const MenuItem MENU_ITEMS[] = {
    /* ---- Kiểu gõ ---- */
    {KIND_IM, 0, "IM: Telex"},
    {KIND_IM, 1, "IM: Simple Telex"},
    {KIND_IM, 2, "IM: VNI"},
    {KIND_IM, 3, "IM: VIQR"},
    /* ---- Bảng mã ---- */
    {KIND_CS, 1,  "CS: Unicode (UTF-8)"},
    {KIND_CS, 40, "CS: VNI Windows"},
    {KIND_CS, 20, "CS: TCVN3 (ABC)"},
    {KIND_CS, 10, "CS: VIQR"},
    {KIND_CS, 4,  "CS: Unicode Composite"},
    {KIND_CS, 5,  "CS: Vietnamese CP 1258"},
    {KIND_CS, 2,  "CS: NCR Decimal"},
    {KIND_CS, 3,  "CS: NCR Hex"},
    {KIND_CS, 22, "CS: VISCII"},
    {KIND_CS, 21, "CS: VPS"},
    {KIND_CS, 41, "CS: BK HCM 2"},
    {KIND_CS, 23, "CS: BK HCM 1"},
    {KIND_CS, 42, "CS: Vietware X"},
    {KIND_CS, 24, "CS: Vietware F"},
    {KIND_CS, 6,  "CS: Unicode C String"},
    /* ---- Tùy chọn ---- */
    {KIND_OPT_SPELL,  0, "Spell check"},
    {KIND_OPT_FREE,   0, "Free tone marking"},
    {KIND_OPT_MODERN, 0, "Modern style (oÃ , uÃ½)"},
    /* ---- Chuyển mã clipboard ---- */
    {KIND_CLIP_TO_UNI,   0, "[CS] \xe2\x86\x92 Unicode (clipboard)"},
    {KIND_CLIP_FROM_UNI, 0, "Unicode \xe2\x86\x92 [CS] (clipboard)"},
};
static constexpr size_t MENU_COUNT = sizeof(MENU_ITEMS) / sizeof(MENU_ITEMS[0]);

// ==================== Lưu trữ cấu hình ====================

static std::string configDir() {
    const char *xdg = std::getenv("XDG_CONFIG_HOME");
    if (xdg && xdg[0])
        return std::string(xdg) + "/vnkey";
    const char *home = std::getenv("HOME");
    if (home && home[0])
        return std::string(home) + "/.config/vnkey";
    return {};
}

static std::string configPath() {
    auto dir = configDir();
    if (dir.empty()) return {};
    return dir + "/config.json";
}

// Trích xuất JSON đơn giản (không phụ thuộc ngoài)
static std::string jsonGet(const std::string &json, const char *key) {
    std::string needle = std::string("\"") + key + "\"";
    auto pos = json.find(needle);
    if (pos == std::string::npos) return {};
    pos = json.find(':', pos + needle.size());
    if (pos == std::string::npos) return {};
    pos++;
    while (pos < json.size() && (json[pos] == ' ' || json[pos] == '\t'))
        pos++;
    auto end = json.find_first_of(",}\n", pos);
    if (end == std::string::npos) end = json.size();
    auto val = json.substr(pos, end - pos);
    // bỏ khoảng trắng
    while (!val.empty() && val.back() <= ' ') val.pop_back();
    while (!val.empty() && val.front() <= ' ') val.erase(val.begin());
    return val;
}

static int jsonGetInt(const std::string &json, const char *key, int def) {
    auto v = jsonGet(json, key);
    if (v.empty()) return def;
    try { return std::stoi(v); } catch (...) { return def; }
}

static bool jsonGetBool(const std::string &json, const char *key, bool def) {
    auto v = jsonGet(json, key);
    if (v == "true") return true;
    if (v == "false") return false;
    return def;
}

// ==================== VnKeyEngine (cấp addon) ====================

VnKeyEngine::VnKeyEngine(Instance *instance)
    : instance_(instance),
      factory_([this](InputContext &ic) {
          return new VnKeyState(this, &ic);
      }) {
    instance->inputContextManager().registerProperty("vnkeyState",
                                                     &factory_);
    loadConfig();
    setupMenu();
}

VnKeyEngine::~VnKeyEngine() {}

void VnKeyEngine::loadConfig() {
    auto path = configPath();
    if (path.empty()) return;
    std::ifstream f(path);
    if (!f.is_open()) return;
    std::ostringstream ss;
    ss << f.rdbuf();
    auto json = ss.str();

    inputMethod_   = jsonGetInt(json, "input_method", 0);
    outputCharset_ = jsonGetInt(json, "output_charset", 1);
    spellCheck_    = jsonGetBool(json, "spell_check", true);
    freeMarking_   = jsonGetBool(json, "free_marking", true);
    modernStyle_   = jsonGetBool(json, "modern_style", true);
}

void VnKeyEngine::saveConfig() {
    auto dir = configDir();
    if (dir.empty()) return;
    std::filesystem::create_directories(dir);

    auto path = configPath();
    std::ofstream f(path);
    if (!f.is_open()) return;
    f << "{\n"
      << "  \"input_method\": " << inputMethod_ << ",\n"
      << "  \"output_charset\": " << outputCharset_ << ",\n"
      << "  \"spell_check\": " << (spellCheck_ ? "true" : "false") << ",\n"
      << "  \"free_marking\": " << (freeMarking_ ? "true" : "false") << ",\n"
      << "  \"modern_style\": " << (modernStyle_ ? "true" : "false") << "\n"
      << "}\n";
}

std::vector<InputMethodEntry> VnKeyEngine::listInputMethods() {
    std::vector<InputMethodEntry> result;
    result.emplace_back("vnkey", "VnKey Vietnamese", "vi", "vnkey");
    result.back().setLabel("Vi").setIcon("fcitx-vnkey");
    return result;
}

void VnKeyEngine::setupMenu() {
    statusAction_.setShortText("VnKey [Telex]");
    statusAction_.setMenu(&menu_);
    instance_->userInterfaceManager().registerAction("vnkey-status",
                                                     &statusAction_);

    for (size_t i = 0; i < MENU_COUNT; i++) {
        auto act = std::make_unique<SimpleAction>();
        const auto &item = MENU_ITEMS[i];
        act->setShortText(item.label);

        bool isClip = (item.kind == KIND_CLIP_TO_UNI ||
                       item.kind == KIND_CLIP_FROM_UNI);
        act->setCheckable(!isClip);

        /* Đặt trạng thái checked ban đầu */
        switch (item.kind) {
        case KIND_IM:    act->setChecked(item.value == inputMethod_); break;
        case KIND_CS:    act->setChecked(item.value == outputCharset_); break;
        case KIND_OPT_SPELL:  act->setChecked(spellCheck_); break;
        case KIND_OPT_FREE:   act->setChecked(freeMarking_); break;
        case KIND_OPT_MODERN: act->setChecked(modernStyle_); break;
        default: break;
        }

        size_t idx = i;
        act->connect<SimpleAction::Activated>(
            [this, idx](InputContext *) {
                const auto &it = MENU_ITEMS[idx];
                switch (it.kind) {
                case KIND_IM:    inputMethod_ = it.value; break;
                case KIND_CS:    outputCharset_ = it.value; break;
                case KIND_OPT_SPELL:  spellCheck_ = !spellCheck_; break;
                case KIND_OPT_FREE:   freeMarking_ = !freeMarking_; break;
                case KIND_OPT_MODERN: modernStyle_ = !modernStyle_; break;
                case KIND_CLIP_TO_UNI:   convertClipboard(true); break;
                case KIND_CLIP_FROM_UNI: convertClipboard(false); break;
                }
                updateLabel();
                if (it.kind != KIND_CLIP_TO_UNI && it.kind != KIND_CLIP_FROM_UNI)
                    saveConfig();
            });

        instance_->userInterfaceManager().registerAction(
            "vnkey-item-" + std::to_string(i), act.get());
        menu_.addAction(act.get());
        menuItems_.push_back(std::move(act));
    }
}

void VnKeyEngine::updateLabel() {
    /* Xác định tên kiểu gõ và bảng mã hiện tại */
    const char *imName = "Telex";
    const char *csName = "Unicode (UTF-8)";
    for (size_t i = 0; i < MENU_COUNT; i++) {
        if (MENU_ITEMS[i].kind == KIND_CS &&
            MENU_ITEMS[i].value == outputCharset_) {
            csName = MENU_ITEMS[i].label + 4; /* bỏ "CS: " */
        }
    }
    for (size_t i = 0; i < MENU_COUNT; i++) {
        if (MENU_ITEMS[i].kind == KIND_IM) {
            bool checked = (MENU_ITEMS[i].value == inputMethod_);
            menuItems_[i]->setChecked(checked);
            if (checked) imName = MENU_ITEMS[i].label + 4; /* bỏ "IM: " */
        } else if (MENU_ITEMS[i].kind == KIND_CS) {
            menuItems_[i]->setChecked(MENU_ITEMS[i].value == outputCharset_);
        } else if (MENU_ITEMS[i].kind == KIND_OPT_SPELL) {
            menuItems_[i]->setChecked(spellCheck_);
        } else if (MENU_ITEMS[i].kind == KIND_OPT_FREE) {
            menuItems_[i]->setChecked(freeMarking_);
        } else if (MENU_ITEMS[i].kind == KIND_OPT_MODERN) {
            menuItems_[i]->setChecked(modernStyle_);
        } else if (MENU_ITEMS[i].kind == KIND_CLIP_TO_UNI) {
            std::string lbl = std::string(csName) +
                " \xe2\x86\x92 Unicode (clipboard)";
            menuItems_[i]->setShortText(lbl);
        } else if (MENU_ITEMS[i].kind == KIND_CLIP_FROM_UNI) {
            std::string lbl = "Unicode \xe2\x86\x92 " +
                std::string(csName) + " (clipboard)";
            menuItems_[i]->setShortText(lbl);
        }
    }

    std::string label = std::string("VnKey [") + imName + "]";
    statusAction_.setShortText(label);

    auto *ic = instance_->mostRecentInputContext();
    if (ic) {
        statusAction_.update(ic);
    }
}

void VnKeyEngine::activate(const InputMethodEntry & /*entry*/,
                           InputContextEvent &event) {
    auto *ic = event.inputContext();
    ic->statusArea().addAction(StatusGroup::InputMethod, &statusAction_);
    auto *state = ic->propertyFor(&factory_);
    state->activate();
}

void VnKeyEngine::deactivate(const InputMethodEntry & /*entry*/,
                             InputContextEvent &event) {
    auto *state = event.inputContext()->propertyFor(&factory_);
    state->deactivate();
}

void VnKeyEngine::reset(const InputMethodEntry & /*entry*/,
                        InputContextEvent &event) {
    auto *state = event.inputContext()->propertyFor(&factory_);
    state->reset();
}

void VnKeyEngine::keyEvent(const InputMethodEntry & /*entry*/,
                           KeyEvent &keyEvent) {
    auto *state = keyEvent.inputContext()->propertyFor(&factory_);
    state->keyEvent(keyEvent);
}

// ==================== VnKeyState (theo IC) ====================

VnKeyState::VnKeyState(VnKeyEngine *engine, InputContext *ic)
    : engine_(engine), ic_(ic) {
    vnkeyEngine_ = vnkey_engine_new();
    syncSettings();
}

VnKeyState::~VnKeyState() {
    vnkey_engine_free(vnkeyEngine_);
}

void VnKeyState::syncSettings() {
    int currentIM = engine_->inputMethod();
    if (currentIM != lastIM_) {
        if (lastIM_ != -1) {
            commitPreedit();
        }
        vnkey_engine_set_input_method(vnkeyEngine_, currentIM);
        vnkey_engine_reset(vnkeyEngine_);
        lastIM_ = currentIM;
    }
    vnkey_engine_set_options(vnkeyEngine_,
        engine_->freeMarking() ? 1 : 0,
        engine_->modernStyle() ? 1 : 0,
        engine_->spellCheck() ? 1 : 0,
        1 /* auto_restore */);
}

void VnKeyState::activate() {
    vietMode_ = true;
    vnkey_engine_set_viet_mode(vnkeyEngine_, 1);
    syncSettings();
    vnkey_engine_reset(vnkeyEngine_);
    preedit_.clear();
}

void VnKeyState::deactivate() {
    commitPreedit();
    vnkey_engine_reset(vnkeyEngine_);
    preedit_.clear();
}

void VnKeyState::reset() {
    vnkey_engine_reset(vnkeyEngine_);
    preedit_.clear();
    if (ic_->capabilityFlags().test(CapabilityFlag::Preedit)) {
        ic_->inputPanel().reset();
        ic_->updatePreedit();
        ic_->updateUserInterface(UserInterfaceComponent::InputPanel);
    }
}

/* Nâng byte thô (Latin-1) lên UTF-8.
 * Mỗi byte 0x00-0x7F giữ nguyên; 0x80-0xFF trở thành chuỗi UTF-8 2 byte.
 * Đây là cách font tiếng Việt cũ (vd: .VnTime cho TCVN3) hoạt động. */
static std::string bytesToUtf8(const uint8_t *data, size_t len) {
    std::string out;
    out.reserve(len * 2);
    for (size_t i = 0; i < len; i++) {
        uint8_t ch = data[i];
        if (ch < 0x80) {
            out.push_back(static_cast<char>(ch));
        } else {
            out.push_back(static_cast<char>(0xC0 | (ch >> 6)));
            out.push_back(static_cast<char>(0x80 | (ch & 0x3F)));
        }
    }
    return out;
}

/* Bảng mã có đầu ra là UTF-8 / ASCII hợp lệ */
static bool isUtf8Charset(int id) {
    return id == 1  /* UTF-8 */
        || id == 2  /* NCR Decimal */
        || id == 3  /* NCR Hex */
        || id == 4  /* Unicode Composite (decomposed UTF-8) */
        || id == 6  /* Unicode C String */
        || id == 10 /* VIQR (ASCII) */
        || id == 11 /* UTF-8 VIQR */;
}

/* Ngược lại của bytesToUtf8: giải mã UTF-8 thành byte thô.
 * Chỉ giữ các codepoint < 256 (phạm vi Latin-1). */
static std::vector<uint8_t> utf8ToBytes(const std::string &s) {
    std::vector<uint8_t> result;
    result.reserve(s.size());
    size_t i = 0;
    while (i < s.size()) {
        auto c = static_cast<uint8_t>(s[i]);
        uint32_t cp;
        if (c < 0x80) {
            cp = c; i += 1;
        } else if ((c & 0xE0) == 0xC0) {
            cp = (c & 0x1Fu) << 6;
            if (i + 1 < s.size())
                cp |= (static_cast<uint8_t>(s[i + 1]) & 0x3Fu);
            i += 2;
        } else if ((c & 0xF0) == 0xE0) {
            cp = (c & 0x0Fu) << 12;
            if (i + 1 < s.size())
                cp |= (static_cast<uint8_t>(s[i + 1]) & 0x3Fu) << 6;
            if (i + 2 < s.size())
                cp |= (static_cast<uint8_t>(s[i + 2]) & 0x3Fu);
            i += 3;
        } else {
            cp = (c & 0x07u) << 18;
            if (i + 1 < s.size())
                cp |= (static_cast<uint8_t>(s[i + 1]) & 0x3Fu) << 12;
            if (i + 2 < s.size())
                cp |= (static_cast<uint8_t>(s[i + 2]) & 0x3Fu) << 6;
            if (i + 3 < s.size())
                cp |= (static_cast<uint8_t>(s[i + 3]) & 0x3Fu);
            i += 4;
        }
        if (cp < 256) {
            result.push_back(static_cast<uint8_t>(cp));
        }
    }
    return result;
}

/* Đọc văn bản clipboard hệ thống â thử từng công cụ cho đến khi thành công */
static std::string getClipboard() {
    const char *cmds[] = {
        "wl-paste --no-newline 2>/dev/null",
        "xclip -selection clipboard -o 2>/dev/null",
        "xsel --clipboard --output 2>/dev/null",
    };
    for (const char *cmd : cmds) {
        FILE *fp = popen(cmd, "r");
        if (!fp) continue;
        std::string result;
        char buf[4096];
        size_t n;
        while ((n = std::fread(buf, 1, sizeof(buf), fp)) > 0) {
            result.append(buf, n);
        }
        int status = pclose(fp);
        if (status == 0 && !result.empty()) return result;
    }
    return {};
}

/* Ghi văn bản vào clipboard hệ thống â thử từng công cụ cho đến khi thành công */
static bool setClipboard(const std::string &text) {
    const char *cmds[] = {
        "wl-copy 2>/dev/null",
        "xclip -selection clipboard -i 2>/dev/null",
        "xsel --clipboard --input 2>/dev/null",
    };
    for (const char *cmd : cmds) {
        FILE *fp = popen(cmd, "w");
        if (!fp) continue;
        std::fwrite(text.data(), 1, text.size(), fp);
        if (pclose(fp) == 0) return true;
    }
    return false;
}

void VnKeyEngine::convertClipboard(bool toUnicode) {
    std::string clip = getClipboard();
    if (clip.empty()) return;

    int cs = outputCharset_;
    if (cs == 1) return; /* UTF-8 â UTF-8 không cần chuyển */

    std::string result;

    if (toUnicode) {
        /* Clipboard chứa văn bản mã cũ â chuyển sang Unicode UTF-8 */
        if (isUtf8Charset(cs)) {
            /* Bảng mã an toàn văn bản (VIQR, NCR, v.v.): clipboard là UTF-8 hợp lệ */
            size_t bufSize = clip.size() * 4 + 1024;
            std::vector<uint8_t> buf(bufSize);
            size_t actualLen = 0;
            int ret = vnkey_charset_to_utf8(
                reinterpret_cast<const uint8_t *>(clip.c_str()),
                clip.size(), cs, buf.data(), bufSize, &actualLen);
            if (ret == 0 && actualLen > 0)
                result.assign(reinterpret_cast<char *>(buf.data()), actualLen);
        } else {
            /* Bảng mã cũ: chuyển ngược UTF-8 thành byte thô trước */
            auto bytes = utf8ToBytes(clip);
            size_t bufSize = bytes.size() * 4 + 1024;
            std::vector<uint8_t> buf(bufSize);
            size_t actualLen = 0;
            int ret = vnkey_charset_to_utf8(
                bytes.data(), bytes.size(), cs,
                buf.data(), bufSize, &actualLen);
            if (ret == 0 && actualLen > 0)
                result.assign(reinterpret_cast<char *>(buf.data()), actualLen);
        }
    } else {
        /* Clipboard chứa Unicode â chuyển sang bảng mã đích */
        size_t bufSize = clip.size() * 4 + 1024;
        std::vector<uint8_t> buf(bufSize);
        size_t actualLen = 0;
        int ret = vnkey_charset_from_utf8(
            reinterpret_cast<const uint8_t *>(clip.c_str()),
            clip.size(), cs, buf.data(), bufSize, &actualLen);
        if (ret == 0 && actualLen > 0) {
            if (isUtf8Charset(cs))
                result.assign(reinterpret_cast<char *>(buf.data()), actualLen);
            else
                result = bytesToUtf8(buf.data(), actualLen);
        }
    }

    if (!result.empty()) {
        setClipboard(result);
    }
}

void VnKeyState::commitPreedit(bool soft) {
    if (!preedit_.empty()) {
        int charset = engine_->outputCharset();
        if (charset == 1) {
            /* UTF-8: commit trực tiếp */
            ic_->commitString(preedit_);
        } else {
            /* Chuyển sang bảng mã đích */
            uint8_t buf[4096];
            size_t actualLen = 0;
            int ret = vnkey_charset_from_utf8(
                reinterpret_cast<const uint8_t *>(preedit_.c_str()),
                preedit_.size(), charset,
                buf, sizeof(buf), &actualLen);
            if (ret == 0 && actualLen > 0) {
                if (isUtf8Charset(charset)) {
                    /* Đầu ra đã là UTF-8/ASCII hợp lệ */
                    ic_->commitString(
                        std::string(reinterpret_cast<const char *>(buf),
                                    actualLen));
                } else {
                    /* Bảng mã cũ: nâng byte thô lên UTF-8
                     * (dùng với font tiếng Việt cũ) */
                    ic_->commitString(bytesToUtf8(buf, actualLen));
                }
            } else {
                /* Dự phòng: dùng UTF-8 */
                ic_->commitString(preedit_);
            }
        }
        preedit_.clear();
        if (ic_->capabilityFlags().test(CapabilityFlag::Preedit)) {
            ic_->inputPanel().reset();
            ic_->updatePreedit();
            ic_->updateUserInterface(UserInterfaceComponent::InputPanel);
        }
    }
    if (soft)
        vnkey_engine_soft_reset(vnkeyEngine_);
    else
        vnkey_engine_reset(vnkeyEngine_);
}

void VnKeyState::trySurroundingContext() {
    /* Nếu engine đang ở đầu từ và preedit trống,
     * thử đọc surrounding text để khôi phục ngữ cảnh phụ âm đứng trước.
     * Giải quyết vấn đề: commit "giá", xóa "iá", gõ lại → engine cần biết 'g'. */
    if (!vnkey_engine_at_word_beginning(vnkeyEngine_) || !preedit_.empty())
        return;

    if (!ic_->capabilityFlags().test(CapabilityFlag::SurroundingText))
        return;

    const auto &st = ic_->surroundingText();
    const auto &text = st.text();
    unsigned int cursor = st.cursor();
    if (text.empty() || cursor == 0)
        return;

    /* Lùi từ vị trí con trỏ để tìm phần đầu từ (chỉ ASCII chữ cái) */
    auto u32text = text;  /* Fcitx5 SurroundingText.text() trả std::string UTF-8 */
    /* Chuyển sang duyệt byte — chỉ quan tâm ASCII trailing */
    size_t bytePos = 0;
    size_t charIdx = 0;
    /* Tìm byte offset của cursor (cursor tính theo ký tự UTF-8) */
    for (size_t i = 0; i < text.size() && charIdx < cursor; ) {
        unsigned char c = static_cast<unsigned char>(text[i]);
        if (c < 0x80) { i++; }
        else if (c < 0xE0) { i += 2; }
        else if (c < 0xF0) { i += 3; }
        else { i += 4; }
        charIdx++;
        bytePos = i;
    }

    /* Lùi lại tìm ký tự ASCII chữ cái liền trước cursor */
    size_t start = bytePos;
    while (start > 0) {
        unsigned char prev = static_cast<unsigned char>(text[start - 1]);
        if (prev >= 0x80 || !std::isalpha(prev))
            break;
        start--;
    }

    if (start >= bytePos)
        return; /* Không có chữ cái ASCII nào trước cursor */

    std::string ctx = text.substr(start, bytePos - start);

    /* Giới hạn ngữ cảnh: chỉ nạp tối đa 10 ký tự cuối từ */
    if (ctx.size() > 10)
        ctx = ctx.substr(ctx.size() - 10);

    vnkey_engine_feed_context(vnkeyEngine_, ctx.c_str());
}

void VnKeyState::keyEvent(KeyEvent &keyEvent) {
    /* Đồng bộ cài đặt từ menu (kiểu gõ, tùy chọn) */
    syncSettings();

    /* Bỏ qua nhả phím và phím modifier */
    if (keyEvent.isRelease()) {
        return;
    }

    auto key = keyEvent.key();

    /* Bật/tắt tiếng Việt: Ctrl+Space */
    if (key.check(Key(FcitxKey_space, KeyState::Ctrl))) {
        vietMode_ = !vietMode_;
        vnkey_engine_set_viet_mode(vnkeyEngine_, vietMode_ ? 1 : 0);
        keyEvent.filterAndAccept();
        return;
    }

    /* Cho qua các phím có modifier (Ctrl, Alt, Super) trừ Shift */
    if (key.states().testAny(KeyStates{KeyState::Ctrl} |
                             KeyState::Alt | KeyState::Super)) {
        commitPreedit();
        return;
    }

    /* Xử lý Enter, Escape, Tab: commit preedit và cho qua */
    if (key.check(FcitxKey_Return) || key.check(FcitxKey_KP_Enter) ||
        key.check(FcitxKey_Escape) || key.check(FcitxKey_Tab)) {
        commitPreedit();
        return; /* để Fcitx xử lý */
    }

    /* Xử lý dấu cách: commit preedit + soft reset để backspace khôi phục */
    if (key.check(FcitxKey_space)) {
        commitPreedit(true);
        return; /* để Fcitx gửi dấu cách */
    }

    /* Xử lý Backspace */
    if (key.check(FcitxKey_BackSpace)) {
        uint8_t buf[256];
        size_t actualLen = 0;
        size_t backspaces = 0;
        int processed = vnkey_engine_backspace(
            vnkeyEngine_, buf, sizeof(buf), &actualLen, &backspaces);

        if (processed && (backspaces > 0 || actualLen > 0)) {
            /* Xóa 'backspaces' ký tự UTF-8 từ preedit */
            for (size_t i = 0; i < backspaces; i++) {
                if (!preedit_.empty()) {
                    auto len = preedit_.size();
                    while (len > 0 &&
                           (static_cast<unsigned char>(preedit_[len - 1]) & 0xC0) == 0x80) {
                        len--;
                    }
                    if (len > 0) {
                        len--;
                    }
                    preedit_.resize(len);
                }
            }
            /* Thêm đầu ra mới */
            if (actualLen > 0) {
                preedit_.append(reinterpret_cast<const char *>(buf), actualLen);
            }

            /* Cập nhật hiển thị preedit */
            if (ic_->capabilityFlags().test(CapabilityFlag::Preedit)) {
                Text preeditText;
                preeditText.append(preedit_,
                                   TextFormatFlag::Underline);
                preeditText.setCursor(preedit_.size());
                ic_->inputPanel().setClientPreedit(preeditText);
                ic_->updatePreedit();
                ic_->updateUserInterface(
                    UserInterfaceComponent::InputPanel);
            }

            keyEvent.filterAndAccept();
            return;
        }

        /* Engine không xử lý: commit và để hệ thống xử lý backspace */
        if (!preedit_.empty()) {
            commitPreedit();
        }
        return;
    }

    /* Phím ASCII in được â gửi tới vnkey engine */
    auto sym = key.sym();
    if (sym >= FcitxKey_exclam && sym <= FcitxKey_asciitilde) {        /* Nếu preedit rỗng, thử đọc surrounding text để khôi phục ngữ cảnh
         * (vd: đã commit 'g', xóa phần sau, gõ tiếp → engine cần biết 'g') */
        trySurroundingContext();
        uint32_t keyCode = static_cast<uint32_t>(sym);
        uint8_t buf[256];
        size_t actualLen = 0;
        size_t backspaces = 0;

        int processed = vnkey_engine_process(
            vnkeyEngine_, keyCode, buf, sizeof(buf), &actualLen, &backspaces);

        if (processed) {
            /* Xóa 'backspaces' ký tự UTF-8 từ preedit_ */
            for (size_t i = 0; i < backspaces; i++) {
                if (!preedit_.empty()) {
                    auto len = preedit_.size();
                    while (len > 0 &&
                           (static_cast<unsigned char>(preedit_[len - 1]) & 0xC0) == 0x80) {
                        len--;
                    }
                    if (len > 0) {
                        len--;
                    }
                    preedit_.resize(len);
                }
            }

            /* Thêm đầu ra mới */
            if (actualLen > 0) {
                preedit_.append(reinterpret_cast<const char *>(buf), actualLen);
            }
        } else {
            /* Engine không biến đổi phím, nhưng vẫn có thể đang theo dõi
             * nội bộ (vd: 'm','u','o','n' trước phím thanh như 'f').
             * Giữ ký tự trong preedit để backspace sau này hoạt động. */
            char ch = static_cast<char>(keyCode);
            preedit_.append(&ch, 1);
        }

        /* Nếu engine báo ranh giới từ, commit ngay */
        if (vnkey_engine_at_word_beginning(vnkeyEngine_)) {
            commitPreedit();
        } else {
            /* Cập nhật preedit */
            if (ic_->capabilityFlags().test(CapabilityFlag::Preedit)) {
                Text preeditText;
                preeditText.append(preedit_,
                                   TextFormatFlag::Underline);
                preeditText.setCursor(preedit_.size());
                ic_->inputPanel().setClientPreedit(preeditText);
                ic_->updatePreedit();
                ic_->updateUserInterface(
                    UserInterfaceComponent::InputPanel);
            }
        }

        keyEvent.filterAndAccept();
        return;
    }

    /* Phím không in được / không ASCII: commit preedit và cho qua */
    commitPreedit();
}

} // namespace fcitx

FCITX_ADDON_FACTORY(fcitx::VnKeyEngineFactory);
