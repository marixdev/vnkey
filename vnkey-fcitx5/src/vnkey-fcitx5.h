/*
 * vnkey-fcitx5 — Bộ gõ tiếng Việt cho Fcitx5
 * Sử dụng vnkey-engine (Rust) qua FFI
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#ifndef VNKEY_FCITX5_H
#define VNKEY_FCITX5_H

#include <fcitx/addonfactory.h>
#include <fcitx/addonmanager.h>
#include <fcitx/inputmethodengine.h>
#include <fcitx/instance.h>
#include <fcitx/action.h>
#include <fcitx/menu.h>
#include <fcitx/statusarea.h>
#include <fcitx/userinterfacemanager.h>
#include <fcitx-utils/i18n.h>

#include <memory>
#include <vector>
#include <string>

#include "vnkey-engine.h"

namespace fcitx {

class VnKeyState;

class VnKeyEngine : public InputMethodEngine {
public:
    VnKeyEngine(Instance *instance);
    ~VnKeyEngine() override;

    std::vector<InputMethodEntry> listInputMethods() override;
    void keyEvent(const InputMethodEntry &entry, KeyEvent &keyEvent) override;
    void activate(const InputMethodEntry &entry,
                  InputContextEvent &event) override;
    void deactivate(const InputMethodEntry &entry,
                    InputContextEvent &event) override;
    void reset(const InputMethodEntry &entry,
               InputContextEvent &event) override;

    auto factory() { return &factory_; }
    Instance *instance() { return instance_; }

    int inputMethod() const { return inputMethod_; }
    int outputCharset() const { return outputCharset_; }
    unsigned settingsGen() const { return settingsGen_; }
    bool spellCheck() const { return spellCheck_; }
    bool freeMarking() const { return freeMarking_; }
    bool modernStyle() const { return modernStyle_; }

private:
    void setupMenu();
    void updateIMAction(InputContext *ic);
    void updateCSAction(InputContext *ic);
    void updateSpellAction(InputContext *ic);
    void updateFreeAction(InputContext *ic);
    void updateModernAction(InputContext *ic);
    void updateClipLabels();
    void updateUI(InputContext *ic);
    void convertClipboard(bool toUnicode);
    void loadConfig();
    void saveConfig();
    void syncActiveIC(InputContext *menuIC = nullptr);

    Instance *instance_;
    FactoryFor<VnKeyState> factory_;

    /* Cài đặt */
    int inputMethod_ = 0;    /* 0=Telex */
    int outputCharset_ = 1;  /* 1=UTF-8 */
    bool spellCheck_ = true;
    bool freeMarking_ = true;
    bool modernStyle_ = true;
    unsigned settingsGen_ = 0;

    /* Menu khay — giống fcitx5-unikey: mỗi nhóm một Action + Menu riêng */
    std::unique_ptr<SimpleAction> imAction_;
    std::unique_ptr<Menu> imMenu_;
    std::vector<std::unique_ptr<SimpleAction>> imSubActions_;

    std::unique_ptr<SimpleAction> csAction_;
    std::unique_ptr<Menu> csMenu_;
    std::vector<std::unique_ptr<SimpleAction>> csSubActions_;

    std::unique_ptr<SimpleAction> spellAction_;
    std::unique_ptr<SimpleAction> freeAction_;
    std::unique_ptr<SimpleAction> modernAction_;
    std::unique_ptr<SimpleAction> clipToUniAction_;
    std::unique_ptr<SimpleAction> clipFromUniAction_;
};

class VnKeyState : public InputContextProperty {
public:
    VnKeyState(VnKeyEngine *engine, InputContext *ic);
    ~VnKeyState() override;

    void keyEvent(KeyEvent &keyEvent);
    void activate();
    void deactivate();
    void reset();
    void syncSettings();

private:
    void commitPreedit(bool soft = false);
    void directCommit(const char *utf8, size_t len);
    void trySurroundingContext();

    VnKeyEngine *engine_;
    InputContext *ic_;
    ::VnKeyEngine *vnkeyEngine_;
    bool vietMode_ = true;
    std::string preedit_;
    int lastIM_ = -1;
    int lastCS_ = -1;
    unsigned lastSettingsGen_ = 0;
};

class VnKeyEngineFactory : public AddonFactory {
    AddonInstance *create(AddonManager *manager) override {
        return new VnKeyEngine(manager->instance());
    }
};

} // namespace fcitx

#endif /* VNKEY_FCITX5_H */
