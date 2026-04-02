/*
 * vnkey-ibus — Bộ gõ tiếng Việt cho IBus
 * Sử dụng vnkey-engine (Rust) qua FFI
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#include <ibus.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <locale.h>
#include <unistd.h>
#include <ctype.h>


#include "vnkey-engine.h"

/* ==================== Phát hiện app đang focus ==================== */

/* Lấy tên exe của cửa sổ đang focus (Linux).
 * Thử xdotool, nếu không có thì trả NULL.
 * Trả chuỗi malloc, cần g_free. */
static char *get_focused_exe_name(void) {
    /* Thử xdotool để lấy PID */
    FILE *fp = popen("xdotool getactivewindow getwindowpid 2>/dev/null", "r");
    if (!fp) return NULL;
    char pidbuf[32];
    if (!fgets(pidbuf, sizeof(pidbuf), fp)) {
        pclose(fp);
        return NULL;
    }
    pclose(fp);

    pid_t pid = (pid_t)atoi(pidbuf);
    if (pid <= 0) return NULL;

    /* Đọc /proc/PID/exe */
    char procpath[64];
    snprintf(procpath, sizeof(procpath), "/proc/%d/exe", (int)pid);
    char exepath[1024];
    ssize_t len = readlink(procpath, exepath, sizeof(exepath) - 1);
    if (len <= 0) return NULL;
    exepath[len] = '\0';

    /* Lấy phần basename */
    const char *base = strrchr(exepath, '/');
    base = base ? base + 1 : exepath;

    /* Chuyển lowercase */
    char *result = g_strdup(base);
    for (char *p = result; *p; p++) *p = tolower((unsigned char)*p);
    return result;
}

/* ==================== Cấu hình ==================== */

typedef struct {
    int  input_method;    /* 0=Telex,1=SimpleTelex,2=VNI,3=VIQR */
    int  output_charset;  /* 1=UTF-8, 20=TCVN3, 40=VNI-WIN, etc. */
    int  spell_check;
    int  free_marking;
    int  modern_style;
    int  ede_mode;
} VnKeyConfig;

static VnKeyConfig g_config = {
    .input_method   = 0,
    .output_charset = 1,
    .spell_check    = 1,
    .free_marking   = 1,
    .modern_style   = 1,
    .ede_mode       = 0,
};

/* ==================== Lưu trữ cấu hình ==================== */

static char *config_dir(void) {
    const char *xdg = getenv("XDG_CONFIG_HOME");
    if (xdg && xdg[0]) {
        char *path = g_strdup_printf("%s/vnkey", xdg);
        return path;
    }
    const char *home = getenv("HOME");
    if (home && home[0]) {
        char *path = g_strdup_printf("%s/.config/vnkey", home);
        return path;
    }
    return NULL;
}

static char *config_path(void) {
    char *dir = config_dir();
    if (!dir) return NULL;
    char *path = g_strdup_printf("%s/config.json", dir);
    g_free(dir);
    return path;
}

/* Trích xuất JSON đơn giản (không phụ thuộc ngoài) */
static const char *json_get(const char *json, const char *key,
                            char *buf, size_t buflen) {
    char needle[256];
    snprintf(needle, sizeof(needle), "\"%s\"", key);
    const char *pos = strstr(json, needle);
    if (!pos) return NULL;
    pos = strchr(pos + strlen(needle), ':');
    if (!pos) return NULL;
    pos++;
    while (*pos == ' ' || *pos == '\t') pos++;
    const char *end = pos;
    while (*end && *end != ',' && *end != '}' && *end != '\n') end++;
    size_t len = end - pos;
    if (len >= buflen) len = buflen - 1;
    memcpy(buf, pos, len);
    buf[len] = '\0';
    /* bỏ khoảng trắng cuối */
    while (len > 0 && buf[len-1] <= ' ') buf[--len] = '\0';
    /* bỏ khoảng trắng đầu */
    char *p = buf;
    while (*p && *p <= ' ') p++;
    if (p != buf) memmove(buf, p, strlen(p) + 1);
    return buf;
}

static int json_get_int(const char *json, const char *key, int def) {
    char buf[64];
    if (!json_get(json, key, buf, sizeof(buf))) return def;
    return atoi(buf);
}

static int json_get_bool(const char *json, const char *key, int def) {
    char buf[64];
    if (!json_get(json, key, buf, sizeof(buf))) return def;
    if (strcmp(buf, "true") == 0) return 1;
    if (strcmp(buf, "false") == 0) return 0;
    return def;
}

/* Trích xuất JSON object con: "key": { ... }
 * Trả con trỏ tới chuỗi malloc (cần g_free), hoặc NULL */
static char *json_get_object(const char *json, const char *key) {
    char needle[256];
    snprintf(needle, sizeof(needle), "\"%s\"", key);
    const char *pos = strstr(json, needle);
    if (!pos) return NULL;
    pos = strchr(pos + strlen(needle), '{');
    if (!pos) return NULL;
    int depth = 1;
    const char *start = pos;
    pos++;
    while (*pos && depth > 0) {
        if (*pos == '{') depth++;
        else if (*pos == '}') depth--;
        pos++;
    }
    if (depth != 0) return NULL;
    size_t len = pos - start;
    return g_strndup(start, len);
}

