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
    bool spellCheck() const { return spellCheck_; }
    bool freeMarking() const { return freeMarking_; }
    bool modernStyle() const { return modernStyle_; }

private:
    void setupMenu();
    void updateLabel();
    void convertClipboard(bool toUnicode);
    void loadConfig();
    void saveConfig();

    Instance *instance_;
    FactoryFor<VnKeyState> factory_;

    /* Cài đặt */
    int inputMethod_ = 0;    /* 0=Telex */
    int outputCharset_ = 1;  /* 1=UTF-8 */
    bool spellCheck_ = true;
    bool freeMarking_ = true;
    bool modernStyle_ = true;

    /* Menu khay */
    SimpleAction statusAction_;
    Menu menu_;
    std::vector<std::unique_ptr<SimpleAction>> menuItems_;
};

class VnKeyState : public InputContextProperty {
public:
    VnKeyState(VnKeyEngine *engine, InputContext *ic);
    ~VnKeyState() override;

    void keyEvent(KeyEvent &keyEvent);
    void activate();
    void deactivate();
    void reset();

private:
    void commitPreedit(bool soft = false);
    void syncSettings();
    void trySurroundingContext();

    VnKeyEngine *engine_;
    InputContext *ic_;
    ::VnKeyEngine *vnkeyEngine_;
    bool vietMode_ = true;
    std::string preedit_;
    int lastIM_ = -1;
};

class VnKeyEngineFactory : public AddonFactory {
    AddonInstance *create(AddonManager *manager) override {
        return new VnKeyEngine(manager->instance());
    }
};

} // namespace fcitx

#endif /* VNKEY_FCITX5_H */
