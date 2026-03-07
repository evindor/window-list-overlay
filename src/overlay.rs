use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use gdk4::prelude::*;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, DrawingArea, Image, Label, Orientation, Overlay as GtkOverlay, Window,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::config::{self, Config, Layout, OverflowStyle, Position};
use crate::hyprland;
use crate::icons;

/// Snapshot of window list state for change detection
type WindowState = Vec<(String, String, bool)>; // (address, title, is_active)

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
    scroll_cancel: RefCell<Rc<Cell<bool>>>,
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
            scroll_cancel: RefCell::new(Rc::new(Cell::new(false))),
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

    /// Signal all running scroll animations to stop
    fn cancel_scroll_animations(&self) {
        self.scroll_cancel.borrow().set(true);
        *self.scroll_cancel.borrow_mut() = Rc::new(Cell::new(false));
    }

    /// Check if window list has changed since last populate
    fn window_list_changed(&self, clients: &[hyprland::HyprClient], active_addr: &str) -> bool {
        let new_state: WindowState = clients
            .iter()
            .map(|c| (c.address.clone(), c.title.clone(), c.address == active_addr))
            .collect();
        let changed = *self.last_window_state.borrow() != new_state;
        if changed {
            *self.last_window_state.borrow_mut() = new_state;
        }
        changed
    }

    /// Clear and rebuild the window list from hyprland state
    pub fn populate(&self) {
        let monitor_name = self.current_monitor.borrow().clone();
        let config = self.config.borrow();
        let workspace_id = match hyprland::get_active_workspace(&config.monitor) {
            Some(id) => id,
            None => {
                self.cancel_scroll_animations();
                while let Some(child) = self.container.first_child() {
                    self.container.remove(&child);
                }
                *self.last_window_state.borrow_mut() = Vec::new();
                return;
            }
        };

        // Scrolling-only filter
        if config.scrolling_only {
            let layout = hyprland::get_workspace_layout(workspace_id).unwrap_or_default();
            if layout != "scrolling" {
                self.cancel_scroll_animations();
                while let Some(child) = self.container.first_child() {
                    self.container.remove(&child);
                }
                *self.last_window_state.borrow_mut() = Vec::new();
                return;
            }
        }

        let clients = hyprland::get_workspace_clients(workspace_id);
        let active_addr = hyprland::get_active_window_address().unwrap_or_default();

        // Skip rebuild if nothing changed (preserves scroll animation state)
        if !self.window_list_changed(&clients, &active_addr) {
            return;
        }

        // Cancel existing scroll animations before rebuilding
        self.cancel_scroll_animations();

        // Clear existing children
        while let Some(child) = self.container.first_child() {
            self.container.remove(&child);
        }

        let display = gdk4::Display::default().unwrap();
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
                        build_scroll_title(
                            &row,
                            &client.title,
                            title_width,
                            config.scroll_speed,
                            &self.scroll_cancel.borrow(),
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
        self.cancel_scroll_animations();
        // Clear children when hidden to free resources
        while let Some(child) = self.container.first_child() {
            self.container.remove(&child);
        }
        *self.last_window_state.borrow_mut() = Vec::new();
    }
}

#[derive(Clone, Copy, PartialEq)]
enum ScrollPhase {
    WaitingForLayout,
    PauseStart,
    Scrolling,
    PauseEnd,
    Resetting,
}

