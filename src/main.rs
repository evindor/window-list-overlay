mod hyprland;
mod icons;
mod keys;
mod overlay;
mod theme;

use std::cell::RefCell;
use std::fs;
use std::rc::Rc;
use std::time::Duration;

use gdk4::prelude::*;
use gtk4::prelude::*;
use gtk4::CssProvider;

const PID_FILE: &str = "/tmp/window-list-overlay.pid";
const STATIC_CSS: &str = include_str!("style.css");

fn write_pid_file() {
    let pid = std::process::id();
    let _ = fs::write(PID_FILE, pid.to_string());
}

fn remove_pid_file() {
    let _ = fs::remove_file(PID_FILE);
}

fn load_css(provider: &CssProvider) {
    let colors = theme::parse_theme();
    let dynamic_css = theme::generate_css(&colors);
    let combined = format!("{STATIC_CSS}\n{dynamic_css}");
    provider.load_from_data(&combined);
}

fn main() {
    write_pid_file();

    let app = gtk4::Application::builder()
        .application_id("com.github.window-list-overlay")
        .build();

    app.connect_activate(move |app| {
        // CSS provider
        let provider = CssProvider::new();
        load_css(&provider);
        gtk4::style_context_add_provider_for_display(
            &gdk4::Display::default().unwrap(),
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        // Find all keyboard devices for Super key polling
        let keyboards = Rc::new(keys::find_keyboards());

        // Create overlay
        let overlay = Rc::new(RefCell::new(overlay::Overlay::new(app)));

        // Present window once so it's realized (but stays hidden)
        {
            let o = overlay.borrow();
            o.window.present();
            o.window.set_visible(false);
        }

        // SIGUSR1 → show overlay + start polling timers
        {
            let overlay = Rc::clone(&overlay);
            let provider = provider.clone();
            let keyboards = Rc::clone(&keyboards);
            glib::source::unix_signal_add_local(libc::SIGUSR1, move || {
                // If Super already released by the time signal arrives, skip
                if !keyboards.is_empty() && !keys::is_super_pressed(&keyboards) {
                    return glib::ControlFlow::Continue;
                }

                load_css(&provider);
                overlay.borrow().show();

                // Poll Super key state every 50ms — auto-hide on release
                if !keyboards.is_empty() {
                    let poll_overlay = Rc::clone(&overlay);
                    let poll_kbds = Rc::clone(&keyboards);
                    glib::timeout_add_local(Duration::from_millis(50), move || {
                        if !poll_overlay.borrow().window.is_visible() {
                            return glib::ControlFlow::Break;
                        }
                        if !keys::is_super_pressed(&poll_kbds) {
                            poll_overlay.borrow().hide();
                            return glib::ControlFlow::Break;
                        }
                        glib::ControlFlow::Continue
                    });
                }

                // Refresh timer: re-populates every 200ms while visible
                let timer_overlay = Rc::clone(&overlay);
                glib::timeout_add_local(Duration::from_millis(200), move || {
                    let o = timer_overlay.borrow();
                    if o.window.is_visible() {
                        o.populate();
                        glib::ControlFlow::Continue
                    } else {
                        glib::ControlFlow::Break
                    }
                });
                glib::ControlFlow::Continue
            });
        }

        // SIGUSR2 → manual hide (fallback)
        {
            let overlay = Rc::clone(&overlay);
            glib::source::unix_signal_add_local(libc::SIGUSR2, move || {
                overlay.borrow().hide();
                glib::ControlFlow::Continue
            });
        }
    });

    app.run_with_args::<String>(&[]);

    remove_pid_file();
}
