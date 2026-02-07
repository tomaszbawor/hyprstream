# hyprstream

Virtual output manager for Hyprland streaming privacy.

hyprstream runs a small daemon that:
- creates a headless (virtual) monitor (named `HEADLESS-*` by Hyprland)
- binds a dedicated "streaming workspace" (default: `9`) to that virtual monitor
- mirrors/unmirrors the physical monitor to the virtual monitor when you enter/leave the streaming workspace

The net effect: OBS captures the headless output, and viewers only ever see workspace `9`.

## Why Rust

The original implementation was in C. The Rust rewrite keeps the same behavior and CLI, but:
- uses `serde_json` instead of ad-hoc JSON parsing
- keeps the daemon as a single-threaded `poll()` loop over two Unix sockets (Hyprland events + control)
- improves error propagation and state cleanup on failures

The C sources are preserved under `csrc/`.

## Usage

CLI (same as before):

```bash
hyprstream daemon [-c /path/to/config] [-v|--verbose]
hyprstream enable
hyprstream disable
hyprstream toggle
hyprstream status
hyprstream version
```

Config file (default): `~/.config/hyprstream/config`

Example:

```conf
streaming_workspace = 9
virtual_resolution = 1920x1080@60

# optional
# physical_monitor = eDP-1
# auto_enable = true
# on_enable = notify-send 'hyprstream enabled'
# on_disable = notify-send 'hyprstream disabled'
# on_streaming_enter = notify-send 'stream visible'
# on_streaming_leave = notify-send 'stream hidden'
```

## Build

### With Nix (recommended)

Dev shell:

```bash
nix develop
cargo build
```

Build the package:

```bash
nix build .#hyprstream
```

### With Cargo

```bash
cargo build --release
./target/release/hyprstream version
```

## How it works

Hyprland IPC:
- Commands are sent over Hyprland's `.socket.sock` (request/response).
- Events are received over Hyprland's `.socket2.sock`.

Daemon loop:
- `poll()` waits on Hyprland events and the hyprstream control socket.
- On relevant Hyprland events (workspace/focus/window), it queries `activeworkspace` and toggles mirroring.

Workspace stealing workaround:
- When the headless output is created, Hyprland may temporarily assign an existing workspace to it.
- hyprstream snapshots `j/workspaces` before creation, and after enable restores any non-streaming workspaces
  that ended up on `HEADLESS-*` back to their previous (physical) monitor.

## Troubleshooting

Useful Hyprland commands:

```bash
hyprctl -j workspaces
hyprctl -j monitors all
hyprctl -j activeworkspace
```

If the daemon isn't reachable:
- the control socket is `${XDG_RUNTIME_DIR}/hyprstream.sock` (or `/tmp/hyprstream-<uid>.sock` fallback)
