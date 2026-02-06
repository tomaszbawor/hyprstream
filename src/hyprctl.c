/* SPDX-License-Identifier: MIT */

#include "hyprstream.h"

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>

struct hs_ws_loc {
    char name[HS_MAX_NAME];
    char monitor[HS_MAX_NAME];
};

static const char *bounded_strstr(const char *hay, const char *needle, const char *end)
{
    size_t nlen = strlen(needle);
    if (nlen == 0)
        return hay;

    for (const char *p = hay; p && p + nlen <= end; p++) {
        if (*p == needle[0] && strncmp(p, needle, nlen) == 0)
            return p;
    }
    return NULL;
}

static int json_read_string_in_object(const char *obj_start,
                                      const char *obj_end,
                                      const char *key,
                                      char *out,
                                      size_t out_len)
{
    const char *p = bounded_strstr(obj_start, key, obj_end);
    if (!p)
        return -1;

    p = strchr(p, ':');
    if (!p || p >= obj_end)
        return -1;
    p++;

    while (p < obj_end && (*p == ' ' || *p == '\t' || *p == '"'))
        p++;

    size_t i = 0;
    while (p < obj_end && *p && *p != '"' && i + 1 < out_len)
        out[i++] = *p++;
    out[i] = '\0';

    return (i > 0) ? 0 : -1;
}

static int parse_workspaces_json(const char *json, struct hs_ws_loc *out, size_t out_cap)
{
    size_t count = 0;
    const char *p = json;

    while (p && (p = strchr(p, '{')) != NULL) {
        const char *end = strchr(p, '}');
        if (!end)
            break;

        if (count >= out_cap)
            break;

        struct hs_ws_loc loc;
        memset(&loc, 0, sizeof(loc));

        if (json_read_string_in_object(p, end, "\"name\"", loc.name, sizeof(loc.name)) == 0 &&
            json_read_string_in_object(p, end, "\"monitor\"", loc.monitor, sizeof(loc.monitor)) == 0) {
            out[count++] = loc;
        }

        p = end + 1;
    }

    return (int)count;
}

static const char *find_monitor_for_workspace(const struct hs_ws_loc *locs,
                                              int n,
                                              const char *workspace)
{
    for (int i = 0; i < n; i++) {
        if (strcmp(locs[i].name, workspace) == 0)
            return locs[i].monitor;
    }
    return NULL;
}

static int hyprland_socket_path(char *buf, size_t len)
{
    const char *sig = getenv("HYPRLAND_INSTANCE_SIGNATURE");
    const char *xdg = getenv("XDG_RUNTIME_DIR");

    if (!sig || !xdg) {
        hs_error("HYPRLAND_INSTANCE_SIGNATURE or XDG_RUNTIME_DIR not set");
        return -1;
    }

    int n = snprintf(buf, len, "%s/hypr/%s/.socket.sock", xdg, sig);
    if (n < 0 || (size_t)n >= len) {
        hs_error("hyprland socket path too long");
        return -1;
    }
    return 0;
}

