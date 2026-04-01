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

/* Trích xuất JSON object con: "key": { ... } */
static std::string jsonGetObject(const std::string &json, const char *key) {
    std::string needle = std::string("\"") + key + "\"";
    auto pos = json.find(needle);
    if (pos == std::string::npos) return {};
    pos = json.find('{', pos + needle.size());
    if (pos == std::string::npos) return {};
    int depth = 1;
    size_t start = pos;
    pos++;
    while (pos < json.size() && depth > 0) {
        if (json[pos] == '{') depth++;
        else if (json[pos] == '}') depth--;
        pos++;
    }
    if (depth != 0) return {};
    return json.substr(start, pos - start);
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

    /* Nạp app_charsets */
    auto acJson = jsonGetObject(json, "app_charsets");
    if (!acJson.empty()) {
        vnkey_app_charset_from_json(acJson.c_str());
    }
}

void VnKeyEngine::saveConfig() {
    auto dir = configDir();
    if (dir.empty()) return;
    std::filesystem::create_directories(dir);

    char *acJson = vnkey_app_charset_to_json();
    std::string acStr = acJson ? acJson : "{}";
    if (acJson) vnkey_app_charset_free_string(acJson);

    auto path = configPath();
    std::ofstream f(path);
    if (!f.is_open()) return;
    f << "{\n"
      << "  \"input_method\": " << inputMethod_ << ",\n"
      << "  \"output_charset\": " << outputCharset_ << ",\n"
      << "  \"spell_check\": " << (spellCheck_ ? "true" : "false") << ",\n"
      << "  \"free_marking\": " << (freeMarking_ ? "true" : "false") << ",\n"
      << "  \"modern_style\": " << (modernStyle_ ? "true" : "false") << ",\n"
      << "  \"app_charsets\": " << acStr << "\n"
      << "}\n";
}

std::vector<InputMethodEntry> VnKeyEngine::listInputMethods() {
    std::vector<InputMethodEntry> result;
    result.emplace_back("vnkey", "VnKey Vietnamese", "vi", "vnkey");
    result.back().setLabel("Vi").setIcon("fcitx-vnkey");
    return result;
}

/* IM names (matching MENU_ITEMS KIND_IM order) */
struct IMDef { int value; const char *label; };
static const IMDef IM_DEFS[] = {
    {0, "Telex"}, {1, "Simple Telex"}, {2, "VNI"}, {3, "VIQR"},
};
static constexpr size_t IM_COUNT = sizeof(IM_DEFS) / sizeof(IM_DEFS[0]);

/* CS names (matching MENU_ITEMS KIND_CS order) */
struct CSDef { int value; const char *label; };
static const CSDef CS_DEFS[] = {
    {1,  "Unicode (UTF-8)"},
    {40, "VNI Windows"},
    {20, "TCVN3 (ABC)"},
    {10, "VIQR"},
    {4,  "Unicode Composite"},
    {5,  "Vietnamese CP 1258"},
    {2,  "NCR Decimal"},
    {3,  "NCR Hex"},
    {22, "VISCII"},
    {21, "VPS"},
    {41, "BK HCM 2"},
    {23, "BK HCM 1"},
    {42, "Vietware X"},
    {24, "Vietware F"},
    {6,  "Unicode C String"},
};
static constexpr size_t CS_COUNT = sizeof(CS_DEFS) / sizeof(CS_DEFS[0]);

/* Label with visual bullet for radio items — workaround for GNOME Shell
 * AppIndicator extension not requesting toggle-type/toggle-state in GetLayout.
 * The "label" property is always returned regardless of property filter. */
static std::string radioLabel(const char *name, bool selected) {
    if (selected)
        return std::string("\xe2\x97\x8f ") + name;  /* ● name */
    return std::string("    ") + name;                  /*   name */
}

