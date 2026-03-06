# window-list-overlay

A lightweight window list overlay for [Hyprland](https://hyprland.org/), designed for scrolling workspace layouts. Hold the Super key to see all windows on the current workspace — release to dismiss.

Built with GTK4 and layer-shell. Integrates with [Omarchy](https://github.com/basecamp/omarchy) themes.

## How it works

1. A Hyprland keybinding fires a shell script on Super press
2. After a 200ms debounce (so `Super+key` combos don't flash the overlay), the script sends `SIGUSR1` to the daemon
3. The daemon shows the overlay on the focused monitor and polls the Super key state via evdev every 50ms
4. On Super release, the overlay hides automatically

By default, the overlay only appears on **scrolling** workspaces — it stays hidden on dwindle/master layouts where the window list is already visible.

## Screenshot

The overlay appears as a translucent panel on the edge of your screen, listing each window with its icon and title. The focused window is highlighted.

## Installation

### Dependencies

- Hyprland (v0.44+, with scrolling layout support)
- GTK4, gtk4-layer-shell
- Rust toolchain

### Install

```sh
git clone <repo> && cd window-list-overlay
./install.sh
```

This builds the binary, installs everything to `~/.local/share/window-list-overlay/`, and prints the lines to add to your Hyprland config.

## Configuration

Configuration is loaded from `~/.config/window-list-overlay/config.toml`. All fields are optional — missing fields use the defaults shown below. A missing config file is fine; everything works out of the box.

```toml
# Monitor to target (empty string = follows focused monitor)
monitor = ""

# Overlay position: "right", "left", "top", "bottom"
position = "right"

# Pixels from the anchor edge
margin = 20

# Overlay width in pixels
width = 320

# Icon size in pixels
icon_size = 24

# Font family and size for window titles
font_family = "monospace"
font_size = 14

# Background opacity (0.0 – 1.0)
opacity = 0.92

# Only show on scrolling workspaces (hides on dwindle/master)
scrolling_only = true

# Truncate window titles after this many characters
max_title_chars = 50
```

### Theme

Colors are automatically picked up from `~/.config/omarchy/current/theme/waybar.css` if it exists (via `@define-color foreground` and `@define-color background`). Falls back to Everforest dark defaults.

## Permissions

The daemon reads keyboard state from `/dev/input/event*` to detect Super key release. Your user needs to be in the `input` group:

```sh
sudo usermod -aG input $USER
```

Then log out and back in.

## License

MIT
