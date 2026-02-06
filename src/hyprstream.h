/*
 * hyprstream - Virtual output manager for Hyprland streaming
 *
 * Creates a headless virtual monitor bound to a designated "streaming
 * workspace" and automatically mirrors/unmirrors the physical display
 * when switching workspaces, so OBS viewers only ever see the streaming
 * workspace content.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef HYPRSTREAM_H
#define HYPRSTREAM_H

#include <stdbool.h>
#include <stddef.h>

/* ------------------------------------------------------------------ */
/*  Limits                                                             */
/* ------------------------------------------------------------------ */

#define HS_MAX_PATH        4096
#define HS_MAX_NAME        256
#define HS_MAX_CMD         8192
#define HS_MAX_HOOKS       8
#define HS_IPC_BUF         65536
#define HS_JSON_BUF        131072
#define HS_RECONNECT_DELAY 2    /* seconds */
#define HS_MAX_RECONNECT   30   /* attempts before giving up */
#define HS_SOCK_BACKLOG    4
#define HS_VERSION         "0.1.0"

/* ------------------------------------------------------------------ */
/*  Configuration                                                      */
/* ------------------------------------------------------------------ */

struct hs_config {
    /* Workspace that is visible to stream viewers */
    char streaming_workspace[HS_MAX_NAME];

    /* Physical monitor name (auto-detected if empty) */
    char physical_monitor[HS_MAX_NAME];

    /* Resolution for the virtual output (WIDTHxHEIGHT@RATE) */
    char virtual_resolution[HS_MAX_NAME];

    /* User hooks: commands executed on state transitions */
    char on_streaming_enter[HS_MAX_CMD];
    char on_streaming_leave[HS_MAX_CMD];
    char on_enable[HS_MAX_CMD];
    char on_disable[HS_MAX_CMD];

    /* Auto-start streaming mode on daemon launch */
    bool auto_enable;
};

/* ------------------------------------------------------------------ */
/*  Runtime state                                                      */
/* ------------------------------------------------------------------ */

enum hs_mode {
    HS_MODE_DISABLED = 0,   /* passthrough, no virtual output */
    HS_MODE_ENABLED,        /* virtual output exists, mirroring managed */
};

struct hs_state {
    enum hs_mode             mode;
    char                     headless_name[HS_MAX_NAME]; /* e.g. HEADLESS-1 */
    char                     physical_monitor[HS_MAX_NAME];
    char                     active_workspace[HS_MAX_NAME];
    bool                     on_streaming_workspace;
    bool                     mirroring_active;
    int                      ipc_fd;       /* hyprland socket2 */
    int                      ctl_fd;       /* our control socket */
    volatile bool            running;
    struct hs_config         cfg;
};

/* ------------------------------------------------------------------ */
/*  hyprctl.c — Hyprland command interface                             */
/* ------------------------------------------------------------------ */

/* Execute a hyprctl command and return malloc'd output (caller frees).
 * Returns NULL on failure. */
char *hs_hyprctl(const char *args);

/* JSON variants */
char *hs_hyprctl_json(const char *args);

/* Convenience wrappers */
int   hs_create_headless(char *out_name, size_t out_len);
int   hs_remove_headless(const char *name);
int   hs_configure_headless(const char *name, const char *resolution);
int   hs_move_workspace_to_monitor(const char *workspace, const char *monitor);
int   hs_switch_workspace(const char *workspace);
int   hs_enable_mirror(const char *headless, const char *physical);
int   hs_disable_mirror(const char *headless, const char *resolution);
int   hs_bind_workspace_to_monitor(const char *workspace, const char *monitor);
int   hs_detect_physical_monitor(char *out, size_t len);
int   hs_get_active_workspace(char *out, size_t len);

/* ------------------------------------------------------------------ */
/*  ipc.c — Hyprland event listener                                    */
/* ------------------------------------------------------------------ */

/* Connect to Hyprland socket2. Returns fd or -1. */
int   hs_ipc_connect(void);

/* Read and dispatch events (blocking). Returns 0 on clean shutdown,
 * -1 on error. Calls the callback for each event. */
typedef void (*hs_event_cb)(const char *event, const char *data, void *ctx);
int   hs_ipc_listen(int fd, hs_event_cb cb, void *ctx);

/* ------------------------------------------------------------------ */
/*  config.c — Configuration                                           */
/* ------------------------------------------------------------------ */

/* Load config from path (NULL = default XDG path).
 * Returns 0 on success, -1 on error, 1 if file not found (defaults used). */
int   hs_config_load(struct hs_config *cfg, const char *path);

/* Return default config path (~/.config/hyprstream/config) */
const char *hs_config_default_path(void);

/* ------------------------------------------------------------------ */
/*  daemon.c — Main daemon loop                                        */
/* ------------------------------------------------------------------ */

int   hs_daemon_run(struct hs_config *cfg);

/* ------------------------------------------------------------------ */
/*  control.c — Control socket (CLI ↔ daemon)                          */
/* ------------------------------------------------------------------ */

/* Commands sent over the control socket */
#define HS_CTL_ENABLE   "enable"
#define HS_CTL_DISABLE  "disable"
#define HS_CTL_TOGGLE   "toggle"
#define HS_CTL_STATUS   "status"
#define HS_CTL_QUIT     "quit"

/* Create the control socket. Returns fd or -1. */
int   hs_ctl_create(void);

/* Send a command to a running daemon. Prints response to stdout.
 * Returns 0 on success, -1 on error. */
int   hs_ctl_send(const char *cmd);

/* Get the control socket path */
const char *hs_ctl_socket_path(void);

/* ------------------------------------------------------------------ */
/*  log.c — Logging                                                    */
/* ------------------------------------------------------------------ */

enum hs_log_level {
    HS_LOG_DEBUG = 0,
    HS_LOG_INFO,
    HS_LOG_WARN,
    HS_LOG_ERROR,
};

void  hs_log_set_level(enum hs_log_level level);
void  hs_log(enum hs_log_level level, const char *fmt, ...)
    __attribute__((format(printf, 2, 3)));

#define hs_debug(...) hs_log(HS_LOG_DEBUG, __VA_ARGS__)
#define hs_info(...)  hs_log(HS_LOG_INFO,  __VA_ARGS__)
#define hs_warn(...)  hs_log(HS_LOG_WARN,  __VA_ARGS__)
#define hs_error(...) hs_log(HS_LOG_ERROR, __VA_ARGS__)

/* ------------------------------------------------------------------ */
/*  util.c — Misc utilities                                            */
/* ------------------------------------------------------------------ */

/* Run a shell command, return exit status. */
int   hs_exec(const char *cmd);

/* Run a shell command asynchronously (fire-and-forget). */
void  hs_exec_async(const char *cmd);

/* Safe string copy */
void  hs_strlcpy(char *dst, const char *src, size_t size);

/* Trim trailing whitespace in-place */
void  hs_trim(char *s);

#endif /* HYPRSTREAM_H */
