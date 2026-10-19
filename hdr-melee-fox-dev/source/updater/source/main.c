// Project TUFF Updater: a Homebrew Menu app that installs the latest Project TUFF files from
// GitHub straight onto the SD card, so updating doesn't need the SD card in a PC.
//
// It mirrors what sd_install/install.ps1 does on a PC:
//  1. Asks GitHub for the file list of the repo's sd_install/ folder (one API call). Every
//     entry comes with its git blob SHA-1, i.e. a fingerprint of the file's contents.
//  2. Downloads only the files whose fingerprint differs from what's on the SD card, checks
//     each download against its fingerprint, backs up the file it replaces to
//     sdmc:/hdr-melee-fox-dev/backup/updater-<date>/, then moves the new one into place.
//  3. Turns on the mods listed in sd_install/enable_mods.txt in every ARCropolis preset.
// The updater itself lives in sd_install/switch/, so it updates itself too.
//
// Smash only loads mods and plugins when it starts, so close and reopen the game afterwards.
#include <dirent.h>
#include <stdarg.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

#include <switch.h>
#include <curl/curl.h>
#include <jansson.h>

#define REPO "KevinIsNotSeven/ProjectTUFF"
#define BRANCH "main"
#define PREFIX "sd_install/"
#define TREE_URL "https://api.github.com/repos/" REPO "/git/trees/" BRANCH "?recursive=1"
#define RAW_URL "https://raw.githubusercontent.com/" REPO "/" BRANCH "/"
#define USER_AGENT "ProjectTUFF-Updater/1.0"
#define WORK_DIR "sdmc:/hdr-melee-fox-dev"
#define MANIFEST WORK_DIR "/updater_manifest.txt"

// sd_install files that are for the PC installer, not for the SD card
static const char *SKIP[] = {"install.ps1", "README.txt", "enable_mods.txt"};

typedef struct {
    char path[512]; // relative to the SD root
    char sha[41];
} Entry;

static Entry *g_manifest = NULL;
static size_t g_manifest_len = 0;
static char g_backup_dir[256];

// ---- small helpers ----------------------------------------------------------------------

static void say(const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    vprintf(fmt, ap);
    va_end(ap);
    consoleUpdate(NULL);
}

static void mkdirs(const char *path) {
    // creates every directory in path up to (not including) the last component
    char tmp[1024];
    snprintf(tmp, sizeof tmp, "%s", path);
    for (char *p = tmp + strlen("sdmc:/"); *p; p++) {
        if (*p == '/') {
            *p = 0;
            mkdir(tmp, 0777);
            *p = '/';
        }
    }
}

static int file_exists(const char *path) {
    struct stat st;
    return stat(path, &st) == 0 && S_ISREG(st.st_mode);
}

static int dir_exists(const char *path) {
    struct stat st;
    return stat(path, &st) == 0 && S_ISDIR(st.st_mode);
}

static void hex(const u8 *in, size_t n, char *out) {
    for (size_t i = 0; i < n; i++) sprintf(out + 2 * i, "%02x", in[i]);
    out[2 * n] = 0;
}

/// Git's blob fingerprint of a file: SHA-1 of "blob <size>\0" + contents. 0 on success.
static int git_sha(const char *path, char out[41]) {
    FILE *f = fopen(path, "rb");
    if (!f) return -1;
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);
    Sha1Context ctx;
    sha1ContextCreate(&ctx);
    char header[64];
    int hl = snprintf(header, sizeof header, "blob %ld", size);
    sha1ContextUpdate(&ctx, header, hl + 1); // includes the terminating NUL
    static u8 buf[0x20000];
    size_t n;
    while ((n = fread(buf, 1, sizeof buf, f)) > 0) sha1ContextUpdate(&ctx, buf, n);
    fclose(f);
    u8 hash[SHA1_HASH_SIZE];
    sha1ContextGetHash(&ctx, hash);
    hex(hash, SHA1_HASH_SIZE, out);
    return 0;
}

// ---- downloads --------------------------------------------------------------------------

typedef struct {
    char *data;
    size_t len;
} Mem;