/// Build a scroll-capable title widget with a cancellation token.
/// Uses a single timer that waits for layout, measures overflow, then animates.
fn build_scroll_title(
    row: &GtkBox,
    title: &str,
    title_width: i32,
    scroll_speed: i32,
    cancel: &Rc<Cell<bool>>,
) {
    let overlay = GtkOverlay::new();
    overlay.add_css_class("title-container");
    overlay.set_overflow(gtk4::Overflow::Hidden);

    let area = DrawingArea::new();
    area.set_content_width(title_width);
    area.set_content_height(1);
    area.set_halign(Align::Start);
    area.add_css_class("window-title");
    let title = Rc::new(title.to_string());
    let offset_px = Rc::new(RefCell::new(0f64));

    {
        let title = Rc::clone(&title);
        let offset_px = Rc::clone(&offset_px);
        area.set_draw_func(move |area, cr, width, height| {
            let layout = area.create_pango_layout(Some(title.as_str()));
            layout.set_single_paragraph_mode(true);

            let (_, text_height) = layout.pixel_size();
            let y = ((height - text_height).max(0) / 2) as f64;

            let _ = cr.save();
            cr.rectangle(0.0, 0.0, width as f64, height as f64);
            cr.clip();
            gtk4::render_layout(&area.style_context(), cr, -*offset_px.borrow(), y, &layout);
            let _ = cr.restore();
        });
    }

    overlay.set_child(Some(&area));

    // Fade overlays
    let fade_left = GtkBox::new(Orientation::Horizontal, 0);
    fade_left.add_css_class("fade-left");
    fade_left.set_halign(Align::Start);
    fade_left.set_visible(false);

    let fade_right = GtkBox::new(Orientation::Horizontal, 0);
    fade_right.add_css_class("fade-right");
    fade_right.set_halign(Align::End);
    fade_right.set_visible(false);

    overlay.add_overlay(&fade_left);
    overlay.add_overlay(&fade_right);

    row.append(&overlay);

    // Single timer: waits for layout → measures overflow → animates
    let phase = Rc::new(RefCell::new(ScrollPhase::WaitingForLayout));
    let overflow = Rc::new(RefCell::new(0i32));
    let offset = Rc::new(RefCell::new(0i32));
    let pause_ticks = Rc::new(RefCell::new(0u32));

    let fps = 30u32;
    let interval = Duration::from_millis(1000 / fps as u64);
    let px_per_tick = (scroll_speed as f64 / fps as f64).max(1.0);
    let pause_duration_ticks = (1.5 * fps as f64) as u32;

    let cancel = Rc::clone(cancel);
    let overlay_weak = overlay.downgrade();
    let area_weak = area.downgrade();
    let fade_left_weak = fade_left.downgrade();
    let fade_right_weak = fade_right.downgrade();
    glib::timeout_add_local(interval, move || {
        if cancel.get() {
            return glib::ControlFlow::Break;
        }

        let Some(overlay) = overlay_weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        let Some(area) = area_weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        let Some(fade_left) = fade_left_weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        let Some(fade_right) = fade_right_weak.upgrade() else {
            return glib::ControlFlow::Break;
        };

        if area.parent().is_none() || overlay.root().is_none() {
            return glib::ControlFlow::Break;
        }

        let mut current_phase = phase.borrow_mut();
        let mut current_offset = offset.borrow_mut();
        let mut ticks = pause_ticks.borrow_mut();

        match *current_phase {
            ScrollPhase::WaitingForLayout => {
                let available = overlay.allocated_width();
                if available <= 0 {
                    return glib::ControlFlow::Continue;
                }
                let layout = area.create_pango_layout(Some(title.as_str()));
                layout.set_single_paragraph_mode(true);
                let (natural, text_height) = layout.pixel_size();
                area.set_content_height(text_height.max(1));
                let ov = natural - available;
                if ov <= 0 {
                    return glib::ControlFlow::Break;
                }
                *overflow.borrow_mut() = ov;
                fade_right.set_visible(true);
                *current_phase = ScrollPhase::PauseStart;
            }
            ScrollPhase::PauseStart => {
                *ticks += 1;
                if *ticks >= pause_duration_ticks {
                    *ticks = 0;
                    *current_phase = ScrollPhase::Scrolling;
                }
            }
            ScrollPhase::Scrolling => {
                let max_scroll = *overflow.borrow();
                *current_offset = (*current_offset + px_per_tick as i32).min(max_scroll);
                *offset_px.borrow_mut() = *current_offset as f64;
                area.queue_draw();

                fade_left.set_visible(*current_offset > 0);
                fade_right.set_visible(*current_offset < max_scroll);

                if *current_offset >= max_scroll {
                    *current_phase = ScrollPhase::PauseEnd;
                    *ticks = 0;
                }
            }
            ScrollPhase::PauseEnd => {
                *ticks += 1;
                if *ticks >= pause_duration_ticks {
                    *ticks = 0;
                    *current_phase = ScrollPhase::Resetting;
                }
            }
            ScrollPhase::Resetting => {
                *current_offset = 0;
                *offset_px.borrow_mut() = 0.0;
                area.queue_draw();
                fade_left.set_visible(false);
                fade_right.set_visible(true);
                *current_phase = ScrollPhase::PauseStart;
            }
        }

        glib::ControlFlow::Continue
    });
}

fn build_truncated_title(title: &str, title_width: i32) -> DrawingArea {
    let area = DrawingArea::new();
    area.set_content_width(title_width);
    area.set_content_height(1);
    area.set_halign(Align::Start);
    area.add_css_class("window-title");

    let title = Rc::new(title.to_string());
    {
        let title = Rc::clone(&title);
        area.set_draw_func(move |area, cr, width, height| {
            let layout = area.create_pango_layout(Some(title.as_str()));
            layout.set_single_paragraph_mode(true);
            layout.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            layout.set_width(width.max(1) * gtk4::pango::SCALE);

            let (_, text_height) = layout.pixel_size();
            area.set_content_height(text_height.max(1));
            let y = ((height - text_height).max(0) / 2) as f64;
            gtk4::render_layout(&area.style_context(), cr, 0.0, y, &layout);
        });
    }

    area
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
