use gdk4::prelude::*;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Image, Label, Orientation, Window};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::config::{Config, Position};
use crate::hyprland;
use crate::icons;

pub struct Overlay {
    pub window: Window,
    container: GtkBox,
    config: Config,
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

        match config.position {
            Position::Right => {
                window.set_anchor(Edge::Right, true);
                window.set_margin(Edge::Right, config.margin);
            }
            Position::Left => {
                window.set_anchor(Edge::Left, true);
                window.set_margin(Edge::Left, config.margin);
            }
            Position::Top => {
                window.set_anchor(Edge::Top, true);
                window.set_margin(Edge::Top, config.margin);
            }
            Position::Bottom => {
                window.set_anchor(Edge::Bottom, true);
                window.set_margin(Edge::Bottom, config.margin);
            }
        }

        window.set_exclusive_zone(-1);
        window.set_keyboard_mode(KeyboardMode::None);
        window.set_namespace(Some("window-list-overlay"));

        // Target monitor: empty = focused (layer shell default), non-empty = lookup by name
        if !config.monitor.is_empty() {
            if let Some(monitor) = find_monitor_by_name(&window, &config.monitor) {
                window.set_monitor(Some(&monitor));
            }
        }

        let container = GtkBox::new(Orientation::Vertical, 4);
        container.set_valign(Align::Center);
        window.set_child(Some(&container));

        // Start hidden
        window.set_visible(false);

        Self {
            window,
            container,
            config: config.clone(),
        }
    }

    /// Clear and rebuild the window list from hyprland state
    pub fn populate(&self) {
        // Clear existing children
        while let Some(child) = self.container.first_child() {
            self.container.remove(&child);
        }

        let workspace_id = match hyprland::get_active_workspace(&self.config.monitor) {
            Some(id) => id,
            None => return,
        };

        // Scrolling-only filter
        if self.config.scrolling_only {
            let layout = hyprland::get_workspace_layout(workspace_id).unwrap_or_default();
            if layout != "scrolling" {
                return;
            }
        }

        let clients = hyprland::get_workspace_clients(workspace_id);
        let active_addr = hyprland::get_active_window_address().unwrap_or_default();

        let display = gdk4::Display::default().unwrap();
        let icon_theme = gtk4::IconTheme::for_display(&display);

        let max_chars = self.config.max_title_chars as usize;

        for client in &clients {
            let row = GtkBox::new(Orientation::Horizontal, 8);
            row.add_css_class("window-row");

            let is_active = client.address == active_addr;
            if is_active {
                row.add_css_class("active");
            }

            // Icon
            let icon_name = icons::resolve_icon(&client.class, &icon_theme);
            let image = Image::from_icon_name(&icon_name);
            image.set_pixel_size(self.config.icon_size);
            image.add_css_class("window-icon");
            row.append(&image);

            // Title — Unicode-safe truncation
            let title_text: String = if client.title.chars().count() > max_chars {
                let mut t: String = client.title.chars().take(max_chars - 1).collect();
                t.push('…');
                t
            } else {
                client.title.clone()
            };
            let label = Label::new(Some(&title_text));
            label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            label.set_max_width_chars(max_chars as i32);
            label.set_xalign(0.0);
            label.add_css_class("window-title");
            row.append(&label);

            self.container.append(&row);
        }
    }

    /// Move overlay to the focused monitor (when config.monitor is empty)
    fn update_monitor(&self) {
        if !self.config.monitor.is_empty() {
            return;
        }
        if let Some(name) = hyprland::get_focused_monitor_name() {
            if let Some(monitor) = find_monitor_by_name(&self.window, &name) {
                self.window.set_monitor(Some(&monitor));
            }
        }
    }

    pub fn show(&self) {
        self.update_monitor();
        self.populate();
        // Only show if there are children to display
        if self.container.first_child().is_some() {
            self.window.set_visible(true);
        }
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
        // Clear children when hidden to free resources
        while let Some(child) = self.container.first_child() {
            self.container.remove(&child);
        }
    }
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