static size_t to_mem(void *ptr, size_t size, size_t nmemb, void *user) {
    Mem *m = user;
    size_t add = size * nmemb;
    char *p = realloc(m->data, m->len + add + 1);
    if (!p) return 0;
    m->data = p;
    memcpy(m->data + m->len, ptr, add);
    m->len += add;
    m->data[m->len] = 0;
    return add;
}

static CURL *new_request(const char *url) {
    CURL *c = curl_easy_init();
    curl_easy_setopt(c, CURLOPT_URL, url);
    curl_easy_setopt(c, CURLOPT_USERAGENT, USER_AGENT);
    curl_easy_setopt(c, CURLOPT_FOLLOWLOCATION, 1L);
    curl_easy_setopt(c, CURLOPT_FAILONERROR, 1L);
    curl_easy_setopt(c, CURLOPT_CONNECTTIMEOUT, 15L);
    curl_easy_setopt(c, CURLOPT_TIMEOUT, 300L);
    return c;
}

static int get_mem(const char *url, Mem *out) {
    out->data = NULL;
    out->len = 0;
    CURL *c = new_request(url);
    struct curl_slist *h = curl_slist_append(NULL, "Accept: application/vnd.github+json");
    curl_easy_setopt(c, CURLOPT_HTTPHEADER, h);
    curl_easy_setopt(c, CURLOPT_WRITEFUNCTION, to_mem);
    curl_easy_setopt(c, CURLOPT_WRITEDATA, out);
    CURLcode r = curl_easy_perform(c);
    curl_slist_free_all(h);
    curl_easy_cleanup(c);
    if (r != CURLE_OK) say("  download failed: %s\n", curl_easy_strerror(r));
    return r == CURLE_OK ? 0 : -1;
}

static int get_file(const char *url, const char *dest) {
    FILE *f = fopen(dest, "wb");
    if (!f) return -1;
    CURL *c = new_request(url);
    curl_easy_setopt(c, CURLOPT_WRITEDATA, f); // default write function = fwrite
    CURLcode r = curl_easy_perform(c);
    curl_easy_cleanup(c);
    fclose(f);
    if (r != CURLE_OK) {
        say("  download failed: %s\n", curl_easy_strerror(r));
        remove(dest);
    }
    return r == CURLE_OK ? 0 : -1;
}

/// RAW_URL + path, with each path segment URL-escaped.
static void raw_url(const char *repo_path, char *out, size_t cap) {
    snprintf(out, cap, "%s", RAW_URL);
    char seg[512];
    const char *p = repo_path;
    while (*p) {
        const char *slash = strchr(p, '/');
        size_t n = slash ? (size_t)(slash - p) : strlen(p);
        snprintf(seg, sizeof seg, "%.*s", (int)n, p);
        char *esc = curl_easy_escape(NULL, seg, 0);
        strncat(out, esc, cap - strlen(out) - 1);
        curl_free(esc);
        if (!slash) break;
        strncat(out, "/", cap - strlen(out) - 1);
        p = slash + 1;
    }
}

// ---- manifest of what this updater installed --------------------------------------------

static const char *manifest_sha(const char *path) {
    for (size_t i = 0; i < g_manifest_len; i++)
        if (!strcmp(g_manifest[i].path, path)) return g_manifest[i].sha;
    return NULL;
}

static void manifest_set(const char *path, const char *sha) {
    for (size_t i = 0; i < g_manifest_len; i++) {
        if (!strcmp(g_manifest[i].path, path)) {
            snprintf(g_manifest[i].sha, sizeof g_manifest[i].sha, "%s", sha);
            return;
        }
    }
    g_manifest = realloc(g_manifest, (g_manifest_len + 1) * sizeof(Entry));
    snprintf(g_manifest[g_manifest_len].path, sizeof g_manifest[0].path, "%s", path);
    snprintf(g_manifest[g_manifest_len].sha, sizeof g_manifest[0].sha, "%s", sha);
    g_manifest_len++;
}

