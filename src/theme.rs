use std::fs;
use std::path::PathBuf;

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

/// Generate dynamic CSS using theme colors
pub fn generate_css(colors: &ThemeColors) -> String {
    let fg = &colors.foreground;
    let bg = &colors.background;
    format!(
        r#"window {{
  background-color: alpha({bg}, 0.92);
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
}}
"#
    )
}