void VnKeyEngine::setupMenu() {
    auto &uiManager = instance_->userInterfaceManager();

    /* ---- Input Method action + submenu ---- */
    imAction_ = std::make_unique<SimpleAction>();
    imAction_->setShortText("Input Method");
    uiManager.registerAction("vnkey-im", imAction_.get());
    imMenu_ = std::make_unique<Menu>();
    imAction_->setMenu(imMenu_.get());

    for (size_t i = 0; i < IM_COUNT; i++) {
        auto act = std::make_unique<SimpleAction>();
        bool sel = (IM_DEFS[i].value == inputMethod_);
        act->setShortText(radioLabel(IM_DEFS[i].label, sel));
        act->setCheckable(true);
        act->setChecked(sel);
        int val = IM_DEFS[i].value;
        act->connect<SimpleAction::Activated>(
            [this, val](InputContext *ic) {
                inputMethod_ = val;
                updateIMAction(ic);
                settingsGen_++;
                saveConfig();
                syncActiveIC(ic);
            });
        uiManager.registerAction("vnkey-im-" + std::to_string(i), act.get());
        imMenu_->addAction(act.get());
        imSubActions_.push_back(std::move(act));
    }

    /* ---- Output Charset action + submenu ---- */
    csAction_ = std::make_unique<SimpleAction>();
    csAction_->setShortText("Character Set");
    uiManager.registerAction("vnkey-cs", csAction_.get());
    csMenu_ = std::make_unique<Menu>();
    csAction_->setMenu(csMenu_.get());

    for (size_t i = 0; i < CS_COUNT; i++) {
        auto act = std::make_unique<SimpleAction>();
        bool sel = (CS_DEFS[i].value == outputCharset_);
        act->setShortText(radioLabel(CS_DEFS[i].label, sel));
        act->setCheckable(true);
        act->setChecked(sel);
        int val = CS_DEFS[i].value;
        act->connect<SimpleAction::Activated>(
            [this, val](InputContext *ic) {
                outputCharset_ = val;
                updateCSAction(ic);
                updateClipLabels();
                settingsGen_++;
                saveConfig();
                syncActiveIC(ic);
            });
        uiManager.registerAction("vnkey-cs-" + std::to_string(i), act.get());
        csMenu_->addAction(act.get());
        csSubActions_.push_back(std::move(act));
    }

    /* ---- Toggle: Spell check ---- */
    spellAction_ = std::make_unique<SimpleAction>();
    spellAction_->setCheckable(true);
    spellAction_->setChecked(spellCheck_);
    spellAction_->setShortText(spellCheck_ ? "Spell check (ON)"
                                           : "Spell check (OFF)");
    spellAction_->connect<SimpleAction::Activated>(
        [this](InputContext *ic) {
            spellCheck_ = !spellCheck_;
            updateSpellAction(ic);
            settingsGen_++;
            saveConfig();
            syncActiveIC(ic);
        });
    uiManager.registerAction("vnkey-spell", spellAction_.get());

    /* ---- Toggle: Free tone marking ---- */
    freeAction_ = std::make_unique<SimpleAction>();
    freeAction_->setCheckable(true);
    freeAction_->setChecked(freeMarking_);
    freeAction_->setShortText(freeMarking_ ? "Free tone marking (ON)"
                                           : "Free tone marking (OFF)");
    freeAction_->connect<SimpleAction::Activated>(
        [this](InputContext *ic) {
            freeMarking_ = !freeMarking_;
            updateFreeAction(ic);
            settingsGen_++;
            saveConfig();
            syncActiveIC(ic);
        });
    uiManager.registerAction("vnkey-free", freeAction_.get());

    /* ---- Toggle: Modern style ---- */
    modernAction_ = std::make_unique<SimpleAction>();
    modernAction_->setCheckable(true);
    modernAction_->setChecked(modernStyle_);
    modernAction_->setShortText(modernStyle_
        ? "Modern style \xe2\x80\x93 o\xc3\xa0, u\xc3\xbd (ON)"
        : "Modern style \xe2\x80\x93 o\xc3\xa0, u\xc3\xbd (OFF)");
    modernAction_->connect<SimpleAction::Activated>(
        [this](InputContext *ic) {
            modernStyle_ = !modernStyle_;
            updateModernAction(ic);
            settingsGen_++;
            saveConfig();
            syncActiveIC(ic);
        });
    uiManager.registerAction("vnkey-modern", modernAction_.get());

    /* ---- Clipboard conversion (non-checkable) ---- */
    clipToUniAction_ = std::make_unique<SimpleAction>();
    clipToUniAction_->setShortText("[CS] \xe2\x86\x92 Unicode (clipboard)");
    clipToUniAction_->connect<SimpleAction::Activated>(
        [this](InputContext *) { convertClipboard(true); });
    uiManager.registerAction("vnkey-clip-to-uni", clipToUniAction_.get());

    clipFromUniAction_ = std::make_unique<SimpleAction>();
    clipFromUniAction_->setShortText("Unicode \xe2\x86\x92 [CS] (clipboard)");
    clipFromUniAction_->connect<SimpleAction::Activated>(
        [this](InputContext *) { convertClipboard(false); });
    uiManager.registerAction("vnkey-clip-from-uni", clipFromUniAction_.get());

    updateClipLabels();
}