static char *send_to_socket(const char *path, const char *payload, size_t payload_len)
{
    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (fd < 0) {
        hs_error("socket(): %s", strerror(errno));
        return NULL;
    }

    struct sockaddr_un addr = { .sun_family = AF_UNIX };
    hs_strlcpy(addr.sun_path, path, sizeof(addr.sun_path));

    if (connect(fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
        hs_error("connect(%s): %s", path, strerror(errno));
        close(fd);
        return NULL;
    }

    if (write(fd, payload, payload_len) < 0) {
        hs_error("write to hyprland socket: %s", strerror(errno));
        close(fd);
        return NULL;
    }

    size_t cap = 4096;
    size_t total = 0;
    char *buf = malloc(cap);
    if (!buf) {
        close(fd);
        return NULL;
    }

    for (;;) {
        if (total + 1024 > cap) {
            cap *= 2;
            char *tmp = realloc(buf, cap);
            if (!tmp) {
                free(buf);
                close(fd);
                return NULL;
            }
            buf = tmp;
        }
        ssize_t n = read(fd, buf + total, cap - total - 1);
        if (n <= 0)
            break;
        total += (size_t)n;
    }

    buf[total] = '\0';
    close(fd);
    return buf;
}

char *hs_hyprctl(const char *args)
{
    char sock[HS_MAX_PATH];
    if (hyprland_socket_path(sock, sizeof(sock)) < 0)
        return NULL;

    char payload[HS_MAX_CMD];
    int n = snprintf(payload, sizeof(payload), "/%s", args);
    if (n < 0 || (size_t)n >= sizeof(payload))
        return NULL;

    hs_debug("hyprctl >> %s", args);
    char *result = send_to_socket(sock, payload, (size_t)n);
    if (result)
        hs_debug("hyprctl << %.200s", result);
    return result;
}

char *hs_hyprctl_json(const char *args)
{
    char sock[HS_MAX_PATH];
    if (hyprland_socket_path(sock, sizeof(sock)) < 0)
        return NULL;

    char payload[HS_MAX_CMD];
    int n = snprintf(payload, sizeof(payload), "j/%s", args);
    if (n < 0 || (size_t)n >= sizeof(payload))
        return NULL;

    hs_debug("hyprctl -j >> %s", args);
    char *result = send_to_socket(sock, payload, (size_t)n);
    if (result)
        hs_debug("hyprctl -j << %.200s", result);
    return result;
}

int hs_create_headless(char *out_name, size_t out_len)
{
    char *resp = hs_hyprctl("output create headless");
    if (!resp) {
        hs_error("failed to create headless output");
        return -1;
    }

    if (strstr(resp, "ok") == NULL && strstr(resp, "Ok") == NULL) {
        hs_error("hyprctl output create headless: %s", resp);
        free(resp);
        return -1;
    }
    free(resp);

    /*
     * Detect the newly created headless output by querying monitors.
     * Headless outputs are named HEADLESS-1, HEADLESS-2, etc.
     * We find the highest-numbered one.
     */
    char *monitors = hs_hyprctl_json("monitors all");
    if (!monitors) {
        hs_error("failed to query monitors after headless creation");
        return -1;
    }

    char best[HS_MAX_NAME] = "";
    int best_num = -1;
    const char *p = monitors;
    while ((p = strstr(p, "\"name\"")) != NULL) {
        p = strchr(p, ':');
        if (!p) break;
        p++;
        while (*p == ' ' || *p == '"') p++;
        char name[HS_MAX_NAME];
        int i = 0;
        while (*p && *p != '"' && i < (int)sizeof(name) - 1)
            name[i++] = *p++;
        name[i] = '\0';

        if (strncmp(name, "HEADLESS-", 9) == 0) {
            int num = atoi(name + 9);
            if (num > best_num) {
                best_num = num;
                hs_strlcpy(best, name, sizeof(best));
            }
        }
    }

    free(monitors);

    if (best_num < 0) {
        hs_error("could not find headless output after creation");
        return -1;
    }

    hs_strlcpy(out_name, best, out_len);
    hs_info("created headless output: %s", out_name);
    return 0;
}

int hs_remove_headless(const char *name)
{
    char cmd[HS_MAX_CMD];
    snprintf(cmd, sizeof(cmd), "output remove %s", name);
    char *resp = hs_hyprctl(cmd);
    if (!resp)
        return -1;

    int ok = (strstr(resp, "ok") || strstr(resp, "Ok")) ? 0 : -1;
    if (ok < 0)
        hs_error("failed to remove %s: %s", name, resp);
    else
        hs_info("removed headless output: %s", name);
    free(resp);
    return ok;
}

int hs_configure_headless(const char *name, const char *resolution)
{
    char cmd[HS_MAX_CMD];
    snprintf(cmd, sizeof(cmd), "keyword monitor %s,%s,auto,1", name, resolution);
    char *resp = hs_hyprctl(cmd);
    if (!resp)
        return -1;
    free(resp);
    hs_info("configured %s: %s", name, resolution);
    return 0;
}

int hs_move_workspace_to_monitor(const char *workspace, const char *monitor)
{
    char cmd[HS_MAX_CMD];
    snprintf(cmd, sizeof(cmd), "dispatch moveworkspacetomonitor %s %s", workspace, monitor);
    char *resp = hs_hyprctl(cmd);
    if (!resp)
        return -1;

    int ok = (strstr(resp, "ok") || strstr(resp, "Ok")) ? 0 : -1;
    if (ok < 0)
        hs_error("failed to move workspace %s -> monitor %s: %s", workspace, monitor, resp);
    else
        hs_info("moved workspace %s -> monitor %s", workspace, monitor);

    free(resp);
    return ok;
}

int hs_switch_workspace(const char *workspace)
{
    char cmd[HS_MAX_CMD];
    snprintf(cmd, sizeof(cmd), "dispatch workspace %s", workspace);
    char *resp = hs_hyprctl(cmd);
    if (!resp)
        return -1;
    free(resp);
    return 0;
}

int hs_enable_mirror(const char *headless, const char *physical)
{
    char cmd[HS_MAX_CMD];
    snprintf(cmd, sizeof(cmd), "keyword monitor %s,preferred,auto,1,mirror,%s",
             headless, physical);
    char *resp = hs_hyprctl(cmd);
    if (!resp)
        return -1;
    free(resp);
    hs_info("mirroring: %s -> %s", headless, physical);
    return 0;
}

int hs_disable_mirror(const char *headless, const char *resolution)
{
    /*
     * Reconfigure the headless output as a standalone monitor positioned
     * off-screen so it doesn't extend the physical desktop. The negative
     * position ensures no cursor or window accidentally lands on it.
     */
    char cmd[HS_MAX_CMD];
    snprintf(cmd, sizeof(cmd), "keyword monitor %s,%s,-9999x0,1", headless, resolution);
    char *resp = hs_hyprctl(cmd);
    if (!resp)
        return -1;
    free(resp);
    hs_info("mirror disabled on %s (off-screen)", headless);
    return 0;
}

int hs_bind_workspace_to_monitor(const char *workspace, const char *monitor)
{
    char cmd[HS_MAX_CMD];
    snprintf(cmd, sizeof(cmd), "keyword workspace %s,monitor:%s,default:true",
             workspace, monitor);
    char *resp = hs_hyprctl(cmd);
    if (!resp)
        return -1;

    int ok = (strstr(resp, "ok") || strstr(resp, "Ok")) ? 0 : -1;
    if (ok < 0)
        hs_error("failed to bind workspace %s -> monitor %s: %s", workspace, monitor, resp);
    else
        hs_info("bound workspace %s -> monitor %s", workspace, monitor);

    free(resp);
    return ok;
}

int hs_restore_headless_stolen_workspaces(const char *before_workspaces_json,
                                          const char *headless,
                                          const char *streaming_workspace)
{
    if (!before_workspaces_json || !headless || !*headless)
        return 0;

    char *after_json = hs_hyprctl_json("workspaces");
    if (!after_json)
        return -1;

    struct hs_ws_loc before[256];
    struct hs_ws_loc after[256];
    int n_before = parse_workspaces_json(before_workspaces_json, before, 256);
    int n_after = parse_workspaces_json(after_json, after, 256);

    int moved = 0;
    for (int i = 0; i < n_after; i++) {
        if (strcmp(after[i].monitor, headless) != 0)
            continue;
        if (streaming_workspace && streaming_workspace[0] &&
            strcmp(after[i].name, streaming_workspace) == 0)
            continue;

        const char *prev = find_monitor_for_workspace(before, n_before, after[i].name);
        if (!prev || !*prev)
            continue;
        if (strcmp(prev, headless) == 0)
            continue;
        if (strncmp(prev, "HEADLESS-", 9) == 0)
            continue;

        hs_warn("workspace %s moved to %s during enable; restoring to %s",
                after[i].name, headless, prev);
        if (hs_move_workspace_to_monitor(after[i].name, prev) == 0)
            moved++;
    }

    free(after_json);
    return moved;
}

int hs_detect_physical_monitor(char *out, size_t len)
{
    char *monitors = hs_hyprctl_json("monitors");
    if (!monitors) {
        hs_error("failed to query monitors");
        return -1;
    }

    /*
     * Lightweight JSON parse: find first monitor whose name does not
     * start with "HEADLESS-". This is the physical monitor.
     */
    const char *p = monitors;
    while ((p = strstr(p, "\"name\"")) != NULL) {
        p = strchr(p, ':');
        if (!p) break;
        p++;
        while (*p == ' ' || *p == '"') p++;
        char name[HS_MAX_NAME];
        int i = 0;
        while (*p && *p != '"' && i < (int)sizeof(name) - 1)
            name[i++] = *p++;
        name[i] = '\0';

        if (strncmp(name, "HEADLESS-", 9) != 0) {
            hs_strlcpy(out, name, len);
            free(monitors);
            hs_info("detected physical monitor: %s", out);
            return 0;
        }
    }

    free(monitors);
    hs_error("no physical monitor found");
    return -1;
}

int hs_get_active_workspace(char *out, size_t len)
{
    char *resp = hs_hyprctl_json("activeworkspace");
    if (!resp)
        return -1;

    const char *p = strstr(resp, "\"name\"");
    if (!p) {
        free(resp);
        return -1;
    }
    p = strchr(p, ':');
    if (!p) { free(resp); return -1; }
    p++;
    while (*p == ' ' || *p == '"') p++;

    int i = 0;
    while (*p && *p != '"' && i < (int)len - 1)
        out[i++] = *p++;
    out[i] = '\0';

    free(resp);
    return 0;
}
