/*
 * VnKeyPreferences — Cửa sổ cài đặt VnKey (code-based, không dùng nib)
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#import "VnKeyPreferences.h"

static NSString *const kVnKeyInputMethod   = @"VnKeyInputMethod";
static NSString *const kVnKeySpellCheck    = @"VnKeySpellCheck";
static NSString *const kVnKeyFreeMarking   = @"VnKeyFreeMarking";
static NSString *const kVnKeyModernStyle   = @"VnKeyModernStyle";
static NSString *const kVnKeyAutoRestore   = @"VnKeyAutoRestore";
static NSString *const kVnKeyEdeMode       = @"VnKeyEdeMode";
static NSString *const kVnKeyMacroEnabled  = @"VnKeyMacroEnabled";
static NSString *const kVnKeyUsePreedit    = @"VnKeyUsePreedit";

/* ==================== PreferencesController ==================== */

@interface VnKeyPreferencesController : NSObject

@property (nonatomic, strong) NSPopUpButton *inputMethodPopup;
@property (nonatomic, strong) NSButton *spellCheckBox;
@property (nonatomic, strong) NSButton *freeMarkingBox;
@property (nonatomic, strong) NSButton *modernStyleBox;
@property (nonatomic, strong) NSButton *autoRestoreBox;
@property (nonatomic, strong) NSButton *edeModeBox;
@property (nonatomic, strong) NSButton *macroEnabledBox;
@property (nonatomic, strong) NSButton *usePreeditBox;

- (void)setupInView:(NSView *)contentView;

@end

@implementation VnKeyPreferencesController

- (void)setupInView:(NSView *)contentView {
    NSUserDefaults *defaults = [NSUserDefaults standardUserDefaults];
    CGFloat y = 320;
    CGFloat leftMargin = 20;
    CGFloat width = 340;

    /* Tiêu đề */
    NSTextField *title = [NSTextField labelWithString:@"Tùy chỉnh VnKey"];
    title.font = [NSFont boldSystemFontOfSize:16];
    title.frame = NSMakeRect(leftMargin, y, width, 24);
    [contentView addSubview:title];
    y -= 40;

    /* Kiểu gõ */
    NSTextField *imLabel = [NSTextField labelWithString:@"Kiểu gõ:"];
    imLabel.frame = NSMakeRect(leftMargin, y, 80, 20);
    [contentView addSubview:imLabel];

    self.inputMethodPopup = [[NSPopUpButton alloc]
        initWithFrame:NSMakeRect(leftMargin + 85, y - 2, 180, 26) pullsDown:NO];
    [self.inputMethodPopup addItemsWithTitles:@[
        @"Telex", @"Simple Telex", @"VNI", @"VIQR", @"Microsoft Vietnamese"
    ]];
    [self.inputMethodPopup selectItemAtIndex:[defaults integerForKey:kVnKeyInputMethod]];
    [self.inputMethodPopup setTarget:self];
    [self.inputMethodPopup setAction:@selector(inputMethodChanged:)];
    [contentView addSubview:self.inputMethodPopup];
    y -= 36;

    /* Kiểm tra chính tả */
    self.spellCheckBox = [NSButton checkboxWithTitle:@"Kiểm tra chính tả"
                                              target:self
                                              action:@selector(optionChanged:)];
    self.spellCheckBox.frame = NSMakeRect(leftMargin, y, width, 20);
    self.spellCheckBox.state = [defaults boolForKey:kVnKeySpellCheck]
                                   ? NSControlStateValueOn
                                   : NSControlStateValueOff;
    self.spellCheckBox.tag = 1;
    [contentView addSubview:self.spellCheckBox];
    y -= 28;

    /* Bỏ dấu tự do */
    self.freeMarkingBox = [NSButton checkboxWithTitle:@"Gõ dấu tự do (bỏ dấu tại vị trí bất kỳ)"
                                              target:self
                                              action:@selector(optionChanged:)];
    self.freeMarkingBox.frame = NSMakeRect(leftMargin, y, width, 20);
    self.freeMarkingBox.state = [defaults boolForKey:kVnKeyFreeMarking]
                                    ? NSControlStateValueOn
                                    : NSControlStateValueOff;
    self.freeMarkingBox.tag = 2;
    [contentView addSubview:self.freeMarkingBox];
    y -= 28;

    /* Kiểu mới */
    self.modernStyleBox = [NSButton checkboxWithTitle:@"Dùng kiểu mới (hoà → hòa)"
                                              target:self
                                              action:@selector(optionChanged:)];
    self.modernStyleBox.frame = NSMakeRect(leftMargin, y, width, 20);
    self.modernStyleBox.state = [defaults boolForKey:kVnKeyModernStyle]
                                    ? NSControlStateValueOn
                                    : NSControlStateValueOff;
    self.modernStyleBox.tag = 3;
    [contentView addSubview:self.modernStyleBox];
    y -= 28;

    /* Tự động khôi phục */
    self.autoRestoreBox = [NSButton checkboxWithTitle:@"Tự phục hồi từ không phải tiếng Việt"
                                              target:self
                                              action:@selector(optionChanged:)];
    self.autoRestoreBox.frame = NSMakeRect(leftMargin, y, width, 20);
    self.autoRestoreBox.state = [defaults boolForKey:kVnKeyAutoRestore]
                                    ? NSControlStateValueOn
                                    : NSControlStateValueOff;
    self.autoRestoreBox.tag = 4;
    [contentView addSubview:self.autoRestoreBox];
    y -= 28;

    /* Tiếng Tây Nguyên (Êđê) */
    self.edeModeBox = [NSButton checkboxWithTitle:@"Tiếng Tây Nguyên (Êđê)"
                                           target:self
                                           action:@selector(optionChanged:)];
    self.edeModeBox.frame = NSMakeRect(leftMargin, y, width, 20);
    self.edeModeBox.state = [defaults boolForKey:kVnKeyEdeMode]
                                ? NSControlStateValueOn
                                : NSControlStateValueOff;
    self.edeModeBox.tag = 5;
    [contentView addSubview:self.edeModeBox];
    y -= 28;

    /* Gõ tắt (Auto-text) */
    self.macroEnabledBox = [NSButton checkboxWithTitle:@"Gõ tắt (Auto-text)"
                                                target:self
                                                action:@selector(optionChanged:)];
    self.macroEnabledBox.frame = NSMakeRect(leftMargin, y, width, 20);
    self.macroEnabledBox.state = [defaults boolForKey:kVnKeyMacroEnabled]
                                     ? NSControlStateValueOn
                                     : NSControlStateValueOff;
    self.macroEnabledBox.tag = 6;
    [contentView addSubview:self.macroEnabledBox];
    y -= 28;

    /* Dùng preedit (gạch chân) */
    self.usePreeditBox = [NSButton checkboxWithTitle:@"Gạch chân khi gõ (preedit)"
                                              target:self
                                              action:@selector(optionChanged:)];
    self.usePreeditBox.frame = NSMakeRect(leftMargin, y, width, 20);
    self.usePreeditBox.state = [defaults boolForKey:kVnKeyUsePreedit]
                                   ? NSControlStateValueOn
                                   : NSControlStateValueOff;
    self.usePreeditBox.tag = 7;
    [contentView addSubview:self.usePreeditBox];
    y -= 40;

    /* Phiên bản */
    NSString *version = [[NSBundle mainBundle]
        objectForInfoDictionaryKey:@"CFBundleShortVersionString"];
    NSTextField *versionLabel = [NSTextField labelWithString:
        [NSString stringWithFormat:@"VnKey %@ — Bộ gõ tiếng Việt", version ?: @""]];
    versionLabel.font = [NSFont systemFontOfSize:11];
    versionLabel.textColor = [NSColor secondaryLabelColor];
    versionLabel.frame = NSMakeRect(leftMargin, y, width, 16);
    [contentView addSubview:versionLabel];
}