void VnKeyEngine::syncActiveIC(InputContext *menuIC) {
    auto *ic = menuIC ? menuIC : instance_->mostRecentInputContext();
    fprintf(stderr, "[vnkey] syncActiveIC: menuIC=%p resolved=%p\n", (void*)menuIC, (void*)ic);
    if (!ic) return;
    auto *state = ic->propertyFor(&factory_);
    if (state) {
        fprintf(stderr, "[vnkey] syncActiveIC: calling syncSettings on state=%p\n", (void*)state);
        state->syncSettings();
    }
}

void VnKeyEngine::updateIMAction(InputContext *ic) {
    for (size_t i = 0; i < imSubActions_.size(); i++) {
        bool sel = (IM_DEFS[i].value == inputMethod_);
        imSubActions_[i]->setShortText(radioLabel(IM_DEFS[i].label, sel));
        imSubActions_[i]->setChecked(sel);
        imSubActions_[i]->update(ic);
    }
    /* Find current IM name for longText */
    const char *imName = "Telex";
    for (size_t i = 0; i < IM_COUNT; i++) {
        if (IM_DEFS[i].value == inputMethod_) {
            imName = IM_DEFS[i].label;
            break;
        }
    }
    imAction_->setLongText(imName);
    imAction_->update(ic);
}

void VnKeyEngine::updateCSAction(InputContext *ic) {
    for (size_t i = 0; i < csSubActions_.size(); i++) {
        bool sel = (CS_DEFS[i].value == outputCharset_);
        csSubActions_[i]->setShortText(radioLabel(CS_DEFS[i].label, sel));
        csSubActions_[i]->setChecked(sel);
        csSubActions_[i]->update(ic);
    }
    const char *csName = "Unicode (UTF-8)";
    for (size_t i = 0; i < CS_COUNT; i++) {
        if (CS_DEFS[i].value == outputCharset_) {
            csName = CS_DEFS[i].label;
            break;
        }
    }
    csAction_->setLongText(csName);
    csAction_->update(ic);
}

void VnKeyEngine::updateSpellAction(InputContext *ic) {
    spellAction_->setChecked(spellCheck_);
    spellAction_->setShortText(spellCheck_ ? "Spell check (ON)"
                                           : "Spell check (OFF)");
    spellAction_->update(ic);
}

void VnKeyEngine::updateFreeAction(InputContext *ic) {
    freeAction_->setChecked(freeMarking_);
    freeAction_->setShortText(freeMarking_ ? "Free tone marking (ON)"
                                           : "Free tone marking (OFF)");
    freeAction_->update(ic);
}

void VnKeyEngine::updateModernAction(InputContext *ic) {
    modernAction_->setChecked(modernStyle_);
    modernAction_->setShortText(modernStyle_
        ? "Modern style \xe2\x80\x93 o\xc3\xa0, u\xc3\xbd (ON)"
        : "Modern style \xe2\x80\x93 o\xc3\xa0, u\xc3\xbd (OFF)");
    modernAction_->update(ic);
}

void VnKeyEngine::updateClipLabels() {
    const char *csName = "Unicode (UTF-8)";
    for (size_t i = 0; i < CS_COUNT; i++) {
        if (CS_DEFS[i].value == outputCharset_) {
            csName = CS_DEFS[i].label;
            break;
        }
    }
    clipToUniAction_->setShortText(
        std::string(csName) + " \xe2\x86\x92 Unicode (clipboard)");
    clipFromUniAction_->setShortText(
        "Unicode \xe2\x86\x92 " + std::string(csName) + " (clipboard)");
}

