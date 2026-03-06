# Plan: Scrolling DP-1 + Window List Overlay

## Context
- **System:** Omarchy (Arch Linux), Hyprland v0.54.0
- **Monitors:** HDMI-A-1 (primary, landscape, 3840x2160@180Hz) + DP-1 (vertical/portrait, 3840x2160@60Hz)
- **Plugin:** split-monitor-workspaces v1.2.0, count=5
  - HDMI-A-1: workspaces 1-5
  - DP-1: workspaces 6-10
- **Current scrolling:** workspace 5 (HDMI-A-1) and workspace 10 (DP-1)
- **Goal:** Make ALL DP-1 workspaces scrolling + add a hidden overlay that shows app icons/titles on SUPER hold

---

## Part 1: All DP-1 workspaces scrolling

**File:** `~/.config/hypr/hyprland.conf`

Add workspace rules for 6-9 alongside existing 5 and 10:
```
workspace = 6, monitor:DP-1, layout:scrolling, layoutopt:direction:down
workspace = 7, monitor:DP-1, layout:scrolling, layoutopt:direction:down
workspace = 8, monitor:DP-1, layout:scrolling, layoutopt:direction:down
workspace = 9, monitor:DP-1, layout:scrolling, layoutopt:direction:down
```

Existing rules stay unchanged:
```
workspace = 5, monitor:HDMI-A-1, layout:scrolling
workspace = 10, monitor:DP-1, layout:scrolling, layoutopt:direction:down
```

---

## Part 2: Window list overlay on SUPER hold

### Approach: Python GTK4 + gtk4-layer-shell daemon

**Dependencies (all verified installed):**
- Python 3.14.3 (`/usr/bin/python3`)
- python-gobject (3.54.5) — `import gi` works
- gtk4-layer-shell (1.3.0) — `gi.require_version('Gtk4LayerShell', '1.0')` works
- Gio 2.0 — for desktop file icon resolution

### New files

| File | Purpose |
|------|---------|
| `~/.config/hypr/scripts/window-list-overlay.py` | GTK4 daemon: layer-shell overlay on DP-1 right edge |
| `~/.config/hypr/scripts/window-list-show.sh` | 200ms debounced show trigger |
| `~/.config/hypr/scripts/window-list-hide.sh` | 200ms debounced hide trigger |

### Modified files

| File | Change |
|------|--------|
| `~/.config/hypr/hyprland.conf` | Add workspace 6-9 scrolling rules |
| `~/.config/hypr/bindings.conf` | Add `bind = , Super_L` / `bindr = , Super_L` for show/hide |
| `~/.config/hypr/autostart.conf` | Add `exec-once = /usr/bin/python3 ~/.config/hypr/scripts/window-list-overlay.py` |

---

### Debounce mechanism (200ms)

**Problem:** SUPER press fires on every SUPER+key combo (SUPER+1, SUPER+TAB, etc.), causing overlay to flash.

**Solution:** Both show and hide scripts use 200ms delayed execution with PID-based cancellation.

**Flow for SUPER+1 (workspace switch, no flash):**
1. SUPER press → show.sh starts 200ms timer, saves background PID to `/tmp/window-list-show.pid`
2. "1" key pressed → workspace switches (unrelated to overlay)
3. SUPER released (<200ms) → hide.sh kills pending show PID, overlay never appeared
4. Result: no flash

**Flow for SUPER hold (overlay appears):**
1. SUPER press → show.sh starts 200ms timer
2. 200ms passes, SUPER still held → timer completes, sends SIGUSR1 to daemon, overlay appears
3. User browses windows with SUPER+UP/DOWN (overlay stays visible, hide debounce prevents flicker)
4. SUPER released → hide.sh starts 200ms timer
5. 200ms passes → sends SIGUSR2, overlay hides

**Flow for quick SUPER re-press (no flicker):**
1. SUPER released → hide.sh starts 200ms timer
2. SUPER pressed again (<200ms) → show.sh cancels pending hide PID
3. Result: overlay stays visible

---

### Overlay daemon design (`window-list-overlay.py`)

**Architecture:**
- Runs as background daemon, creates GTK4 window with layer-shell
- Listens for Unix signals via `GLib.unix_signal_add()`:
  - SIGUSR1 → show (refresh window list + set visible)
  - SIGUSR2 → hide
- PID written to `/tmp/window-list-overlay.pid`