static void load_config(void) {
    char *path = config_path();
    if (!path) return;
    gchar *contents = NULL;
    gsize length = 0;
    if (!g_file_get_contents(path, &contents, &length, NULL)) {
        g_free(path);
        return;
    }
    g_free(path);

    g_config.input_method   = json_get_int(contents, "input_method", 0);
    g_config.output_charset = json_get_int(contents, "output_charset", 1);
    g_config.spell_check    = json_get_bool(contents, "spell_check", 1);
    g_config.free_marking   = json_get_bool(contents, "free_marking", 1);
    g_config.modern_style   = json_get_bool(contents, "modern_style", 1);
    g_config.ede_mode       = json_get_bool(contents, "ede_mode", 0);

    /* Nạp app_charsets */
    char *ac_json = json_get_object(contents, "app_charsets");
    if (ac_json) {
        vnkey_app_charset_from_json(ac_json);
        g_free(ac_json);
    }

    g_free(contents);
}

static void save_config(void) {
    char *dir = config_dir();
    if (!dir) return;
    g_mkdir_with_parents(dir, 0755);
    char *path = g_strdup_printf("%s/config.json", dir);
    g_free(dir);

    char *ac_json = vnkey_app_charset_to_json();
    const char *ac_str = ac_json ? ac_json : "{}";

    char buf[4096];
    snprintf(buf, sizeof(buf),
        "{\n"
        "  \"input_method\": %d,\n"
        "  \"output_charset\": %d,\n"
        "  \"spell_check\": %s,\n"
        "  \"free_marking\": %s,\n"
        "  \"modern_style\": %s,\n"
        "  \"ede_mode\": %s,\n"
        "  \"app_charsets\": %s\n"
        "}\n",
        g_config.input_method,
        g_config.output_charset,
        g_config.spell_check ? "true" : "false",
        g_config.free_marking ? "true" : "false",
        g_config.modern_style ? "true" : "false",
        g_config.ede_mode ? "true" : "false",
        ac_str
    );
    g_file_set_contents(path, buf, -1, NULL);
    g_free(path);

    if (ac_json) vnkey_app_charset_free_string(ac_json);
}

/* ==================== Trợ giúp bảng mã ==================== */

/* Bảng mã có đầu ra là UTF-8 / ASCII hợp lệ */
static int is_utf8_charset(int id) {
    return id == 1 || id == 2 || id == 3 || id == 4 ||
           id == 6 || id == 10 || id == 11;
}

/* Nâng byte thô (Latin-1) lên UTF-8 cho font tiếng Việt cũ */
static char *bytes_to_utf8(const uint8_t *data, size_t len, size_t *out_len) {
    char *out = g_malloc(len * 2 + 1);
    size_t j = 0;
    for (size_t i = 0; i < len; i++) {
        uint8_t ch = data[i];
        if (ch < 0x80) {
            out[j++] = (char)ch;
        } else {
            out[j++] = (char)(0xC0 | (ch >> 6));
            out[j++] = (char)(0x80 | (ch & 0x3F));
        }
    }
    out[j] = '\0';
    if (out_len) *out_len = j;
    return out;
}

/* Ngược lại của bytes_to_utf8: giải mã UTF-8 thành byte thô (phạm vi Latin-1) */
static uint8_t *utf8_to_bytes(const char *s, size_t slen, size_t *out_len) {
    uint8_t *result = g_malloc(slen + 1);
    size_t j = 0, i = 0;
    while (i < slen) {
        unsigned char c = (unsigned char)s[i];
        uint32_t cp;
        if (c < 0x80) { cp = c; i += 1; }
        else if ((c & 0xE0) == 0xC0) {
            cp = (c & 0x1Fu) << 6;
            if (i + 1 < slen) cp |= ((unsigned char)s[i+1] & 0x3Fu);
            i += 2;
        } else if ((c & 0xF0) == 0xE0) {
            cp = (c & 0x0Fu) << 12;
            if (i + 1 < slen) cp |= ((unsigned char)s[i+1] & 0x3Fu) << 6;
            if (i + 2 < slen) cp |= ((unsigned char)s[i+2] & 0x3Fu);
            i += 3;
        } else {
            cp = (c & 0x07u) << 18;
            if (i + 1 < slen) cp |= ((unsigned char)s[i+1] & 0x3Fu) << 12;
            if (i + 2 < slen) cp |= ((unsigned char)s[i+2] & 0x3Fu) << 6;
            if (i + 3 < slen) cp |= ((unsigned char)s[i+3] & 0x3Fu);
            i += 4;
        }
        if (cp < 256) result[j++] = (uint8_t)cp;
    }
    result[j] = 0;
    if (out_len) *out_len = j;
    return result;
}

/* ==================== Trợ giúp clipboard ==================== */

