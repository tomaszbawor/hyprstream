/* SPDX-License-Identifier: MIT */

#include "hyprstream.h"

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/un.h>
#include <unistd.h>

static char g_ctl_path[HS_MAX_PATH];

const char *hs_ctl_socket_path(void)
{
    const char *xdg = getenv("XDG_RUNTIME_DIR");
    if (xdg && *xdg)
        snprintf(g_ctl_path, sizeof(g_ctl_path), "%s/hyprstream.sock", xdg);
    else
        snprintf(g_ctl_path, sizeof(g_ctl_path), "/tmp/hyprstream-%d.sock", getuid());
    return g_ctl_path;
}

int hs_ctl_create(void)
{
    const char *path = hs_ctl_socket_path();
    unlink(path);

    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (fd < 0) {
        hs_error("socket(): %s", strerror(errno));
        return -1;
    }

    struct sockaddr_un addr = { .sun_family = AF_UNIX };
    hs_strlcpy(addr.sun_path, path, sizeof(addr.sun_path));

    if (bind(fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
        hs_error("bind(%s): %s", path, strerror(errno));
        close(fd);
        return -1;
    }

    chmod(path, 0700);

    if (listen(fd, HS_SOCK_BACKLOG) < 0) {
        hs_error("listen(): %s", strerror(errno));
        close(fd);
        unlink(path);
        return -1;
    }

    hs_info("control socket: %s", path);
    return fd;
}

int hs_ctl_send(const char *cmd)
{
    const char *path = hs_ctl_socket_path();

    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (fd < 0) {
        hs_error("socket(): %s", strerror(errno));
        return -1;
    }

    struct sockaddr_un addr = { .sun_family = AF_UNIX };
    hs_strlcpy(addr.sun_path, path, sizeof(addr.sun_path));

    if (connect(fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
        fprintf(stderr, "hyprstream: daemon not running (connect %s: %s)\n",
                path, strerror(errno));
        close(fd);
        return -1;
    }

    size_t len = strlen(cmd);
    if (write(fd, cmd, len) < 0) {
        hs_error("write: %s", strerror(errno));
        close(fd);
        return -1;
    }

    char buf[4096];
    ssize_t n = read(fd, buf, sizeof(buf) - 1);
    if (n > 0) {
        buf[n] = '\0';
        printf("%s\n", buf);
    }

    close(fd);
    return 0;
}