void VnKeyEngine::updateUI(InputContext *ic) {
    updateIMAction(ic);
    updateCSAction(ic);
    updateSpellAction(ic);
    updateFreeAction(ic);
    updateModernAction(ic);
}

void VnKeyEngine::activate(const InputMethodEntry & /*entry*/,
                           InputContextEvent &event) {
    auto *ic = event.inputContext();
    auto &sa = ic->statusArea();
    sa.addAction(StatusGroup::InputMethod, imAction_.get());
    sa.addAction(StatusGroup::InputMethod, csAction_.get());
    sa.addAction(StatusGroup::InputMethod, spellAction_.get());
    sa.addAction(StatusGroup::InputMethod, freeAction_.get());
    sa.addAction(StatusGroup::InputMethod, modernAction_.get());
    sa.addAction(StatusGroup::InputMethod, clipToUniAction_.get());
    sa.addAction(StatusGroup::InputMethod, clipFromUniAction_.get());
    updateUI(ic);
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
    unsigned gen = engine_->settingsGen();
    int currentIM = engine_->inputMethod();
    int currentCS = engine_->outputCharset();
    bool changed = (gen != lastSettingsGen_);

    if (currentIM != lastIM_ || changed) {
        fprintf(stderr, "[vnkey] syncSettings APPLY: im %d->%d cs %d->%d gen %u->%u\n",
                lastIM_, currentIM, lastCS_, currentCS, lastSettingsGen_, gen);
        if (lastIM_ != -1) {
            commitPreedit();
        }
        vnkey_engine_set_input_method(vnkeyEngine_, currentIM);
        vnkey_engine_reset(vnkeyEngine_);
        lastIM_ = currentIM;
    }
    lastCS_ = currentCS;
    lastSettingsGen_ = gen;
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
    fprintf(stderr, "[vnkey] activate: vietMode=1 engine=%p ic=%p\n",
            (void*)vnkeyEngine_, (void*)ic_);

    /* Cập nhật charset override theo app đang focus */
    auto prog = ic_->program();
    if (!prog.empty()) {
        /* Lấy basename và chuyển lowercase */
        auto pos = prog.rfind('/');
        std::string name = (pos != std::string::npos) ? prog.substr(pos + 1) : prog;
        for (auto &c : name) c = std::tolower(static_cast<unsigned char>(c));
        vnkey_app_charset_update(name.c_str());
    } else {
        vnkey_app_charset_update(nullptr);
    }
}

void VnKeyState::deactivate() {
    fprintf(stderr, "[vnkey] deactivate: ic=%p\n", (void*)ic_);
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
        int app_cs = vnkey_app_charset_get_current();
        if (app_cs >= 0) charset = app_cs;
        if (charset == 1) {
            ic_->commitString(preedit_);
        } else {
            uint8_t buf[4096];
            size_t actualLen = 0;
            int ret = vnkey_charset_from_utf8(
                reinterpret_cast<const uint8_t *>(preedit_.c_str()),
                preedit_.size(), charset,
                buf, sizeof(buf), &actualLen);
            if (ret == 0 && actualLen > 0) {
                if (isUtf8Charset(charset)) {
                    ic_->commitString(
                        std::string(reinterpret_cast<const char *>(buf),
                                    actualLen));
                } else {
                    ic_->commitString(bytesToUtf8(buf, actualLen));
                }
            } else {
                ic_->commitString(preedit_);
            }
        }
        preedit_.clear();
    }
    if (soft)
        vnkey_engine_soft_reset(vnkeyEngine_);
    else
        vnkey_engine_reset(vnkeyEngine_);
}