/* Đọc văn bản clipboard hệ thống â thử từng công cụ cho đến khi thành công */
static char *get_clipboard(size_t *out_len) {
    const char *cmds[] = {
        "wl-paste --no-newline 2>/dev/null",
        "xclip -selection clipboard -o 2>/dev/null",
        "xsel --clipboard --output 2>/dev/null",
    };
    for (int c = 0; c < 3; c++) {
        FILE *fp = popen(cmds[c], "r");
        if (!fp) continue;
        GString *result = g_string_new(NULL);
        char buf[4096];
        size_t n;
        while ((n = fread(buf, 1, sizeof(buf), fp)) > 0)
            g_string_append_len(result, buf, n);
        int status = pclose(fp);
        if (status == 0 && result->len > 0) {
            if (out_len) *out_len = result->len;
            return g_string_free(result, FALSE);
        }
        g_string_free(result, TRUE);
    }
    if (out_len) *out_len = 0;
    return NULL;
}

/* Ghi văn bản vào clipboard hệ thống â thử từng công cụ cho đến khi thành công */
static int set_clipboard(const char *text, size_t len) {
    const char *cmds[] = {
        "wl-copy 2>/dev/null",
        "xclip -selection clipboard -i 2>/dev/null",
        "xsel --clipboard --input 2>/dev/null",
    };
    for (int c = 0; c < 3; c++) {
        FILE *fp = popen(cmds[c], "w");
        if (!fp) continue;
        fwrite(text, 1, len, fp);
        if (pclose(fp) == 0) return 1;
    }
    return 0;
}

/* Chuyển mã clipboard giữa Unicode và bảng mã cũ */
static void convert_clipboard(int to_unicode) {
    int cs = g_config.output_charset;
    if (cs == 1) return; /* UTF-8 â UTF-8 không cần chuyển */

    size_t clip_len = 0;
    char *clip = get_clipboard(&clip_len);
    if (!clip || clip_len == 0) {
        g_free(clip);
        return;
    }

    char *result = NULL;
    size_t result_len = 0;

    if (to_unicode) {
        /* Clipboard chứa văn bản mã cũ â chuyển sang Unicode UTF-8 */
        if (is_utf8_charset(cs)) {
            size_t buf_size = clip_len * 4 + 1024;
            uint8_t *buf = g_malloc(buf_size);
            size_t actual_len = 0;
            int ret = vnkey_charset_to_utf8(
                (const uint8_t *)clip, clip_len, cs,
                buf, buf_size, &actual_len);
            if (ret == 0 && actual_len > 0) {
                result = (char *)buf;
                result_len = actual_len;
            } else {
                g_free(buf);
            }
        } else {
            /* Bảng mã cũ: chuyển ngược UTF-8 thành byte thô trước */
            size_t bytes_len = 0;
            uint8_t *bytes = utf8_to_bytes(clip, clip_len, &bytes_len);
            size_t buf_size = bytes_len * 4 + 1024;
            uint8_t *buf = g_malloc(buf_size);
            size_t actual_len = 0;
            int ret = vnkey_charset_to_utf8(
                bytes, bytes_len, cs,
                buf, buf_size, &actual_len);
            g_free(bytes);
            if (ret == 0 && actual_len > 0) {
                result = (char *)buf;
                result_len = actual_len;
            } else {
                g_free(buf);
            }
        }
    } else {
        /* Clipboard chứa Unicode â chuyển sang bảng mã đích */
        size_t buf_size = clip_len * 4 + 1024;
        uint8_t *buf = g_malloc(buf_size);
        size_t actual_len = 0;
        int ret = vnkey_charset_from_utf8(
            (const uint8_t *)clip, clip_len, cs,
            buf, buf_size, &actual_len);
        if (ret == 0 && actual_len > 0) {
            if (is_utf8_charset(cs)) {
                result = (char *)buf;
                result_len = actual_len;
            } else {
                result = bytes_to_utf8(buf, actual_len, &result_len);
                g_free(buf);
            }
        } else {
            g_free(buf);
        }
    }

    if (result && result_len > 0) {
        set_clipboard(result, result_len);
        g_message("vnkey: clipboard converted (%s), %zu bytes",
                  to_unicode ? "to Unicode" : "from Unicode", result_len);
    }
    g_free(result);
    g_free(clip);
}

/* ==================== Engine IBus ==================== */

typedef struct _VnkIBusEngine VnkIBusEngine;
typedef struct _VnkIBusEngineClass VnkIBusEngineClass;

struct _VnkIBusEngine {
    IBusEngine parent;
    VnKeyEngine *engine;
    gchar preedit[4096];
    size_t preedit_len;   /* độ dài byte */
    gboolean viet_mode;
    gboolean is_terminal;
};

struct _VnkIBusEngineClass {
    IBusEngineClass parent;
};

#define VNK_TYPE_IBUS_ENGINE (vnk_ibus_engine_get_type())
GType vnk_ibus_engine_get_type(void);
G_DEFINE_TYPE(VnkIBusEngine, vnk_ibus_engine, IBUS_TYPE_ENGINE)

static IBusEngineClass *parent_class = NULL;

/* Khai báo trước */
static void vnk_ibus_engine_init(VnkIBusEngine *self);
static void vnk_ibus_engine_class_init(VnkIBusEngineClass *klass);
static void vnk_ibus_engine_destroy(IBusObject *obj);
static gboolean vnk_ibus_engine_process_key_event(IBusEngine *engine,
    guint keyval, guint keycode, guint state);