- (void)inputMethodChanged:(id)sender {
    NSInteger idx = self.inputMethodPopup.indexOfSelectedItem;
    [[NSUserDefaults standardUserDefaults] setInteger:idx forKey:kVnKeyInputMethod];
    [self notifyChange];
}

- (void)optionChanged:(id)sender {
    NSButton *btn = (NSButton *)sender;
    BOOL val = (btn.state == NSControlStateValueOn);
    NSUserDefaults *defaults = [NSUserDefaults standardUserDefaults];

    switch (btn.tag) {
        case 1: [defaults setBool:val forKey:kVnKeySpellCheck]; break;
        case 2: [defaults setBool:val forKey:kVnKeyFreeMarking]; break;
        case 3: [defaults setBool:val forKey:kVnKeyModernStyle]; break;
        case 4: [defaults setBool:val forKey:kVnKeyAutoRestore]; break;
        case 5: [defaults setBool:val forKey:kVnKeyEdeMode]; break;
        case 6: [defaults setBool:val forKey:kVnKeyMacroEnabled]; break;
        case 7: [defaults setBool:val forKey:kVnKeyUsePreedit]; break;
    }
    [self notifyChange];
}

- (void)notifyChange {
    [[NSUserDefaults standardUserDefaults] synchronize];
    [[NSNotificationCenter defaultCenter]
        postNotificationName:@"VnKeyPreferencesChanged" object:nil];
}

@end

/* ==================== Window factory ==================== */

/* Giữ strong reference để controller không bị dealloc */
static VnKeyPreferencesController *sPrefsController = nil;

NSWindow *createPreferencesWindow(void) {
    NSRect frame = NSMakeRect(0, 0, 400, 380);
    NSWindow *window = [[NSWindow alloc]
        initWithContentRect:frame
                  styleMask:(NSWindowStyleMaskTitled |
                             NSWindowStyleMaskClosable)
                    backing:NSBackingStoreBuffered
                      defer:NO];
    window.title = @"VnKey — Tùy chỉnh";
    [window center];
    window.releasedWhenClosed = NO;

    sPrefsController = [[VnKeyPreferencesController alloc] init];
    [sPrefsController setupInView:window.contentView];

    return window;
}
