#!/bin/bash
set -e

INSTALL_DIR="$HOME/.local/share/window-list-overlay"
CONFIG_DIR="$HOME/.config/window-list-overlay"

echo "Building window-list-overlay..."
cargo build --release

echo "Installing to $INSTALL_DIR/"
mkdir -p "$INSTALL_DIR" "$CONFIG_DIR"

cp target/release/window-list-overlay "$INSTALL_DIR/"
cp scripts/window-list-overlay-show "$INSTALL_DIR/"
cp scripts/window-list-overlay-hide "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/window-list-overlay"
chmod +x "$INSTALL_DIR/window-list-overlay-show"
chmod +x "$INSTALL_DIR/window-list-overlay-hide"

if [ ! -f "$CONFIG_DIR/config.toml" ]; then
    cat > "$CONFIG_DIR/config.toml" << 'TOML'
# Window List Overlay Configuration
# All fields are optional — defaults are shown below (commented out).

# monitor = ""
# position = "right"
# layout = "vertical"
# margin = 20
# width = 320
# icon_size = 24
# font_family = "monospace"
# font_size = 14
# opacity = 0.92
# scrolling_only = true
# max_title_chars = 50

# Per-monitor overrides (uncomment and adjust for your setup):
# [monitors.DP-1]
# position = "right"
# layout = "horizontal"
#
# [monitors.HDMI-A-1]
# position = "top"
# layout = "vertical"
TOML
    echo "Created default config at $CONFIG_DIR/config.toml"
fi

echo ""
echo "Installed to $INSTALL_DIR/"
echo ""
echo "Add these lines to your Hyprland config (~/.config/hypr/hyprland.conf):"
echo ""
echo "  exec-once = $INSTALL_DIR/window-list-overlay"
echo "  bind = , Super_L, exec, $INSTALL_DIR/window-list-overlay-show"
echo "  layerrule = noanim, match:namespace window-list-overlay"
echo ""
echo "Config: $CONFIG_DIR/config.toml"
echo ""
echo "NOTE: The overlay reads keyboard state from /dev/input/event*."
echo "If you're not in the 'input' group: sudo usermod -aG input \$USER"