static void vnk_ibus_engine_focus_in(IBusEngine *engine);
static void vnk_ibus_engine_focus_out(IBusEngine *engine);
static void vnk_ibus_engine_reset(IBusEngine *engine);
static void vnk_ibus_engine_enable(IBusEngine *engine);
static void vnk_ibus_engine_disable(IBusEngine *engine);
static void vnk_ibus_engine_property_activate(IBusEngine *engine,
    const gchar *prop_name, guint prop_state);

/* ==================== Trợ giúp engine ==================== */

/* Đếm số ký tự UTF-8 trong buffer byte */
static guint utf8_char_count(const char *s, size_t byte_len) {
    guint count = 0;
    for (size_t i = 0; i < byte_len; ) {
        unsigned char c = (unsigned char)s[i];
        if (c < 0x80) i += 1;
        else if ((c & 0xE0) == 0xC0) i += 2;
        else if ((c & 0xF0) == 0xE0) i += 3;
        else i += 4;
        count++;
    }
    return count;
}

static void sync_settings(VnkIBusEngine *self) {
    vnkey_engine_set_input_method(self->engine, g_config.input_method);
    vnkey_engine_set_viet_mode(self->engine, self->viet_mode ? 1 : 0);
    vnkey_engine_set_options(self->engine,
        g_config.free_marking,
        g_config.modern_style,
        g_config.spell_check,
        1 /* auto_restore */,
        g_config.ede_mode);
}

static void clear_preedit(VnkIBusEngine *self) {
    self->preedit[0] = '\0';
    self->preedit_len = 0;
}

static void update_preedit_display(VnkIBusEngine *self) {
    IBusEngine *engine = IBUS_ENGINE(self);
    if (!self->is_terminal) return; /* Chỉ hiển thị preedit cho terminal */
    if (self->preedit_len == 0) {
        ibus_engine_hide_preedit_text(engine);
        return;
    }
    IBusText *text = ibus_text_new_from_string(self->preedit);
    ibus_text_append_attribute(text, IBUS_ATTR_TYPE_UNDERLINE,
                               IBUS_ATTR_UNDERLINE_SINGLE, 0,
                               g_utf8_strlen(self->preedit, -1));
    ibus_engine_update_preedit_text(engine, text, g_utf8_strlen(self->preedit, -1), TRUE);
}

/* Xóa n ký tự UTF-8 từ cuối preedit */
static void preedit_remove_chars(VnkIBusEngine *self, size_t n) {
    for (size_t i = 0; i < n; i++) {
        if (self->preedit_len == 0) break;
        size_t len = self->preedit_len;
        /* Lùi qua các byte tiếp nối */
        while (len > 0 && ((unsigned char)self->preedit[len - 1] & 0xC0) == 0x80)
            len--;
        /* Xóa byte dẫn đầu */
        if (len > 0) len--;
        self->preedit[len] = '\0';
        self->preedit_len = len;
    }
}

/* Thêm byte UTF-8 vào preedit */
static void preedit_append(VnkIBusEngine *self, const uint8_t *data, size_t len) {
    if (self->preedit_len + len < sizeof(self->preedit) - 1) {
        memcpy(self->preedit + self->preedit_len, data, len);
        self->preedit_len += len;
        self->preedit[self->preedit_len] = '\0';
    }
}

/* Commit văn bản UTF-8 trực tiếp (chế độ không preedit) */
static void direct_commit(VnkIBusEngine *self, const char *utf8, size_t len) {
    IBusEngine *engine = IBUS_ENGINE(self);
    int charset = g_config.output_charset;
    /* Kiểm tra per-app charset override */
    int app_cs = vnkey_app_charset_get_current();
    if (app_cs >= 0) charset = app_cs;
    if (charset == 1) {
        char *s = g_strndup(utf8, len);
        IBusText *text = ibus_text_new_from_string(s);
        ibus_engine_commit_text(engine, text);
        g_free(s);
    } else {
        uint8_t buf[4096];
        size_t actual_len = 0;
        int ret = vnkey_charset_from_utf8(
            (const uint8_t *)utf8, len,
            charset, buf, sizeof(buf), &actual_len);
        if (ret == 0 && actual_len > 0) {
            if (is_utf8_charset(charset)) {
                char *s = g_strndup((const char *)buf, actual_len);
                IBusText *text = ibus_text_new_from_string(s);
                ibus_engine_commit_text(engine, text);
                g_free(s);
            } else {
                size_t utf8_len;
                char *s = bytes_to_utf8(buf, actual_len, &utf8_len);
                IBusText *text = ibus_text_new_from_string(s);
                ibus_engine_commit_text(engine, text);
                g_free(s);
            }
        } else {
            char *s = g_strndup(utf8, len);
            IBusText *text = ibus_text_new_from_string(s);
            ibus_engine_commit_text(engine, text);
            g_free(s);
        }
    }
}

static void commit_preedit_ex(VnkIBusEngine *self, int soft) {
    if (self->preedit_len > 0) {
        g_message("vnkey: commit preedit '%s' (len=%zu)", self->preedit, self->preedit_len);
        direct_commit(self, self->preedit, self->preedit_len);
        clear_preedit(self);
    }
    if (soft)
        vnkey_engine_soft_reset(self->engine);
    else
        vnkey_engine_reset(self->engine);
}

static void commit_preedit(VnkIBusEngine *self) {
    commit_preedit_ex(self, 0);
}

/* ==================== Danh sách thuộc tính (menu chuột phải) ==================== */

