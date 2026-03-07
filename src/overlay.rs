use std::cell::RefCell;
use std::time::SystemTime;

use gdk4::prelude::*;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Image, Label, Orientation, Window};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::config::{self, Config, Layout, OverflowStyle, Position};
use crate::hyprland;
use crate::icons;
use crate::scroller;

/// Snapshot of window list state for change detection (address, title)
type WindowState = Vec<(String, String)>;

const ROW_HORIZONTAL_PADDING: i32 = 32;
const ROW_CHILD_SPACING: i32 = 8;
const ICON_TRAILING_MARGIN: i32 = 12;

pub struct Overlay {
    pub window: Window,
    container: GtkBox,
    pub config: RefCell<Config>,
    config_mtime: RefCell<Option<SystemTime>>,
    current_monitor: RefCell<String>,
    last_window_state: RefCell<WindowState>,
    last_active_addr: RefCell<String>,
}

impl Overlay {
    pub fn new(app: &gtk4::Application, config: &Config) -> Self {
        let window = Window::builder()
            .application(app)
            .default_width(config.width)
            .build();

        // Layer shell setup
        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_exclusive_zone(-1);
        window.set_keyboard_mode(KeyboardMode::None);
        window.set_namespace(Some("window-list-overlay"));

        let container = GtkBox::new(Orientation::Vertical, 4);
        container.set_valign(Align::Center);
        window.set_child(Some(&container));

        // Start hidden
        window.set_visible(false);

        Self {
            window,
            container,
            config: RefCell::new(config.clone()),
            config_mtime: RefCell::new(config::config_mtime()),
            current_monitor: RefCell::new(String::new()),
            last_window_state: RefCell::new(Vec::new()),
            last_active_addr: RefCell::new(String::new()),
        }
    }

    /// Determine which monitor to show on, set GDK monitor, return the name
    fn resolve_monitor(&self) -> String {
        let config = self.config.borrow();
        if !config.monitor.is_empty() {
            // Pinned to a specific monitor
            if let Some(monitor) = find_monitor_by_name(&self.window, &config.monitor) {
                self.window.set_monitor(Some(&monitor));
            }
            return config.monitor.clone();
        }
        // Follow focused monitor
        if let Some(name) = hyprland::get_focused_monitor_name() {
            if let Some(monitor) = find_monitor_by_name(&self.window, &name) {
                self.window.set_monitor(Some(&monitor));
            }
            return name;
        }
        String::new()
    }

    /// Reconfigure anchors, margins, and container orientation for the given monitor
    fn apply_layout(&self, monitor_name: &str) {
        let effective = self.config.borrow().effective_for(monitor_name);

        // Reset all anchors and margins
        for edge in [Edge::Right, Edge::Left, Edge::Top, Edge::Bottom] {
            self.window.set_anchor(edge, false);
            self.window.set_margin(edge, 0);
        }

        // Apply position
        match effective.position {
            Position::Right => {
                self.window.set_anchor(Edge::Right, true);
                self.window.set_margin(Edge::Right, effective.margin);
            }
            Position::Left => {
                self.window.set_anchor(Edge::Left, true);
                self.window.set_margin(Edge::Left, effective.margin);
            }
            Position::Top => {
                self.window.set_anchor(Edge::Top, true);
                self.window.set_margin(Edge::Top, effective.margin);
            }
            Position::Bottom => {
                self.window.set_anchor(Edge::Bottom, true);
                self.window.set_margin(Edge::Bottom, effective.margin);
            }
        }

        // Apply layout orientation
        let win_width = if effective.max_element_width > 0 {
            effective.max_element_width.min(effective.width)
        } else {
            effective.width
        };

        match effective.layout {
            Layout::Vertical => {
                self.container.set_orientation(Orientation::Vertical);
                self.container.set_valign(Align::Center);
                self.container.set_halign(Align::Fill);
                self.window.set_default_size(win_width, -1);
            }
            Layout::Horizontal => {
                self.container.set_orientation(Orientation::Horizontal);
                self.container.set_halign(Align::Center);
                self.container.set_valign(Align::Fill);
                self.window.set_default_size(-1, -1);
            }
        }
    }

    /// Remove all children and reset window state.
    /// Tick-callback animations stop automatically when widgets are dropped.
    fn clear_content(&self) {
        while let Some(child) = self.container.first_child() {
            self.container.remove(&child);
        }
        *self.last_window_state.borrow_mut() = Vec::new();
        *self.last_active_addr.borrow_mut() = String::new();
    }

    /// Check if the window list (addresses + titles) has changed since last populate
    fn window_list_changed(&self, clients: &[hyprland::HyprClient]) -> bool {
        let new_state: WindowState = clients
            .iter()
            .map(|c| (c.address.clone(), c.title.clone()))
            .collect();
        let changed = *self.last_window_state.borrow() != new_state;
        if changed {
            *self.last_window_state.borrow_mut() = new_state;
        }
        changed
    }

    /// Update active highlight CSS classes on existing rows without rebuilding
    fn update_active_highlight(&self, active_addr: &str, clients: &[hyprland::HyprClient]) {
        let mut child = self.container.first_child();
        for client in clients {
            let Some(row) = child else { break };
            if client.address == active_addr {
                row.add_css_class("active");
            } else {
                row.remove_css_class("active");
            }
            child = row.next_sibling();
        }
        *self.last_active_addr.borrow_mut() = active_addr.to_string();
    }

