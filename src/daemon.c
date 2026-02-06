/* SPDX-License-Identifier: MIT */

#include "hyprstream.h"

#include <errno.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

static struct hs_state g_state;

static void reconcile(struct hs_state *st);
static int  streaming_enable(struct hs_state *st);
static int  streaming_disable(struct hs_state *st);

static void handle_signal(int sig)
{
    (void)sig;
    g_state.running = false;
}

static int streaming_enable(struct hs_state *st)
{
    if (st->mode == HS_MODE_ENABLED) {
        hs_info("already enabled");
        return 0;
    }

    if (st->physical_monitor[0] == '\0') {
        if (hs_detect_physical_monitor(st->physical_monitor,
                                       sizeof(st->physical_monitor)) < 0)
            return -1;
    }

    char *before_workspaces = hs_hyprctl_json("workspaces");

    if (hs_create_headless(st->headless_name, sizeof(st->headless_name)) < 0) {
        free(before_workspaces);
        return -1;
    }

    hs_disable_mirror(st->headless_name, st->cfg.virtual_resolution);
    if (hs_bind_workspace_to_monitor(st->cfg.streaming_workspace, st->headless_name) < 0)
        goto fail;
    if (hs_move_workspace_to_monitor(st->cfg.streaming_workspace, st->headless_name) < 0)
        goto fail;

    /*
     * Hyprland may assign an existing workspace to a newly-created monitor.
     * If that steals a workspace from a physical monitor, restore it back.
     */
    hs_restore_headless_stolen_workspaces(before_workspaces,
                                          st->headless_name,
                                          st->cfg.streaming_workspace);
    free(before_workspaces);

    st->mode = HS_MODE_ENABLED;
    st->mirroring_active = false;
    st->on_streaming_workspace = false;

    reconcile(st);

    if (st->cfg.on_enable[0])
        hs_exec_async(st->cfg.on_enable);

    hs_info("streaming mode enabled (headless=%s, physical=%s)",
            st->headless_name, st->physical_monitor);
    return 0;

fail:
    hs_restore_headless_stolen_workspaces(before_workspaces,
                                          st->headless_name,
                                          st->cfg.streaming_workspace);
    free(before_workspaces);
    if (st->headless_name[0] != '\0') {
        hs_remove_headless(st->headless_name);
        st->headless_name[0] = '\0';
    }
    return -1;
}

static int streaming_disable(struct hs_state *st)
{
    if (st->mode == HS_MODE_DISABLED) {
        hs_info("already disabled");
        return 0;
    }

    if (st->mirroring_active) {
        hs_disable_mirror(st->headless_name, st->cfg.virtual_resolution);
        st->mirroring_active = false;
    }

    hs_move_workspace_to_monitor(st->cfg.streaming_workspace, st->physical_monitor);

    if (st->headless_name[0] != '\0') {
        hs_remove_headless(st->headless_name);
        st->headless_name[0] = '\0';
    }

    st->mode = HS_MODE_DISABLED;
    st->on_streaming_workspace = false;

    if (st->cfg.on_disable[0])
        hs_exec_async(st->cfg.on_disable);

    hs_info("streaming mode disabled");
    return 0;
}

static void reconcile(struct hs_state *st)
{
    if (st->mode != HS_MODE_ENABLED)
        return;

    char ws[HS_MAX_NAME];
    if (hs_get_active_workspace(ws, sizeof(ws)) < 0)
        return;

    bool on_stream = (strcmp(ws, st->cfg.streaming_workspace) == 0);

    if (on_stream && !st->mirroring_active) {
        hs_info("entering streaming workspace -> enable mirror");
        hs_enable_mirror(st->headless_name, st->physical_monitor);
        st->mirroring_active = true;
        st->on_streaming_workspace = true;

        if (st->cfg.on_streaming_enter[0])
            hs_exec_async(st->cfg.on_streaming_enter);

    } else if (!on_stream && st->mirroring_active) {
        hs_info("leaving streaming workspace -> disable mirror");
        hs_disable_mirror(st->headless_name, st->cfg.virtual_resolution);
        st->mirroring_active = false;
        st->on_streaming_workspace = false;

        if (st->cfg.on_streaming_leave[0])
            hs_exec_async(st->cfg.on_streaming_leave);
    }

    hs_strlcpy(st->active_workspace, ws, sizeof(st->active_workspace));
}

static void on_hyprland_event(const char *event, const char *data, void *ctx)
{
    struct hs_state *st = ctx;
    (void)data;

    if (strcmp(event, "workspace") == 0 ||
        strcmp(event, "focusedmon") == 0 ||
        strcmp(event, "activewindow") == 0 ||
        strcmp(event, "movewindow") == 0) {
        reconcile(st);
    } else if (strcmp(event, "monitorremoved") == 0) {
        if (st->mode == HS_MODE_ENABLED &&
            st->headless_name[0] != '\0' &&
            strcmp(data, st->headless_name) == 0) {
            hs_warn("headless output was removed externally");
            st->mode = HS_MODE_DISABLED;
            st->headless_name[0] = '\0';
            st->mirroring_active = false;
            st->on_streaming_workspace = false;
        }
    }
}