/* Tên kiểu gõ */
static const char *IM_NAMES[] = {"Telex", "Simple Telex", "VNI", "VIQR"};
static const int IM_COUNT = 4;

/* Mục bảng mã: {id, nhãn} */
static const struct { int id; const char *label; } CHARSETS[] = {
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
    {25, "VNU"},
    {6,  "Unicode C String"},
};
static const int CS_COUNT = sizeof(CHARSETS) / sizeof(CHARSETS[0]);

static IBusPropList *build_prop_list(void) {
    IBusPropList *props = ibus_prop_list_new();

    /* Menu con kiểu gõ */
    IBusPropList *im_menu = ibus_prop_list_new();
    for (int i = 0; i < IM_COUNT; i++) {
        char name[64];
        snprintf(name, sizeof(name), "im-%d", i);
        IBusText *label = ibus_text_new_from_printf("IM: %s", IM_NAMES[i]);
        IBusProperty *prop = ibus_property_new(
            name, PROP_TYPE_RADIO, label, NULL, NULL,
            TRUE, TRUE,
            (i == g_config.input_method) ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED,
            NULL);
        ibus_prop_list_append(im_menu, prop);
    }
    IBusText *im_label = ibus_text_new_from_string("Input Method");
    IBusProperty *im_prop = ibus_property_new(
        "im-menu", PROP_TYPE_MENU, im_label, NULL, NULL,
        TRUE, TRUE, PROP_STATE_UNCHECKED, im_menu);
    ibus_prop_list_append(props, im_prop);

    /* Menu con bảng mã */
    IBusPropList *cs_menu = ibus_prop_list_new();
    for (int i = 0; i < CS_COUNT; i++) {
        char name[64];
        snprintf(name, sizeof(name), "cs-%d", CHARSETS[i].id);
        IBusText *label = ibus_text_new_from_printf("CS: %s", CHARSETS[i].label);
        IBusProperty *prop = ibus_property_new(
            name, PROP_TYPE_RADIO, label, NULL, NULL,
            TRUE, TRUE,
            (CHARSETS[i].id == g_config.output_charset) ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED,
            NULL);
        ibus_prop_list_append(cs_menu, prop);
    }
    IBusText *cs_label = ibus_text_new_from_string("Charset");
    IBusProperty *cs_prop = ibus_property_new(
        "cs-menu", PROP_TYPE_MENU, cs_label, NULL, NULL,
        TRUE, TRUE, PROP_STATE_UNCHECKED, cs_menu);
    ibus_prop_list_append(props, cs_prop);

    /* Tùy chọn */
    IBusText *spell_label = ibus_text_new_from_string("Spell check");
    IBusProperty *spell_prop = ibus_property_new(
        "opt-spell", PROP_TYPE_TOGGLE, spell_label, NULL, NULL,
        TRUE, TRUE,
        g_config.spell_check ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED,
        NULL);
    ibus_prop_list_append(props, spell_prop);

    IBusText *free_label = ibus_text_new_from_string("Free tone marking");
    IBusProperty *free_prop = ibus_property_new(
        "opt-free", PROP_TYPE_TOGGLE, free_label, NULL, NULL,
        TRUE, TRUE,
        g_config.free_marking ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED,
        NULL);
    ibus_prop_list_append(props, free_prop);

    IBusText *modern_label = ibus_text_new_from_string("Modern style (oÃ , uÃ½)");
    IBusProperty *modern_prop = ibus_property_new(
        "opt-modern", PROP_TYPE_TOGGLE, modern_label, NULL, NULL,
        TRUE, TRUE,
        g_config.modern_style ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED,
        NULL);
    ibus_prop_list_append(props, modern_prop);

    IBusText *ede_label = ibus_text_new_from_string(
        "T\xc3\xa2y Nguy\xc3\xaan \xe2\x80\x93 \xc3\x8a\xc4\x91\xc3\xaa");
    IBusProperty *ede_prop = ibus_property_new(
        "opt-ede", PROP_TYPE_TOGGLE, ede_label, NULL, NULL,
        TRUE, TRUE,
        g_config.ede_mode ? PROP_STATE_CHECKED : PROP_STATE_UNCHECKED,
        NULL);
    ibus_prop_list_append(props, ede_prop);

    /* Thao tác chuyển mã clipboard */
    const char *cs_name = "Unicode (UTF-8)";
    for (int i = 0; i < CS_COUNT; i++) {
        if (CHARSETS[i].id == g_config.output_charset) {
            cs_name = CHARSETS[i].label;
            break;
        }
    }

    char clip_label_buf[256];
    snprintf(clip_label_buf, sizeof(clip_label_buf),
             "%s \xe2\x86\x92 Unicode (clipboard)", cs_name);
    IBusText *clip_to_label = ibus_text_new_from_string(clip_label_buf);
    IBusProperty *clip_to_prop = ibus_property_new(
        "clip-to-uni", PROP_TYPE_NORMAL, clip_to_label, NULL, NULL,
        TRUE, TRUE, PROP_STATE_UNCHECKED, NULL);
    ibus_prop_list_append(props, clip_to_prop);

    snprintf(clip_label_buf, sizeof(clip_label_buf),
             "Unicode \xe2\x86\x92 %s (clipboard)", cs_name);
    IBusText *clip_from_label = ibus_text_new_from_string(clip_label_buf);
    IBusProperty *clip_from_prop = ibus_property_new(
        "clip-from-uni", PROP_TYPE_NORMAL, clip_from_label, NULL, NULL,
        TRUE, TRUE, PROP_STATE_UNCHECKED, NULL);
    ibus_prop_list_append(props, clip_from_prop);

    return props;
}

