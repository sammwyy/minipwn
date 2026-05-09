use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use crate::config::themes_dir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub author: String,
    pub colors: ThemeColors,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeColors {
    pub primary: String,
    pub secondary: String,
    pub background: String,
    pub surface: String,
    pub text: String,
    pub text_dim: String,
    pub success: String,
    pub warning: String,
    pub error: String,
    pub user_bubble: String,
    pub assistant_bubble: String,
    pub tool_bubble: String,
    pub border: String,
    pub title: String,
}

impl Theme {
    pub fn get_color(&self, color_str: &str) -> Color {
        parse_color(color_str).unwrap_or(Color::White)
    }

    pub fn primary(&self) -> Color { self.get_color(&self.colors.primary) }
    pub fn secondary(&self) -> Color { self.get_color(&self.colors.secondary) }
    pub fn background(&self) -> Color { self.get_color(&self.colors.background) }
    pub fn surface(&self) -> Color { self.get_color(&self.colors.surface) }
    pub fn text(&self) -> Color { self.get_color(&self.colors.text) }
    pub fn text_dim(&self) -> Color { self.get_color(&self.colors.text_dim) }
    pub fn success(&self) -> Color { self.get_color(&self.colors.success) }
    // pub fn warning(&self) -> Color { self.get_color(&self.colors.warning) }
    pub fn error(&self) -> Color { self.get_color(&self.colors.error) }
    pub fn user_bubble(&self) -> Color { self.get_color(&self.colors.user_bubble) }
    pub fn assistant_bubble(&self) -> Color { self.get_color(&self.colors.assistant_bubble) }
    // pub fn tool_bubble(&self) -> Color { self.get_color(&self.colors.tool_bubble) }
    // pub fn border(&self) -> Color { self.get_color(&self.colors.border) }
    // pub fn title(&self) -> Color { self.get_color(&self.colors.title) }
}

fn parse_color(s: &str) -> Option<Color> {
    if s.starts_with('#') && s.len() == 7 {
        let r = u8::from_str_radix(&s[1..3], 16).ok()?;
        let g = u8::from_str_radix(&s[3..5], 16).ok()?;
        let b = u8::from_str_radix(&s[5..7], 16).ok()?;
        Some(Color::Rgb(r, g, b))
    } else {
        match s.to_lowercase().as_str() {
            "black" => Some(Color::Black),
            "red" => Some(Color::Red),
            "green" => Some(Color::Green),
            "yellow" => Some(Color::Yellow),
            "blue" => Some(Color::Blue),
            "magenta" => Some(Color::Magenta),
            "cyan" => Some(Color::Cyan),
            "gray" => Some(Color::Gray),
            "darkgray" => Some(Color::DarkGray),
            "lightred" => Some(Color::LightRed),
            "lightgreen" => Some(Color::LightGreen),
            "lightyellow" => Some(Color::LightYellow),
            "lightblue" => Some(Color::LightBlue),
            "lightmagenta" => Some(Color::LightMagenta),
            "lightcyan" => Some(Color::LightCyan),
            "white" => Some(Color::White),
            _ => None,
        }
    }
}

pub struct ThemeRegistry {
    themes: HashMap<String, Theme>,
}

impl ThemeRegistry {
    pub fn load() -> Self {
        let mut themes = HashMap::new();

        // 1. Load embedded themes
        let embedded = [
            ("dracula", include_str!("../../data/themes/dracula.toml")),
            ("matrix", include_str!("../../data/themes/matrix.toml")),
            ("cyberpunk", include_str!("../../data/themes/cyberpunk.toml")),
            ("nord", include_str!("../../data/themes/nord.toml")),
        ];

        for (id, content) in embedded {
            if let Ok(theme) = toml::from_str::<Theme>(content) {
                themes.insert(id.to_string(), theme);
            }
        }

        // 2. Load user themes from themes_dir()
        if let Ok(dir) = themes_dir() {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map_or(false, |ext| ext == "toml") {
                        if let Some(id) = path.file_stem().and_then(|s| s.to_str()) {
                            if let Ok(content) = fs::read_to_string(&path) {
                                if let Ok(theme) = toml::from_str::<Theme>(&content) {
                                    themes.insert(id.to_string(), theme);
                                }
                            }
                        }
                    }
                }
            }
        }

        Self { themes }
    }

    pub fn get(&self, id: &str) -> Option<&Theme> {
        self.themes.get(id)
    }

    pub fn list(&self) -> Vec<(&String, &Theme)> {
        let mut list: Vec<_> = self.themes.iter().collect();
        list.sort_by(|a, b| a.0.cmp(b.0));
        list
    }
}