static void manifest_load(void) {
    FILE *f = fopen(MANIFEST, "r");
    if (!f) return;
    char line[600];
    while (fgets(line, sizeof line, f)) {
        line[strcspn(line, "\r\n")] = 0;
        if (strlen(line) > 41 && line[40] == ' ') {
            line[40] = 0;
            manifest_set(line + 41, line);
        }
    }
    fclose(f);
}

static void manifest_save(void) {
    mkdirs(MANIFEST);
    FILE *f = fopen(MANIFEST, "w");
    if (!f) return;
    for (size_t i = 0; i < g_manifest_len; i++) fprintf(f, "%s %s\n", g_manifest[i].sha, g_manifest[i].path);
    fclose(f);
}

// ---- installing one file ----------------------------------------------------------------

/// 1 = updated, 0 = already up to date, -1 = failed
static int install_file(const char *repo_path, const char *rel, const char *sha) {
    char dest[600], tmp[620], url[1200], have[41];
    snprintf(dest, sizeof dest, "sdmc:/%s", rel);
    const char *known = manifest_sha(rel);
    if (file_exists(dest)) {
        if (known && !strcmp(known, sha)) return 0;
        if (git_sha(dest, have) == 0 && !strcmp(have, sha)) {
            manifest_set(rel, sha);
            return 0;
        }
    }
    say("  updating %s\n", rel);
    mkdirs(dest);
    snprintf(tmp, sizeof tmp, "%s.tufftmp", dest);
    raw_url(repo_path, url, sizeof url);
    if (get_file(url, tmp) != 0) return -1;
    if (git_sha(tmp, have) != 0 || strcmp(have, sha)) {
        say("    download didn't match its fingerprint, skipped\n");
        remove(tmp);
        return -1;
    }
    if (file_exists(dest)) {
        char bak[800];
        snprintf(bak, sizeof bak, "%s/%s", g_backup_dir, rel);
        mkdirs(bak);
        if (rename(dest, bak) != 0) {
            // couldn't move it aside (e.g. it's in use): replace it directly
            remove(dest);
        }
    }
    if (rename(tmp, dest) != 0) {
        say("    couldn't put the new file in place (%s)\n", strerror(errno));
        return -1;
    }
    manifest_set(rel, sha);
    return 1;
}

// ---- ARCropolis presets -----------------------------------------------------------------

static u64 hash40(const char *s) {
    size_t n = strlen(s);
    return (u64)crc32Calculate(s, n) | ((u64)n << 32);
}

static void enable_mods(const char *list) {
    const char *root = "sdmc:/ultimate/arcropolis/config";
    DIR *d1 = opendir(root);
    if (!d1) {
        say("No ARCropolis config found; turn new mods on in the ARCropolis menu.\n");
        return;
    }
    struct dirent *a, *b;
    while ((a = readdir(d1))) {
        if (a->d_name[0] == '.') continue;
        char p1[512];
        snprintf(p1, sizeof p1, "%s/%s", root, a->d_name);
        DIR *d2 = opendir(p1);
        if (!d2) continue;
        while ((b = readdir(d2))) {
            if (b->d_name[0] == '.') continue;
            char presets[800];
            snprintf(presets, sizeof presets, "%s/%s/presets", p1, b->d_name);
            if (!file_exists(presets)) continue;
            json_error_t err;
            json_t *arr = json_load_file(presets, 0, &err);
            if (!json_is_array(arr)) {
                json_decref(arr);
                continue;
            }
            int added = 0;
            char line[256];
            const char *p = list;
            while (*p) {
                size_t n = strcspn(p, "\r\n");
                snprintf(line, sizeof line, "%.*s", (int)n, p);
                p += n;
                while (*p == '\r' || *p == '\n') p++;
                char *name = line;
                while (*name == ' ' || *name == '\t') name++;
                for (char *e = name + strlen(name); e > name && (e[-1] == ' ' || e[-1] == '\t'); e--) e[-1] = 0;
                if (!*name) continue;
                char folder[512], key[512];
                snprintf(folder, sizeof folder, "sdmc:/ultimate/mods/%s", name);
                if (!dir_exists(folder)) continue;
                snprintf(key, sizeof key, "sd:/ultimate/mods/%s", name);
                json_int_t h = (json_int_t)hash40(key);
                int found = 0;
                size_t i;
                json_t *v;
                json_array_foreach(arr, i, v) if (json_is_integer(v) && json_integer_value(v) == h) found = 1;
                if (!found) {
                    json_array_append_new(arr, json_integer(h));
                    say("  turned on %s\n", name);
                    added++;
                }
            }
            if (added) {
                char bak[820];
                snprintf(bak, sizeof bak, "%s.bak-updater", presets);
                remove(bak);
                rename(presets, bak);
                json_dump_file(arr, presets, JSON_COMPACT);
            }
            json_decref(arr);
        }
        closedir(d2);
    }
    closedir(d1);
}