/* ==================== Vòng đời GObject ==================== */

static void vnk_ibus_engine_class_init(VnkIBusEngineClass *klass) {
    IBusObjectClass *ibus_obj_class = IBUS_OBJECT_CLASS(klass);
    IBusEngineClass *engine_class   = IBUS_ENGINE_CLASS(klass);

    parent_class = (IBusEngineClass *)g_type_class_peek_parent(klass);

    ibus_obj_class->destroy = vnk_ibus_engine_destroy;

    engine_class->process_key_event = vnk_ibus_engine_process_key_event;
    engine_class->focus_in          = vnk_ibus_engine_focus_in;
    engine_class->focus_out         = vnk_ibus_engine_focus_out;
    engine_class->reset             = vnk_ibus_engine_reset;
    engine_class->enable            = vnk_ibus_engine_enable;
    engine_class->disable           = vnk_ibus_engine_disable;
    engine_class->property_activate = vnk_ibus_engine_property_activate;
}

static void register_properties(VnkIBusEngine *self) {
    IBusPropList *props = build_prop_list();
    ibus_engine_register_properties(IBUS_ENGINE(self), props);
    g_object_unref(props);
}

static void vnk_ibus_engine_init(VnkIBusEngine *self) {
    self->engine = vnkey_engine_new();
    self->viet_mode = TRUE;
    self->is_terminal = FALSE;
    clear_preedit(self);
    sync_settings(self);
    g_message("vnkey: engine init, engine=%p, viet_mode=%d, im=%d",
              (void *)self->engine, self->viet_mode, g_config.input_method);
}

static void vnk_ibus_engine_destroy(IBusObject *obj) {
    VnkIBusEngine *self = (VnkIBusEngine *)obj;
    if (self->engine) {
        vnkey_engine_free(self->engine);
        self->engine = NULL;
    }
    IBUS_OBJECT_CLASS(parent_class)->destroy(obj);
}

/* ==================== Callback engine ==================== */

static void vnk_ibus_engine_enable(IBusEngine *engine) {
    VnkIBusEngine *self = (VnkIBusEngine *)engine;
    self->viet_mode = TRUE;
    sync_settings(self);
    vnkey_engine_reset(self->engine);
    clear_preedit(self);
    register_properties(self);
    parent_class->enable(engine);
}

static void vnk_ibus_engine_disable(IBusEngine *engine) {
    VnkIBusEngine *self = (VnkIBusEngine *)engine;
    commit_preedit(self);
    vnkey_engine_reset(self->engine);
    clear_preedit(self);
    parent_class->disable(engine);
}

static gboolean is_terminal_program(const char *name) {
    if (!name) return FALSE;
    static const char *terminals[] = {
        "alacritty", "kitty", "foot", "gnome-terminal-server",
        "konsole", "xterm", "urxvt", "wezterm-gui", "wezterm",
        "st", "terminator", "tilix", "xfce4-terminal", "mate-terminal",
        "lxterminal", "sakura", "terminology", "cool-retro-term",
        "deepin-terminal", "qterminal", "guake", "yakuake",
        "hyper", "tabby", "rio", "ghostty", "contour",
        NULL
    };
    for (const char **t = terminals; *t; t++) {
        if (strcasecmp(name, *t) == 0) return TRUE;
    }
    return FALSE;
}

static void vnk_ibus_engine_focus_in(IBusEngine *engine) {
    VnkIBusEngine *self = (VnkIBusEngine *)engine;
    sync_settings(self);
    register_properties(self);

    /* Cập nhật charset override và phát hiện terminal */
    char *exe = get_focused_exe_name();
    vnkey_app_charset_update(exe);
    self->is_terminal = is_terminal_program(exe);
    g_message("vnkey: focus_in app=%s terminal=%d", exe ? exe : "(null)", self->is_terminal);
    g_free(exe);

    parent_class->focus_in(engine);
}

static void vnk_ibus_engine_focus_out(IBusEngine *engine) {
    VnkIBusEngine *self = (VnkIBusEngine *)engine;
    commit_preedit(self);
    vnkey_engine_reset(self->engine);
    clear_preedit(self);
    parent_class->focus_out(engine);
}

static void vnk_ibus_engine_reset(IBusEngine *engine) {
    VnkIBusEngine *self = (VnkIBusEngine *)engine;
    commit_preedit(self);
    vnkey_engine_reset(self->engine);
    clear_preedit(self);
    parent_class->reset(engine);
}

