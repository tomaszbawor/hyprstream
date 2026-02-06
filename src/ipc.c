/* SPDX-License-Identifier: MIT */

#include "hyprstream.h"

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>

int hs_ipc_connect(void)
{
    const char *sig = getenv("HYPRLAND_INSTANCE_SIGNATURE");
    const char *xdg = getenv("XDG_RUNTIME_DIR");

    if (!sig || !xdg) {
        hs_error("HYPRLAND_INSTANCE_SIGNATURE or XDG_RUNTIME_DIR not set");
        return -1;
    }

    char path[HS_MAX_PATH];
    int n = snprintf(path, sizeof(path), "%s/hypr/%s/.socket2.sock", xdg, sig);
    if (n < 0 || (size_t)n >= sizeof(path)) {
        hs_error("socket2 path too long");
        return -1;
    }

    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (fd < 0) {
        hs_error("socket(): %s", strerror(errno));
        return -1;
    }

    struct sockaddr_un addr = { .sun_family = AF_UNIX };
    hs_strlcpy(addr.sun_path, path, sizeof(addr.sun_path));

    if (connect(fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
        hs_error("connect(%s): %s", path, strerror(errno));
        close(fd);
        return -1;
    }

    hs_info("connected to hyprland IPC: %s", path);
    return fd;
}

int hs_ipc_listen(int fd, hs_event_cb cb, void *ctx)
{
    char buf[HS_IPC_BUF];
    size_t pos = 0;

    for (;;) {
        ssize_t n = read(fd, buf + pos, sizeof(buf) - pos - 1);
        if (n <= 0) {
            if (n == 0)
                hs_warn("hyprland IPC connection closed");
            else if (errno == EINTR)
                continue;
            else
                hs_error("read from IPC: %s", strerror(errno));
            return -1;
        }

        pos += (size_t)n;
        buf[pos] = '\0';

        char *line = buf;
        char *nl;
        while ((nl = strchr(line, '\n')) != NULL) {
            *nl = '\0';

            char *sep = strstr(line, ">>");
            if (sep) {
                *sep = '\0';
                const char *event = line;
                const char *data = sep + 2;
                cb(event, data, ctx);
            }

            line = nl + 1;
        }

        size_t remaining = (size_t)(buf + pos - line);
        if (remaining > 0 && line != buf)
            memmove(buf, line, remaining);
        pos = remaining;
    }
}
