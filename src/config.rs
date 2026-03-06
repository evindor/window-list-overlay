use serde::Deserialize;
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
#[serde(default)]
pub struct Config {
    pub monitor: String,
    pub position: Position,
    pub margin: i32,
    pub width: i32,
    pub icon_size: i32,
    pub font_family: String,
    pub font_size: i32,
    pub opacity: f64,
    pub scrolling_only: bool,
    pub max_title_chars: i32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            monitor: String::new(),
            position: Position::Right,
            margin: 20,
            width: 320,
            icon_size: 24,
            font_family: "monospace".to_string(),
            font_size: 14,
            opacity: 0.92,
            scrolling_only: true,
            max_title_chars: 50,
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
