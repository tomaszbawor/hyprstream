/*
 * hyprstream - logging
 * SPDX-License-Identifier: MIT
 */

#include "hyprstream.h"

#include <stdarg.h>
#include <stdio.h>
#include <time.h>

static enum hs_log_level g_level = HS_LOG_INFO;

static const char *level_str[] = {
    [HS_LOG_DEBUG] = "DEBUG",
    [HS_LOG_INFO]  = "INFO",
    [HS_LOG_WARN]  = "WARN",
    [HS_LOG_ERROR] = "ERROR",
};

void hs_log_set_level(enum hs_log_level level)
{
    g_level = level;
}

void hs_log(enum hs_log_level level, const char *fmt, ...)
{
    if (level < g_level)
        return;

    time_t now = time(NULL);
    struct tm tm;
    localtime_r(&now, &tm);

    char timebuf[32];
    strftime(timebuf, sizeof(timebuf), "%H:%M:%S", &tm);

    fprintf(stderr, "[%s] %s: ", timebuf, level_str[level]);

    va_list ap;
    va_start(ap, fmt);
    vfprintf(stderr, fmt, ap);
    va_end(ap);

    fputc('\n', stderr);
}
