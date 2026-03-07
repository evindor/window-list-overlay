use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, DrawingArea, Orientation, Overlay as GtkOverlay};

/// Build a scroll-capable title widget.
/// Uses `add_tick_callback` for frame-synced animation matching the monitor refresh rate.
pub fn build_scroll_title(row: &GtkBox, title: &str, title_width: i32, scroll_speed: i32) {
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
    let (natural_width, text_height) = measure_title(&area, title.as_str());
    let overflow = (natural_width - title_width).max(0);
    area.set_content_height(text_height.max(1));

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
    fade_right.set_visible(overflow > 0);

    overlay.add_overlay(&fade_left);
    overlay.add_overlay(&fade_right);

    row.append(&overlay);

    if overflow <= 0 {
        return;
    }

    // Frame-synced scroll animation via tick callback.
    // Automatically stops when the DrawingArea is removed from the widget tree.
    let start_time = Rc::new(Cell::new(0i64));
    let fade_left_weak = fade_left.downgrade();
    let fade_right_weak = fade_right.downgrade();

    area.add_tick_callback(move |area, clock| {
        let frame_time = clock.frame_time(); // microseconds

        let start = start_time.get();
        if start == 0 {
            start_time.set(frame_time);
            return glib::ControlFlow::Continue;
        }

        let elapsed_ms = (frame_time - start) as f64 / 1000.0;
        let offset = scroll_offset(elapsed_ms, overflow, scroll_speed);
        *offset_px.borrow_mut() = offset;
        area.queue_draw();

        if let Some(fl) = fade_left_weak.upgrade() {
            fl.set_visible(offset > 0.5);
        }
        if let Some(fr) = fade_right_weak.upgrade() {
            fr.set_visible(offset < overflow as f64 - 0.5);
        }

        glib::ControlFlow::Continue
    });
}

/// Compute scroll offset based on elapsed time.
/// Cycle: pause -> ease scroll right -> pause -> ease scroll back -> repeat
fn scroll_offset(elapsed_ms: f64, overflow: i32, scroll_speed: i32) -> f64 {
    let scroll_ms = overflow as f64 / scroll_speed.max(1) as f64 * 1000.0;
    let pause_ms = 1500.0;
    let half_cycle = pause_ms + scroll_ms;
    let full_cycle = 2.0 * half_cycle;

    let t = elapsed_ms % full_cycle;
    let max = overflow as f64;

    if t < pause_ms {
        0.0
    } else if t < half_cycle {
        let progress = (t - pause_ms) / scroll_ms;
        ease_in_out(progress) * max
    } else if t < half_cycle + pause_ms {
        max
    } else {
        let progress = (t - half_cycle - pause_ms) / scroll_ms;
        (1.0 - ease_in_out(progress)) * max
    }
}

/// Cubic ease-in-out for smooth acceleration/deceleration
fn ease_in_out(t: f64) -> f64 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

pub fn measure_title(widget: &impl IsA<gtk4::Widget>, title: &str) -> (i32, i32) {
    let layout = widget.create_pango_layout(Some(title));
    layout.set_single_paragraph_mode(true);
    layout.pixel_size()
}
