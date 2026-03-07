use std::fs;
use std::path::PathBuf;

use crate::config::Config;

const DEFAULT_FG: &str = "#d3c6aa";
const DEFAULT_BG: &str = "#2d353b";

pub struct ThemeColors {
    pub foreground: String,
    pub background: String,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            foreground: DEFAULT_FG.to_string(),
            background: DEFAULT_BG.to_string(),
        }
    }
}

fn waybar_css_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home".to_string());
    PathBuf::from(home).join(".config/omarchy/current/theme/waybar.css")
}

/// Parse `@define-color name #hex;` lines from waybar.css
pub fn parse_theme() -> ThemeColors {
    let path = waybar_css_path();
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return ThemeColors::default(),
    };

    let mut colors = ThemeColors::default();

    for line in content.lines() {
        let line = line.trim();
        if !line.starts_with("@define-color ") {
            continue;
        }
        // Format: @define-color name value;
        let rest = &line["@define-color ".len()..];
        let mut parts = rest.splitn(2, ' ');
        let name = match parts.next() {
            Some(n) => n.trim(),
            None => continue,
        };
        let value = match parts.next() {
            Some(v) => v.trim().trim_end_matches(';').trim(),
            None => continue,
        };
        match name {
            "foreground" => colors.foreground = value.to_string(),
            "background" => colors.background = value.to_string(),
            _ => {}
        }
    }

    colors
}

/// Parse a hex color (#RRGGBB or #RGB) into (r, g, b) components
fn hex_to_rgb(hex: &str) -> (u8, u8, u8) {
    let hex = hex.trim_start_matches('#');
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).unwrap_or(0);
            (r, g, b)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            (r, g, b)
        }
        _ => (0, 0, 0),
    }
}

/// Blend a foreground color at a given alpha over a background color
fn blend_alpha(fg: (u8, u8, u8), bg: (u8, u8, u8), alpha: f64) -> (u8, u8, u8) {
    let blend = |f: u8, b: u8| -> u8 {
        (f as f64 * alpha + b as f64 * (1.0 - alpha)).round() as u8
    };
    (blend(fg.0, bg.0), blend(fg.1, bg.1), blend(fg.2, bg.2))
}

/// Generate dynamic CSS using theme colors and config
pub fn generate_css(colors: &ThemeColors, config: &Config) -> String {
    let fg = &colors.foreground;
    let bg = &colors.background;
    let opacity = config.opacity;
    let font_family = &config.font_family;
    let font_size = config.font_size;
    // Compute blended colors for fade gradients
    let bg_rgb = hex_to_rgb(bg);
    let fg_rgb = hex_to_rgb(fg);

    // Inactive row bg = base bg at window opacity
    let inactive_bg = bg_rgb;
    let inactive_a = opacity;

    // Active row bg = fg at 0.12 alpha composited over bg
    let active_bg = blend_alpha(fg_rgb, bg_rgb, 0.12);
    let active_a = opacity;

    format!(
        r#"window {{
  background-color: alpha({bg}, {opacity});
  border: 1px solid alpha({fg}, 0.15);
}}
.window-row {{
  color: {fg};
}}
.window-row.active {{
  background-color: alpha({fg}, 0.12);
}}
.window-title {{
  color: {fg};
  font-family: '{font_family}';
  font-size: {font_size}px;
}}
.window-row .fade-left {{
  background: linear-gradient(to right, rgba({ir},{ig},{ib},{inactive_a}), transparent);
}}
.window-row .fade-right {{
  background: linear-gradient(to left, rgba({ir},{ig},{ib},{inactive_a}), transparent);
}}
.window-row.active .fade-left {{
  background: linear-gradient(to right, rgba({ar},{ag},{ab},{active_a}), transparent);
}}
.window-row.active .fade-right {{
  background: linear-gradient(to left, rgba({ar},{ag},{ab},{active_a}), transparent);
}}
"#,
        ir = inactive_bg.0,
        ig = inactive_bg.1,
        ib = inactive_bg.2,
        ar = active_bg.0,
        ag = active_bg.1,
        ab = active_bg.2,
    )
}