static void handle_ctl_client(int client_fd, struct hs_state *st)
{
    char buf[256];
    ssize_t n = read(client_fd, buf, sizeof(buf) - 1);
    if (n <= 0) {
        close(client_fd);
        return;
    }
    buf[n] = '\0';
    hs_trim(buf);

    char response[1024];

    if (strcmp(buf, HS_CTL_ENABLE) == 0) {
        int rc = streaming_enable(st);
        snprintf(response, sizeof(response),
                 rc == 0 ? "enabled" : "error: enable failed");

    } else if (strcmp(buf, HS_CTL_DISABLE) == 0) {
        int rc = streaming_disable(st);
        snprintf(response, sizeof(response),
                 rc == 0 ? "disabled" : "error: disable failed");

    } else if (strcmp(buf, HS_CTL_TOGGLE) == 0) {
        int rc;
        if (st->mode == HS_MODE_ENABLED)
            rc = streaming_disable(st);
        else
            rc = streaming_enable(st);
        snprintf(response, sizeof(response),
                 rc == 0 ? (st->mode == HS_MODE_ENABLED ? "enabled" : "disabled")
                         : "error: toggle failed");

    } else if (strcmp(buf, HS_CTL_STATUS) == 0) {
        snprintf(response, sizeof(response),
                 "mode=%s headless=%s physical=%s workspace=%s mirroring=%s",
                 st->mode == HS_MODE_ENABLED ? "enabled" : "disabled",
                 st->headless_name[0] ? st->headless_name : "none",
                 st->physical_monitor[0] ? st->physical_monitor : "unknown",
                 st->active_workspace,
                 st->mirroring_active ? "on" : "off");

    } else if (strcmp(buf, HS_CTL_QUIT) == 0) {
        snprintf(response, sizeof(response), "shutting down");
        st->running = false;

    } else {
        snprintf(response, sizeof(response), "error: unknown command: %s", buf);
    }

    ssize_t wr = write(client_fd, response, strlen(response));
    (void)wr;
    close(client_fd);
}

int hs_daemon_run(struct hs_config *cfg)
{
    memset(&g_state, 0, sizeof(g_state));
    g_state.cfg = *cfg;
    g_state.running = true;
    g_state.mode = HS_MODE_DISABLED;

    signal(SIGINT, handle_signal);
    signal(SIGTERM, handle_signal);
    signal(SIGPIPE, SIG_IGN);

    if (cfg->physical_monitor[0]) {
        hs_strlcpy(g_state.physical_monitor, cfg->physical_monitor,
                    sizeof(g_state.physical_monitor));
    } else {
        if (hs_detect_physical_monitor(g_state.physical_monitor,
                                       sizeof(g_state.physical_monitor)) < 0) {
            hs_error("could not detect physical monitor");
            return 1;
        }
    }

    int ctl_fd = hs_ctl_create();
    if (ctl_fd < 0)
        return 1;
    g_state.ctl_fd = ctl_fd;

    if (cfg->auto_enable) {
        if (streaming_enable(&g_state) < 0)
            hs_warn("auto-enable failed, continuing in disabled mode");
    }

    int reconnect_attempts = 0;

    while (g_state.running) {
        int ipc_fd = hs_ipc_connect();
        if (ipc_fd < 0) {
            reconnect_attempts++;
            if (reconnect_attempts > HS_MAX_RECONNECT) {
                hs_error("max reconnect attempts reached, exiting");
                break;
            }
            hs_warn("IPC connect failed, retrying in %ds (%d/%d)",
                    HS_RECONNECT_DELAY, reconnect_attempts, HS_MAX_RECONNECT);
            sleep(HS_RECONNECT_DELAY);
            continue;
        }
        reconnect_attempts = 0;
        g_state.ipc_fd = ipc_fd;

        struct pollfd fds[2];
        fds[0].fd = ipc_fd;
        fds[0].events = POLLIN;
        fds[1].fd = ctl_fd;
        fds[1].events = POLLIN;

        while (g_state.running) {
            int ret = poll(fds, 2, 1000);
            if (ret < 0) {
                if (errno == EINTR)
                    continue;
                hs_error("poll: %s", strerror(errno));
                break;
            }

            if (fds[0].revents & POLLIN) {
                char buf[HS_IPC_BUF];
                ssize_t n = read(ipc_fd, buf, sizeof(buf) - 1);
                if (n <= 0) {
                    hs_warn("IPC connection lost, reconnecting...");
                    break;
                }
                buf[n] = '\0';

                char *line = buf;
                char *nl;
                while ((nl = strchr(line, '\n')) != NULL) {
                    *nl = '\0';
                    char *sep = strstr(line, ">>");
                    if (sep) {
                        *sep = '\0';
                        on_hyprland_event(line, sep + 2, &g_state);
                    }
                    line = nl + 1;
                }
            }

            if (fds[0].revents & (POLLHUP | POLLERR)) {
                hs_warn("IPC socket error, reconnecting...");
                break;
            }

            if (fds[1].revents & POLLIN) {
                int client = accept(ctl_fd, NULL, NULL);
                if (client >= 0)
                    handle_ctl_client(client, &g_state);
            }
        }

        close(ipc_fd);
        g_state.ipc_fd = -1;
    }

    if (g_state.mode == HS_MODE_ENABLED)
        streaming_disable(&g_state);

    close(ctl_fd);
    unlink(hs_ctl_socket_path());

    hs_info("daemon stopped");
    return 0;
}
