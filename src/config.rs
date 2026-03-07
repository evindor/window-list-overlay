use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Position {
    Right,
    Left,
    Top,
    Bottom,
}

impl Default for Position {
    fn default() -> Self {
        Self::Right
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    Vertical,
    Horizontal,
}

impl Default for Layout {
    fn default() -> Self {
        Self::Vertical
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverflowStyle {
    Truncate,
    Scroll,
}

impl Default for OverflowStyle {
    fn default() -> Self {
        Self::Truncate
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct MonitorOverride {
    pub position: Option<Position>,
    pub layout: Option<Layout>,
    pub margin: Option<i32>,
    pub width: Option<i32>,
    pub max_element_width: Option<i32>,
}

/// Resolved config for a specific monitor (global defaults + per-monitor overrides merged)
pub struct EffectiveConfig {
    pub position: Position,
    pub layout: Layout,
    pub margin: i32,
    pub width: i32,
    pub max_element_width: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub monitor: String,
    pub position: Position,
    pub layout: Layout,
    pub margin: i32,
    pub width: i32,
    pub icon_size: i32,
    pub font_family: String,
    pub font_size: i32,
    pub opacity: f64,
    pub scrolling_only: bool,
    pub max_title_chars: i32,
    pub max_element_width: i32,
    pub overflow_style: OverflowStyle,
    pub scroll_speed: i32,
    #[serde(default)]
    pub monitors: HashMap<String, MonitorOverride>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            monitor: String::new(),
            position: Position::Right,
            layout: Layout::Vertical,
            margin: 20,
            width: 320,
            icon_size: 24,
            font_family: "monospace".to_string(),
            font_size: 14,
            opacity: 0.92,
            scrolling_only: true,
            max_title_chars: 50,
            max_element_width: 0,
            overflow_style: OverflowStyle::default(),
            scroll_speed: 40,
            monitors: HashMap::new(),
        }
    }
}

impl Config {
    pub fn effective_for(&self, monitor: &str) -> EffectiveConfig {
        let overrides = self.monitors.get(monitor);
        EffectiveConfig {
            position: overrides
                .and_then(|o| o.position.clone())
                .unwrap_or_else(|| self.position.clone()),
            layout: overrides
                .and_then(|o| o.layout.clone())
                .unwrap_or_else(|| self.layout.clone()),
            margin: overrides
                .and_then(|o| o.margin)
                .unwrap_or(self.margin),
            width: overrides
                .and_then(|o| o.width)
                .unwrap_or(self.width),
            max_element_width: overrides
                .and_then(|o| o.max_element_width)
                .unwrap_or(self.max_element_width),
        }
    }
}

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home".to_string());
    PathBuf::from(home).join(".config/window-list-overlay/config.toml")
}

pub fn load() -> Config {
    let path = config_path();
    match fs::read_to_string(&path) {
        Ok(content) => toml::from_str(&content).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

/// Returns the mtime of the config file (or None if it doesn't exist)
pub fn config_mtime() -> Option<std::time::SystemTime> {
    fs::metadata(config_path()).ok()?.modified().ok()
}