static void vnk_ibus_engine_property_activate(IBusEngine *engine,
    const gchar *prop_name, guint prop_state) {
    VnkIBusEngine *self = (VnkIBusEngine *)engine;

    g_message("vnkey: property_activate '%s' state=%u", prop_name, prop_state);

    /* Chọn kiểu gõ: im-0, im-1, ... */
    if (g_str_has_prefix(prop_name, "im-") && strcmp(prop_name, "im-menu") != 0) {
        int im = atoi(prop_name + 3);
        if (im >= 0 && im < IM_COUNT) {
            g_config.input_method = im;
            sync_settings(self);
            save_config();
            g_message("vnkey: input_method changed to %d", im);
        }
    }
    /* Chọn bảng mã: cs-1, cs-20, ... */
    else if (g_str_has_prefix(prop_name, "cs-") && strcmp(prop_name, "cs-menu") != 0) {
        int cs = atoi(prop_name + 3);
        g_config.output_charset = cs;
        save_config();
        g_message("vnkey: output_charset changed to %d", cs);
    }
    /* Tùy chọn */
    else if (strcmp(prop_name, "opt-spell") == 0) {
        g_config.spell_check = !g_config.spell_check;
        sync_settings(self);
        save_config();
    }
    else if (strcmp(prop_name, "opt-free") == 0) {
        g_config.free_marking = !g_config.free_marking;
        sync_settings(self);
        save_config();
    }
    else if (strcmp(prop_name, "opt-modern") == 0) {
        g_config.modern_style = !g_config.modern_style;
        sync_settings(self);
        save_config();
    }
    else if (strcmp(prop_name, "opt-ede") == 0) {
        g_config.ede_mode = !g_config.ede_mode;
        sync_settings(self);
        save_config();
    }
    /* Chuyển mã clipboard */
    else if (strcmp(prop_name, "clip-to-uni") == 0) {
        convert_clipboard(1);
    }
    else if (strcmp(prop_name, "clip-from-uni") == 0) {
        convert_clipboard(0);
    }

    /* Đăng ký lại thuộc tính để cập nhật trạng thái checked */
    register_properties(self);
}

/* Đọc surrounding text để khôi phục ngữ cảnh khi preedit rỗng.
 * Ví dụ: đã commit "giá", xóa "iá", gõ tiếp → engine cần biết chữ 'g'. */
static void try_surrounding_context(VnkIBusEngine *self) {
    if (self->preedit_len > 0)
        return;
    if (!vnkey_engine_at_word_beginning(self->engine))
        return;

    IBusText *text = NULL;
    guint cursor_pos = 0;
    guint anchor_pos = 0;
    ibus_engine_get_surrounding_text((IBusEngine *)self,
                                     &text, &cursor_pos, &anchor_pos);
    if (!text)
        return;

    const gchar *str = ibus_text_get_text(text);
    if (!str || !str[0]) {
        g_object_unref(text);
        return;
    }

    /* Tìm vị trí byte tương ứng cursor_pos (đơn vị ký tự) */
    size_t byte_offset = 0;
    for (guint i = 0; i < cursor_pos && str[byte_offset]; i++) {
        byte_offset += g_utf8_skip[(unsigned char)str[byte_offset]];
    }

    /* Quét ngược lấy tối đa 10 ký tự ASCII alpha liên tiếp */
    size_t start = byte_offset;
    int count = 0;
    while (start > 0 && count < 10) {
        unsigned char ch = (unsigned char)str[start - 1];
        if ((ch >= 'A' && ch <= 'Z') || (ch >= 'a' && ch <= 'z')) {
            start--;
            count++;
        } else {
            break;
        }
    }

    if (count > 0) {
        char ctx[16];
        size_t len = byte_offset - start;
        if (len > sizeof(ctx) - 1) len = sizeof(ctx) - 1;
        memcpy(ctx, str + start, len);
        ctx[len] = '\0';
        vnkey_engine_feed_context(self->engine, ctx);
    }

    g_object_unref(text);
}