**Layer shell config:**
- **Monitor:** DP-1 (found via `Gdk.Display.get_monitors()`, matched by connector name)
- **Layer:** OVERLAY (above everything)
- **Anchor:** RIGHT edge, centered vertically
- **Margin:** 20px from right edge
- **Exclusive zone:** -1 (floats over content, doesn't push windows)
- **Keyboard mode:** NONE (critical — must not steal focus, or SUPER release won't be detected)
- **Namespace:** `"window-list-overlay"` (for layer rules if needed)

**Content on show:**
1. Query `hyprctl monitors -j` → find DP-1's active workspace ID
2. Query `hyprctl clients -j` → filter by that workspace ID + `mapped=true`
3. Query `hyprctl activewindow -j` → get focused window address
4. For each window, create a row with:
   - App icon (resolved via `Gio.DesktopAppInfo` from window class → desktop file → Icon= field)
   - Window title (truncated to 30 chars)
   - Active highlight CSS class if address matches active window

**Icon resolution strategy (in order):**
1. `Gio.DesktopAppInfo.new(wm_class + ".desktop")` — works for most apps
2. `Gio.DesktopAppInfo.new(wm_class.lower() + ".desktop")` — case-insensitive fallback
3. Direct icon theme lookup by class name and last segment after "." (e.g., `com.mitchellh.ghostty` → `ghostty`)
4. Fallback: `application-x-executable` generic icon

**Styling (matches current Omarchy theme):**
- Background: `rgba(45, 53, 59, 0.92)` (from `@define-color background #2d353b`)
- Foreground: `#d3c6aa` (from `@define-color foreground #d3c6aa`)
- Font: `JetBrainsMono Nerd Font` (matches waybar)
- Border radius: 12px
- Active row: slightly lighter background + bold text

---

### Show script (`window-list-show.sh`)

```bash
#!/bin/bash
PIDFILE="/tmp/window-list-overlay.pid"
SHOW_PIDFILE="/tmp/window-list-show.pid"
HIDE_PIDFILE="/tmp/window-list-hide.pid"

# Cancel any pending hide
[ -f "$HIDE_PIDFILE" ] && kill "$(cat "$HIDE_PIDFILE")" 2>/dev/null && rm -f "$HIDE_PIDFILE"

# Start debounced show in background
(
    echo $$ > "$SHOW_PIDFILE"
    sleep 0.2
    rm -f "$SHOW_PIDFILE"
    [ -f "$PIDFILE" ] && kill -USR1 "$(cat "$PIDFILE")" 2>/dev/null
) &
disown
```

### Hide script (`window-list-hide.sh`)

```bash
#!/bin/bash
PIDFILE="/tmp/window-list-overlay.pid"
SHOW_PIDFILE="/tmp/window-list-show.pid"
HIDE_PIDFILE="/tmp/window-list-hide.pid"

# Cancel any pending show
[ -f "$SHOW_PIDFILE" ] && kill "$(cat "$SHOW_PIDFILE")" 2>/dev/null && rm -f "$SHOW_PIDFILE"

# Cancel any previous pending hide
[ -f "$HIDE_PIDFILE" ] && kill "$(cat "$HIDE_PIDFILE")" 2>/dev/null && rm -f "$HIDE_PIDFILE"

# Start debounced hide in background
(
    echo $$ > "$HIDE_PIDFILE"
    sleep 0.2
    rm -f "$HIDE_PIDFILE"
    [ -f "$PIDFILE" ] && kill -USR2 "$(cat "$PIDFILE")" 2>/dev/null
) &
disown
```

---

### Keybindings (`~/.config/hypr/bindings.conf`)

```
# Window list overlay on SUPER hold (DP-1)
bind = , Super_L, exec, ~/.config/hypr/scripts/window-list-show.sh
bindr = , Super_L, exec, ~/.config/hypr/scripts/window-list-hide.sh
```

**Syntax notes:**
- Empty modifier field + `Super_L` key — binds to the physical Super key itself
- `bindr` flag = release trigger
- These don't interfere with SUPER+key combos in split-monitor.conf

### Autostart (`~/.config/hypr/autostart.conf`)

```
exec-once = /usr/bin/python3 ~/.config/hypr/scripts/window-list-overlay.py
```

Uses system Python (`/usr/bin/python3`), not mise-managed Python, because system Python has PyGObject + GTK4 bindings.

---

### Verification steps

1. `hyprctl reload` after config changes
2. Launch daemon: `/usr/bin/python3 ~/.config/hypr/scripts/window-list-overlay.py &`
3. Open a few windows on DP-1
4. Hold SUPER for >200ms → overlay should appear on DP-1 right edge with window list
5. Quick SUPER+1 combo → verify no flash
6. SUPER+UP/DOWN while holding SUPER → verify overlay stays visible, active highlight updates on next show
7. Release SUPER → overlay hides after 200ms
8. Switch to different workspace on DP-1, hold SUPER → verify overlay shows that workspace's windows
9. Verify all DP-1 workspaces (6-10) use scrolling layout

---

### Existing config reference

**split-monitor.conf** (count=5):
- HDMI-A-1 (monitor 0): workspaces 1-5
- DP-1 (monitor 1): workspaces 6-10
- SUPER+[1-5]: `split-workspace` (relative)
- SUPER+TAB: `split-cycleworkspaces`
- SUPER+ALT+[1-5]: move all windows script

**waybar config.jsonc**:
- persistent-workspaces: 1-5 on HDMI-A-1, 6-10 on DP-1
- format-icons: 6-10 display as "1"-"5"

**Theme colors** (`~/.config/omarchy/current/theme/waybar.css`):
- `@define-color foreground #d3c6aa`
- `@define-color background #2d353b`
