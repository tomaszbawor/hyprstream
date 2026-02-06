PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin

CC      ?= gcc
CFLAGS  ?= -Wall -Wextra -Wpedantic -std=c11 -D_POSIX_C_SOURCE=200809L -O2
LDFLAGS ?=

SRCS = src/main.c src/daemon.c src/hyprctl.c src/ipc.c \
       src/config.c src/control.c src/log.c src/util.c
OBJS = $(SRCS:.c=.o)
BIN  = hyprstream

all: $(BIN)

$(BIN): $(OBJS)
	$(CC) $(LDFLAGS) -o $@ $^

src/%.o: src/%.c src/hyprstream.h
	$(CC) $(CFLAGS) -c -o $@ $<

install: $(BIN)
	install -Dm755 $(BIN) $(DESTDIR)$(BINDIR)/$(BIN)

uninstall:
	rm -f $(DESTDIR)$(BINDIR)/$(BIN)

clean:
	rm -f $(OBJS) $(BIN)

.PHONY: all install uninstall clean
