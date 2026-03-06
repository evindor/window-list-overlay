use gdk4::prelude::*;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Image, Label, Orientation, Window};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::hyprland;
use crate::icons;

pub struct Overlay {
    pub window: Window,
    container: GtkBox,
}

impl Overlay {
    pub fn new(app: &gtk4::Application) -> Self {
        let window = Window::builder()
            .application(app)
            .default_width(320)
            .build();

        // Layer shell setup
        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_anchor(Edge::Right, true);
        window.set_margin(Edge::Right, 20);
        window.set_exclusive_zone(-1);
        window.set_keyboard_mode(KeyboardMode::None);
        window.set_namespace(Some("window-list-overlay"));

        // Target DP-1 monitor
        if let Some(monitor) = find_dp1_monitor(&window) {
            window.set_monitor(Some(&monitor));
        }

        let container = GtkBox::new(Orientation::Vertical, 4);
        container.set_valign(Align::Center);
        container.set_margin_top(20);
        container.set_margin_bottom(20);
        window.set_child(Some(&container));

        // Start hidden
        window.set_visible(false);

        Self { window, container }
    }

    /// Clear and rebuild the window list from hyprland state
    pub fn populate(&self) {
        // Clear existing children
        while let Some(child) = self.container.first_child() {
            self.container.remove(&child);
        }

        let workspace_id = match hyprland::get_dp1_active_workspace() {
            Some(id) => id,
            None => return,
        };

        let clients = hyprland::get_workspace_clients(workspace_id);
        let active_addr = hyprland::get_active_window_address().unwrap_or_default();

        let display = gdk4::Display::default().unwrap();
        let icon_theme = gtk4::IconTheme::for_display(&display);

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
            image.set_pixel_size(24);
            image.add_css_class("window-icon");
            row.append(&image);

            // Title
            let title_text = if client.title.len() > 60 {
                format!("{}…", &client.title[..59])
            } else {
                client.title.clone()
            };
            let label = Label::new(Some(&title_text));
            label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            label.set_max_width_chars(30);
            label.set_xalign(0.0);
            label.add_css_class("window-title");
            row.append(&label);

            self.container.append(&row);
        }

        // If no windows, show a hint
        if clients.is_empty() {
            let label = Label::new(Some("No windows"));
            label.add_css_class("window-row");
            label.set_opacity(0.5);
            self.container.append(&label);
        }
    }

    pub fn show(&self) {
        self.populate();
        self.window.set_visible(true);
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
        // Clear children when hidden to free resources
        while let Some(child) = self.container.first_child() {
            self.container.remove(&child);
        }
    }
}

fn find_dp1_monitor(window: &Window) -> Option<gdk4::Monitor> {
    let display = gtk4::prelude::WidgetExt::display(window);
    let monitors = display.monitors();
    for i in 0..monitors.n_items() {
        let obj = monitors.item(i)?;
        let monitor: gdk4::Monitor = obj.downcast().ok()?;
        if monitor.connector().as_deref() == Some("DP-1") {
            return Some(monitor);
        }
    }
    None
}
