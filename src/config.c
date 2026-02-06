/* SPDX-License-Identifier: MIT */

#include "hyprstream.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>

static char g_default_path[HS_MAX_PATH];

const char *hs_config_default_path(void)
{
    const char *xdg = getenv("XDG_CONFIG_HOME");
    const char *home = getenv("HOME");

    if (xdg && *xdg)
        snprintf(g_default_path, sizeof(g_default_path),
                 "%s/hyprstream/config", xdg);
    else if (home && *home)
        snprintf(g_default_path, sizeof(g_default_path),
                 "%s/.config/hyprstream/config", home);
    else
        hs_strlcpy(g_default_path, "/etc/hyprstream/config",
                    sizeof(g_default_path));

    return g_default_path;
}

static void set_defaults(struct hs_config *cfg)
{
    memset(cfg, 0, sizeof(*cfg));
    hs_strlcpy(cfg->streaming_workspace, "0", sizeof(cfg->streaming_workspace));
    hs_strlcpy(cfg->virtual_resolution, "1920x1080@60", sizeof(cfg->virtual_resolution));
    cfg->auto_enable = false;
}

static void parse_line(struct hs_config *cfg, const char *key, const char *val)
{
    if (strcmp(key, "streaming_workspace") == 0)
        hs_strlcpy(cfg->streaming_workspace, val, sizeof(cfg->streaming_workspace));
    else if (strcmp(key, "physical_monitor") == 0)
        hs_strlcpy(cfg->physical_monitor, val, sizeof(cfg->physical_monitor));
    else if (strcmp(key, "virtual_resolution") == 0)
        hs_strlcpy(cfg->virtual_resolution, val, sizeof(cfg->virtual_resolution));
    else if (strcmp(key, "on_streaming_enter") == 0)
        hs_strlcpy(cfg->on_streaming_enter, val, sizeof(cfg->on_streaming_enter));
    else if (strcmp(key, "on_streaming_leave") == 0)
        hs_strlcpy(cfg->on_streaming_leave, val, sizeof(cfg->on_streaming_leave));
    else if (strcmp(key, "on_enable") == 0)
        hs_strlcpy(cfg->on_enable, val, sizeof(cfg->on_enable));
    else if (strcmp(key, "on_disable") == 0)
        hs_strlcpy(cfg->on_disable, val, sizeof(cfg->on_disable));
    else if (strcmp(key, "auto_enable") == 0)
        cfg->auto_enable = (strcmp(val, "true") == 0 || strcmp(val, "1") == 0);
    else
        hs_warn("unknown config key: %s", key);
}

int hs_config_load(struct hs_config *cfg, const char *path)
{
    set_defaults(cfg);

    if (!path)
        path = hs_config_default_path();

    FILE *f = fopen(path, "r");
    if (!f) {
        hs_info("config file not found: %s (using defaults)", path);
        return 1;
    }

    hs_info("loading config: %s", path);

    char line[HS_MAX_CMD];
    int lineno = 0;
    while (fgets(line, sizeof(line), f)) {
        lineno++;
        hs_trim(line);

        if (!*line || *line == '#')
            continue;

        char *eq = strchr(line, '=');
        if (!eq) {
            hs_warn("config:%d: malformed line (missing '=')", lineno);
            continue;
        }

        *eq = '\0';
        char *key = line;
        char *val = eq + 1;

        while (*key && isspace((unsigned char)*key)) key++;
        char *kend = key + strlen(key) - 1;
        while (kend > key && isspace((unsigned char)*kend)) *kend-- = '\0';

        while (*val && isspace((unsigned char)*val)) val++;

        parse_line(cfg, key, val);
    }

    fclose(f);
    return 0;
}