/* Commit văn bản UTF-8 trực tiếp (chế độ không preedit) */
void VnKeyState::directCommit(const char *utf8, size_t len) {
    int charset = engine_->outputCharset();
    int app_cs = vnkey_app_charset_get_current();
    fprintf(stderr, "[vnkey] directCommit: cs=%d app_cs=%d len=%zu\n", charset, app_cs, len);
    if (app_cs >= 0) charset = app_cs;
    std::string s(utf8, len);
    if (charset == 1) {
        ic_->commitString(s);
    } else {
        uint8_t buf[4096];
        size_t actualLen = 0;
        int ret = vnkey_charset_from_utf8(
            reinterpret_cast<const uint8_t *>(utf8), len,
            charset, buf, sizeof(buf), &actualLen);
        if (ret == 0 && actualLen > 0) {
            if (isUtf8Charset(charset)) {
                ic_->commitString(
                    std::string(reinterpret_cast<const char *>(buf), actualLen));
            } else {
                ic_->commitString(bytesToUtf8(buf, actualLen));
            }
        } else {
            ic_->commitString(s);
        }
    }
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

    /* Xử lý Backspace — chế độ commit trực tiếp */
    if (key.check(FcitxKey_BackSpace)) {
        uint8_t buf[256];
        size_t actualLen = 0;
        size_t backspaces = 0;
        int processed = vnkey_engine_backspace(
            vnkeyEngine_, buf, sizeof(buf), &actualLen, &backspaces, nullptr);

        if (processed && (backspaces > 0 || actualLen > 0)) {
            /* Xóa ký tự đã commit */
            if (backspaces > 0 &&
                ic_->capabilityFlags().test(CapabilityFlag::SurroundingText)) {
                ic_->deleteSurroundingText(
                    -static_cast<int>(backspaces),
                    static_cast<unsigned int>(backspaces));
            }
            /* Commit đầu ra mới */
            if (actualLen > 0) {
                directCommit(reinterpret_cast<const char *>(buf), actualLen);
            }
            keyEvent.filterAndAccept();
            return;
        }

        /* Engine không xử lý: cho hệ thống xử lý backspace */
        return;
    }

    /* Phím ASCII in được  commit trực tiếp (không preedit/gạch chân) */
    auto sym = key.sym();
    if (sym >= FcitxKey_exclam && sym <= FcitxKey_asciitilde) {
        /* Không gọi trySurroundingContext() trong chế độ commit trực tiếp:
         * Surrounding text chứa ký tự đã commit thô (ASCII) hoặc phần đuôi
         * của từ tiếng Việt (ví dụ 'o' sau 'à'), nạp lại sẽ gộp hai từ
         * thành một từ dài vô nghĩa → engine đánh dấu NonVn → mất dấu. */
        uint32_t keyCode = static_cast<uint32_t>(sym);
        uint8_t buf[256];
        size_t actualLen = 0;
        size_t backspaces = 0;

        int processed = vnkey_engine_process(
            vnkeyEngine_, keyCode, buf, sizeof(buf), &actualLen, &backspaces, nullptr);

        bool hasSurrounding = ic_->capabilityFlags().test(CapabilityFlag::SurroundingText);
        fprintf(stderr, "[vnkey] process: key=%c(0x%x) processed=%d bs=%zu len=%zu surrounding=%d\n",
                (keyCode >= 0x20 && keyCode < 0x7f) ? (char)keyCode : '?',
                keyCode, processed, backspaces, actualLen, hasSurrounding);

        if (processed) {
            /* Xóa ký tự đã commit bằng deleteSurroundingText */
            if (backspaces > 0) {
                if (hasSurrounding) {
                    ic_->deleteSurroundingText(
                        -static_cast<int>(backspaces),
                        static_cast<unsigned int>(backspaces));
                    fprintf(stderr, "[vnkey] deleteSurrounding: %zu\n", backspaces);
                } else {
                    fprintf(stderr, "[vnkey] WARNING: need %zu backspaces but no SurroundingText!\n", backspaces);
                }
            }
            /* Commit đầu ra mới */
            if (actualLen > 0) {
                directCommit(reinterpret_cast<const char *>(buf), actualLen);
            }
        } else {
            /* Engine không xử lý: commit ký tự thô */
            char ch = static_cast<char>(keyCode);
            directCommit(&ch, 1);
        }

        /* Nếu engine ở ranh giới từ, reset */
        if (vnkey_engine_at_word_beginning(vnkeyEngine_)) {
            vnkey_engine_reset(vnkeyEngine_);
        }

        keyEvent.filterAndAccept();
        return;
    }

    /* Phím không in được / không ASCII: commit preedit và cho qua */
    commitPreedit();
}

} // namespace fcitx

FCITX_ADDON_FACTORY(fcitx::VnKeyEngineFactory);