    /// Clear and rebuild the window list from hyprland state
    pub fn populate(&self) {
        let monitor_name = self.current_monitor.borrow().clone();
        let config = self.config.borrow();
        let workspace_id = match hyprland::get_active_workspace(&config.monitor) {
            Some(id) => id,
            None => {
                self.clear_content();
                return;
            }
        };

        // Scrolling-only filter
        if config.scrolling_only {
            let layout = hyprland::get_workspace_layout(workspace_id).unwrap_or_default();
            if layout != "scrolling" {
                self.clear_content();
                return;
            }
        }

        let clients = hyprland::get_workspace_clients(workspace_id);
        let active_addr = hyprland::get_active_window_address().unwrap_or_default();

        let list_changed = self.window_list_changed(&clients);
        let active_changed = active_addr != *self.last_active_addr.borrow();

        if !list_changed && !active_changed {
            return;
        }

        // If only the active window changed, update CSS classes without rebuilding
        if !list_changed {
            self.update_active_highlight(&active_addr, &clients);
            return;
        }

        // Full rebuild: remove children (tick callbacks stop when widgets are dropped)
        while let Some(child) = self.container.first_child() {
            self.container.remove(&child);
        }

        let display = gdk4::Display::default().expect("GTK display not available");
        let icon_theme = gtk4::IconTheme::for_display(&display);

        let effective = config.effective_for(&monitor_name);
        let max_chars = config.max_title_chars as usize;
        let max_width = effective.max_element_width;
        let title_width = title_width_budget(max_width, config.icon_size);

        for client in &clients {
            let row = GtkBox::new(Orientation::Horizontal, 8);
            row.add_css_class("window-row");
            row.set_tooltip_text(Some(&client.title));

            let is_active = client.address == active_addr;
            if is_active {
                row.add_css_class("active");
            }

            // Icon
            let icon_name = icons::resolve_icon(&client.class, &icon_theme);
            let image = Image::from_icon_name(&icon_name);
            image.set_pixel_size(config.icon_size);
            image.add_css_class("window-icon");
            row.append(&image);

            if let Some(title_width) = title_width {
                match config.overflow_style {
                    OverflowStyle::Truncate => {
                        row.append(&build_truncated_title(&client.title, title_width));
                    }
                    OverflowStyle::Scroll => {
                        scroller::build_scroll_title(
                            &row,
                            &client.title,
                            title_width,
                            config.scroll_speed,
                        );
                    }
                }
            } else {
                // Legacy character-based truncation
                let title_text: String = if client.title.chars().count() > max_chars {
                    let mut t: String = client.title.chars().take(max_chars - 1).collect();
                    t.push('\u{2026}');
                    t
                } else {
                    client.title.clone()
                };
                let label = Label::new(Some(&title_text));
                label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                label.set_xalign(0.0);
                label.add_css_class("window-title");

                match effective.layout {
                    Layout::Vertical => {
                        label.set_max_width_chars(max_chars as i32);
                    }
                    Layout::Horizontal => {}
                }

                row.append(&label);
            }

            self.container.append(&row);
        }

        *self.last_active_addr.borrow_mut() = active_addr;
    }

    /// Reload config from disk only if the file has been modified
    pub fn reload_config_if_changed(&self) -> bool {
        let current_mtime = config::config_mtime();
        if current_mtime == *self.config_mtime.borrow() {
            return false;
        }
        *self.config.borrow_mut() = config::load();
        *self.config_mtime.borrow_mut() = current_mtime;
        true
    }

    pub fn show(&self) {
        let monitor_name = self.resolve_monitor();
        *self.current_monitor.borrow_mut() = monitor_name.clone();
        self.apply_layout(&monitor_name);
        self.populate();
        // Only show if there are children to display
        if self.container.first_child().is_some() {
            self.window.set_visible(true);
        }
    }

    /// Refresh while visible: re-populate, and move to new monitor if focus changed
    pub fn refresh(&self) {
        let new_monitor = self.resolve_monitor();
        let current = self.current_monitor.borrow().clone();

        if new_monitor != current {
            *self.current_monitor.borrow_mut() = new_monitor.clone();
            self.apply_layout(&new_monitor);
        }

        self.populate();

        // Hide if populate yielded nothing (e.g. non-scrolling workspace on new monitor)
        if self.container.first_child().is_none() {
            self.window.set_visible(false);
        }
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
        self.clear_content();
    }
}

fn build_truncated_title(title: &str, title_width: i32) -> Label {
    let label = Label::new(Some(title));
    label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    label.set_xalign(0.0);
    label.set_size_request(title_width, -1);
    label.add_css_class("window-title");
    label
}

fn title_width_budget(max_element_width: i32, icon_size: i32) -> Option<i32> {
    if max_element_width <= 0 {
        return None;
    }

    let reserved = ROW_HORIZONTAL_PADDING + ROW_CHILD_SPACING + ICON_TRAILING_MARGIN + icon_size;
    Some((max_element_width - reserved).max(1))
}

fn find_monitor_by_name(window: &Window, name: &str) -> Option<gdk4::Monitor> {
    let display = gtk4::prelude::WidgetExt::display(window);
    let monitors = display.monitors();
    for i in 0..monitors.n_items() {
        let obj = monitors.item(i)?;
        let monitor: gdk4::Monitor = obj.downcast().ok()?;
        if monitor.connector().as_deref() == Some(name) {
            return Some(monitor);
        }
    }
    None
}