// ---- main -------------------------------------------------------------------------------

static int run(void) {
    say("Asking GitHub for the latest files...\n");
    Mem tree;
    if (get_mem(TREE_URL, &tree) != 0) {
        say("Couldn't reach GitHub. Check the Switch's internet connection.\n");
        return -1;
    }
    json_error_t err;
    json_t *root = json_loads(tree.data, 0, &err);
    free(tree.data);
    json_t *items = json_object_get(root, "tree");
    if (!json_is_array(items)) {
        say("GitHub sent something unexpected (rate limit? try again in a bit).\n");
        json_decref(root);
        return -1;
    }
    if (json_is_true(json_object_get(root, "truncated"))) say("Warning: GitHub's file list was cut short.\n");

    time_t now = time(NULL);
    struct tm *t = localtime(&now);
    snprintf(g_backup_dir, sizeof g_backup_dir, WORK_DIR "/backup/updater-%04d-%02d-%02d_%02d-%02d-%02d",
             t->tm_year + 1900, t->tm_mon + 1, t->tm_mday, t->tm_hour, t->tm_min, t->tm_sec);
    manifest_load();

    int updated = 0, current = 0, failed = 0;
    size_t i;
    json_t *it;
    json_array_foreach(items, i, it) {
        const char *type = json_string_value(json_object_get(it, "type"));
        const char *path = json_string_value(json_object_get(it, "path"));
        const char *sha = json_string_value(json_object_get(it, "sha"));
        if (!type || !path || !sha || strcmp(type, "blob") || strncmp(path, PREFIX, strlen(PREFIX))) continue;
        const char *rel = path + strlen(PREFIX);
        int skip = 0;
        for (size_t k = 0; k < sizeof SKIP / sizeof *SKIP; k++)
            if (!strcmp(rel, SKIP[k])) skip = 1;
        if (skip) continue;
        int r = install_file(path, rel, sha);
        if (r > 0) updated++;
        else if (r == 0) current++;
        else failed++;
    }
    json_decref(root);
    manifest_save();

    say("\nChecking which mods should be on...\n");
    Mem list;
    if (get_mem(RAW_URL PREFIX "enable_mods.txt", &list) == 0) {
        enable_mods(list.data ? list.data : "");
        free(list.data);
    }

    say("\n%d updated, %d already up to date", updated, current);
    if (failed) say(", %d FAILED (run the updater again)", failed);
    say(".\n");
    if (updated) say("Backups of replaced files: %s\n", g_backup_dir + strlen("sdmc:"));
    return failed ? -1 : 0;
}

int main(int argc, char *argv[]) {
    consoleInit(NULL);
    padConfigureInput(1, HidNpadStyleSet_NpadStandard);
    PadState pad;
    padInitializeDefault(&pad);
    socketInitializeDefault();
    curl_global_init(CURL_GLOBAL_DEFAULT);

    say("Project TUFF Updater\n====================\n\n");
    run();
    say("\nClose Smash and open it again to load the changes.\nPress + to exit.\n");

    while (appletMainLoop()) {
        padUpdate(&pad);
        if (padGetButtonsDown(&pad) & (HidNpadButton_Plus | HidNpadButton_B)) break;
        consoleUpdate(NULL);
    }
    curl_global_cleanup();
    socketExit();
    consoleExit(NULL);
    return 0;
}
