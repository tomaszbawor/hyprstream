/* SPDX-License-Identifier: MIT */

#include "hyprstream.h"

#include <stdio.h>
#include <string.h>

static void usage(const char *prog)
{
    fprintf(stderr,
        "hyprstream %s - virtual output manager for Hyprland streaming\n"
        "\n"
        "Usage: %s <command> [options]\n"
        "\n"
        "Commands:\n"
        "  daemon [-c config]   Start the daemon\n"
        "  enable               Enable streaming mode\n"
        "  disable              Disable streaming mode\n"
        "  toggle               Toggle streaming mode\n"
        "  status               Show current status\n"
        "  version              Show version\n"
        "\n"
        "Config: ~/.config/hyprstream/config\n",
        HS_VERSION, prog);
}

int main(int argc, char **argv)
{
    if (argc < 2) {
        usage(argv[0]);
        return 1;
    }

    const char *cmd = argv[1];

    if (strcmp(cmd, "version") == 0 || strcmp(cmd, "--version") == 0 ||
        strcmp(cmd, "-v") == 0) {
        printf("hyprstream %s\n", HS_VERSION);
        return 0;
    }

    if (strcmp(cmd, "daemon") == 0) {
        const char *config_path = NULL;

        for (int i = 2; i < argc; i++) {
            if ((strcmp(argv[i], "-c") == 0 || strcmp(argv[i], "--config") == 0)
                && i + 1 < argc) {
                config_path = argv[++i];
            } else if (strcmp(argv[i], "-v") == 0 || strcmp(argv[i], "--verbose") == 0) {
                hs_log_set_level(HS_LOG_DEBUG);
            } else {
                fprintf(stderr, "unknown option: %s\n", argv[i]);
                return 1;
            }
        }

        struct hs_config cfg;
        hs_config_load(&cfg, config_path);
        return hs_daemon_run(&cfg);
    }

    if (strcmp(cmd, "enable") == 0)
        return hs_ctl_send(HS_CTL_ENABLE);

    if (strcmp(cmd, "disable") == 0)
        return hs_ctl_send(HS_CTL_DISABLE);

    if (strcmp(cmd, "toggle") == 0)
        return hs_ctl_send(HS_CTL_TOGGLE);

    if (strcmp(cmd, "status") == 0)
        return hs_ctl_send(HS_CTL_STATUS);

    if (strcmp(cmd, "--help") == 0 || strcmp(cmd, "-h") == 0 || strcmp(cmd, "help") == 0) {
        usage(argv[0]);
        return 0;
    }

    fprintf(stderr, "unknown command: %s\n", cmd);
    usage(argv[0]);
    return 1;
}