static gboolean vnk_ibus_engine_process_key_event(IBusEngine *engine,
    guint keyval, guint keycode, guint state) {
    VnkIBusEngine *self = (VnkIBusEngine *)engine;

    /* Bỏ qua nhả phím */
    if (state & IBUS_RELEASE_MASK)
        return FALSE;

    /* Bật/tắt tiếng Việt: Ctrl+Space */
    if (keyval == IBUS_KEY_space && (state & IBUS_CONTROL_MASK)) {
        self->viet_mode = !self->viet_mode;
        vnkey_engine_set_viet_mode(self->engine, self->viet_mode ? 1 : 0);
        g_message("vnkey: toggle viet_mode=%d", self->viet_mode);
        register_properties(self);
        return TRUE;
    }

    /* Cho qua các phím có Ctrl hoặc Alt (trừ Shift) */
    if (state & (IBUS_CONTROL_MASK | IBUS_MOD1_MASK)) {
        commit_preedit(self);
        return FALSE;
    }

    /* Enter, Escape, Tab: commit preedit và cho qua */
    if (keyval == IBUS_KEY_Return || keyval == IBUS_KEY_KP_Enter ||
        keyval == IBUS_KEY_Escape || keyval == IBUS_KEY_Tab) {
        commit_preedit(self);
        return FALSE;
    }

    /* Dấu cách: commit preedit + soft reset để backspace khôi phục */
    if (keyval == IBUS_KEY_space) {
        commit_preedit_ex(self, 1);
        return FALSE;
    }

    /* Xử lý Backspace */
    if (keyval == IBUS_KEY_BackSpace) {
        if (self->is_terminal) {
            /* Chế độ preedit cho terminal */
            if (self->preedit_len == 0)
                return FALSE; /* không có gì, để app xử lý */

            uint8_t buf[256];
            size_t actual_len = 0;
            size_t backspaces = 0;
            int processed = vnkey_engine_backspace(
                self->engine, buf, sizeof(buf), &actual_len, &backspaces, NULL);

            if (processed && (backspaces > 0 || actual_len > 0)) {
                preedit_remove_chars(self, backspaces);
                if (actual_len > 0)
                    preedit_append(self, buf, actual_len);
                update_preedit_display(self);
                return TRUE;
            }
            /* Xóa 1 ký tự UTF-8 cuối */
            preedit_remove_chars(self, 1);
            update_preedit_display(self);
            return TRUE;
        }

        /* Chế độ commit trực tiếp (ứng dụng GUI) */
        uint8_t buf[256];
        size_t actual_len = 0;
        size_t backspaces = 0;
        int processed = vnkey_engine_backspace(
            self->engine, buf, sizeof(buf), &actual_len, &backspaces, NULL);

        if (processed && (backspaces > 0 || actual_len > 0)) {
            if (backspaces > 0) {
                ibus_engine_delete_surrounding_text(
                    IBUS_ENGINE(self), -(gint)backspaces, (guint)backspaces);
            }
            if (actual_len > 0) {
                direct_commit(self, (const char *)buf, actual_len);
            }
            return TRUE;
        }
        return FALSE;
    }

    /* Phím ASCII in được */
    if (keyval >= IBUS_KEY_exclam && keyval <= IBUS_KEY_asciitilde) {
        uint8_t buf[256];
        size_t actual_len = 0;
        size_t backspaces = 0;

        if (!self->is_terminal) {
            try_surrounding_context(self);
        }

        int processed = vnkey_engine_process(
            self->engine, (uint32_t)keyval,
            buf, sizeof(buf), &actual_len, &backspaces, NULL);

        g_message("vnkey: key '%c' (0x%x) -> processed=%d bs=%zu out=%zu terminal=%d",
                  (char)keyval, keyval, processed, backspaces, actual_len, self->is_terminal);

        if (self->is_terminal) {
            /* Chế độ preedit cho terminal */
            if (processed) {
                preedit_remove_chars(self, backspaces);
                if (actual_len > 0)
                    preedit_append(self, buf, actual_len);
            } else {
                uint8_t ch = (uint8_t)keyval;
                preedit_append(self, &ch, 1);
            }
            if (vnkey_engine_at_word_beginning(self->engine)) {
                commit_preedit(self);
            } else {
                update_preedit_display(self);
            }
            return TRUE;
        }

        /* Chế độ commit trực tiếp (ứng dụng GUI) */
        if (processed) {
            if (backspaces > 0) {
                ibus_engine_delete_surrounding_text(
                    IBUS_ENGINE(self), -(gint)backspaces, (guint)backspaces);
            }
            if (actual_len > 0) {
                direct_commit(self, (const char *)buf, actual_len);
            }
        } else {
            char ch = (char)keyval;
            direct_commit(self, &ch, 1);
        }

        if (vnkey_engine_at_word_beginning(self->engine)) {
            vnkey_engine_reset(self->engine);
        }
        return TRUE;
    }

    /* Phím không in được / không ASCII: commit preedit và cho qua */
    commit_preedit(self);
    return FALSE;
}

/* ==================== Cài đặt component IBus ==================== */

static IBusBus *bus = NULL;
static IBusFactory *factory = NULL;

static void on_bus_disconnected(IBusBus *bus, gpointer user_data) {
    (void)bus; (void)user_data;
    ibus_quit();
}

static void start_component(void) {
    ibus_init();

    bus = ibus_bus_new();
    if (!ibus_bus_is_connected(bus)) {
        g_printerr("vnkey-ibus: cannot connect to IBus daemon\n");
        exit(1);
    }

    g_signal_connect(bus, "disconnected",
                     G_CALLBACK(on_bus_disconnected), NULL);

    factory = ibus_factory_new(ibus_bus_get_connection(bus));
    ibus_factory_add_engine(factory, "vnkey", VNK_TYPE_IBUS_ENGINE);

    if (!ibus_bus_request_name(bus, "org.freedesktop.IBus.VnKey", 0)) {
        g_printerr("vnkey-ibus: cannot request IBus name\n");
        exit(1);
    }

    ibus_main();
}

/* ==================== Hàm chính ==================== */

static gboolean opt_ibus = FALSE;

static const GOptionEntry options[] = {
    {"ibus", 'i', 0, G_OPTION_ARG_NONE, &opt_ibus,
     "Component is executed by IBus", NULL},
    {NULL}
};

int main(int argc, char *argv[]) {
    setlocale(LC_ALL, "");

    GError *error = NULL;
    GOptionContext *context = g_option_context_new("- VnKey Vietnamese IME for IBus");
    g_option_context_add_main_entries(context, options, "vnkey-ibus");
    if (!g_option_context_parse(context, &argc, &argv, &error)) {
        g_printerr("Option parsing failed: %s\n", error->message);
        g_error_free(error);
        exit(1);
    }
    g_option_context_free(context);

    load_config();
    start_component();
    return 0;
}
