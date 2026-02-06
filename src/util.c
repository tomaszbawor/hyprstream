/*
 * hyprstream - utility functions
 * SPDX-License-Identifier: MIT
 */

#include "hyprstream.h"

#include <ctype.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/wait.h>

void hs_strlcpy(char *dst, const char *src, size_t size)
{
    if (size == 0)
        return;
    size_t len = strlen(src);
    if (len >= size)
        len = size - 1;
    memcpy(dst, src, len);
    dst[len] = '\0';
}

void hs_trim(char *s)
{
    if (!s || !*s)
        return;
    char *end = s + strlen(s) - 1;
    while (end > s && isspace((unsigned char)*end))
        *end-- = '\0';
}

int hs_exec(const char *cmd)
{
    if (!cmd || !*cmd)
        return 0;

    hs_debug("exec: %s", cmd);
    int status = system(cmd);
    if (status == -1) {
        hs_error("failed to execute: %s", cmd);
        return -1;
    }
    return WEXITSTATUS(status);
}

void hs_exec_async(const char *cmd)
{
    if (!cmd || !*cmd)
        return;

    hs_debug("exec_async: %s", cmd);

    pid_t pid = fork();
    if (pid == 0) {
        /* child: double-fork to avoid zombies */
        pid_t pid2 = fork();
        if (pid2 == 0) {
            setsid();
            execl("/bin/sh", "sh", "-c", cmd, (char *)NULL);
            _exit(127);
        }
        _exit(0);
    } else if (pid > 0) {
        waitpid(pid, NULL, 0);
    } else {
        hs_error("fork failed for async exec");
    }
}
