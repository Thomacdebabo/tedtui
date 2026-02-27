use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;
use std::fs;
use std::path::Path;

// ============================================================================
// Theme Configuration
// ============================================================================

/// JSON-deserializable color: supports named colors and RGB hex strings
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ThemeColor {
    Named(String),
}

impl ThemeColor {
    fn to_color(&self) -> Color {
        let ThemeColor::Named(s) = self;
        match s.to_lowercase().as_str() {
            "black" => Color::Black,
            "red" => Color::Red,
            "green" => Color::Green,
            "yellow" => Color::Yellow,
            "blue" => Color::Blue,
            "magenta" => Color::Magenta,
            "cyan" => Color::Cyan,
            "gray" | "grey" => Color::Gray,
            "darkgray" | "darkgrey" | "dark_gray" | "dark_grey" => Color::DarkGray,
            "lightred" | "light_red" => Color::LightRed,
            "lightgreen" | "light_green" => Color::LightGreen,
            "lightyellow" | "light_yellow" => Color::LightYellow,
            "lightblue" | "light_blue" => Color::LightBlue,
            "lightmagenta" | "light_magenta" => Color::LightMagenta,
            "lightcyan" | "light_cyan" => Color::LightCyan,
            "white" => Color::White,
            "reset" | "default" => Color::Reset,
            hex if hex.starts_with('#') && hex.len() == 7 => {
                let r = u8::from_str_radix(&hex[1..3], 16).unwrap_or(255);
                let g = u8::from_str_radix(&hex[3..5], 16).unwrap_or(255);
                let b = u8::from_str_radix(&hex[5..7], 16).unwrap_or(255);
                Color::Rgb(r, g, b)
            }
            _ => Color::Reset,
        }
    }
}

fn default_active_border() -> ThemeColor {
    ThemeColor::Named("yellow".into())
}
fn default_inactive_border() -> ThemeColor {
    ThemeColor::Named("reset".into())
}
fn default_task_completed() -> ThemeColor {
    ThemeColor::Named("dark_gray".into())
}
fn default_task_highlight_bg() -> ThemeColor {
    ThemeColor::Named("dark_gray".into())
}
fn default_task_highlight_fg() -> ThemeColor {
    ThemeColor::Named("white".into())
}
fn default_help_key() -> ThemeColor {
    ThemeColor::Named("cyan".into())
}
fn default_status_success() -> ThemeColor {
    ThemeColor::Named("green".into())
}
fn default_status_error() -> ThemeColor {
    ThemeColor::Named("red".into())
}
fn default_status_info() -> ThemeColor {
    ThemeColor::Named("yellow".into())
}
fn default_popup_bg() -> ThemeColor {
    ThemeColor::Named("black".into())
}
fn default_popup_text() -> ThemeColor {
    ThemeColor::Named("white".into())
}
fn default_popup_label() -> ThemeColor {
    ThemeColor::Named("green".into())
}
fn default_popup_active_input() -> ThemeColor {
    ThemeColor::Named("yellow".into())
}
fn default_popup_inactive_input() -> ThemeColor {
    ThemeColor::Named("white".into())
}
fn default_popup_hint() -> ThemeColor {
    ThemeColor::Named("gray".into())
}
fn default_popup_help() -> ThemeColor {
    ThemeColor::Named("cyan".into())
}
fn default_popup_error() -> ThemeColor {
    ThemeColor::Named("red".into())
}
fn default_project_border() -> ThemeColor {
    ThemeColor::Named("yellow".into())
}
fn default_new_project_border() -> ThemeColor {
    ThemeColor::Named("green".into())
}
fn default_confirm_border() -> ThemeColor {
    ThemeColor::Named("yellow".into())
}
fn default_confirm_prompt() -> ThemeColor {
    ThemeColor::Named("yellow".into())
}
fn default_confirm_yes() -> ThemeColor {
    ThemeColor::Named("green".into())
}
fn default_confirm_no() -> ThemeColor {
    ThemeColor::Named("red".into())
}
fn default_history_border() -> ThemeColor {
    ThemeColor::Named("cyan".into())
}
fn default_history_entry() -> ThemeColor {
    ThemeColor::Named("white".into())
}
fn default_history_scroll_hint() -> ThemeColor {
    ThemeColor::Named("yellow".into())
}
fn default_move_border() -> ThemeColor {
    ThemeColor::Named("magenta".into())
}
fn default_selected_bg() -> ThemeColor {
    ThemeColor::Named("white".into())
}
fn default_selected_fg() -> ThemeColor {
    ThemeColor::Named("black".into())
}
fn default_filter_input() -> ThemeColor {
    ThemeColor::Named("yellow".into())
}
fn default_separator() -> ThemeColor {
    ThemeColor::Named("dark_gray".into())
}
fn default_no_results() -> ThemeColor {
    ThemeColor::Named("dark_gray".into())
}
fn default_field_text() -> ThemeColor {
    ThemeColor::Named("reset".into())
}
fn default_title_text() -> ThemeColor {
    ThemeColor::Named("white".into())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    #[serde(default = "default_active_border")]
    pub active_border: ThemeColor,
    #[serde(default = "default_inactive_border")]
    pub inactive_border: ThemeColor,
    #[serde(default = "default_task_completed")]
    pub task_completed: ThemeColor,
    #[serde(default = "default_task_highlight_bg")]
    pub task_highlight_bg: ThemeColor,
    #[serde(default = "default_task_highlight_fg")]
    pub task_highlight_fg: ThemeColor,
    #[serde(default = "default_help_key")]
    pub help_key: ThemeColor,
    #[serde(default = "default_status_success")]
    pub status_success: ThemeColor,
    #[serde(default = "default_status_error")]
    pub status_error: ThemeColor,
    #[serde(default = "default_status_info")]
    pub status_info: ThemeColor,
    #[serde(default = "default_popup_bg")]
    pub popup_bg: ThemeColor,
    #[serde(default = "default_popup_text")]
    pub popup_text: ThemeColor,
    #[serde(default = "default_popup_label")]
    pub popup_label: ThemeColor,
    #[serde(default = "default_popup_active_input")]
    pub popup_active_input: ThemeColor,
    #[serde(default = "default_popup_inactive_input")]
    pub popup_inactive_input: ThemeColor,
    #[serde(default = "default_popup_hint")]
    pub popup_hint: ThemeColor,
    #[serde(default = "default_popup_help")]
    pub popup_help: ThemeColor,
    #[serde(default = "default_popup_error")]
    pub popup_error: ThemeColor,
    #[serde(default = "default_project_border")]
    pub project_border: ThemeColor,
    #[serde(default = "default_new_project_border")]
    pub new_project_border: ThemeColor,
    #[serde(default = "default_confirm_border")]
    pub confirm_border: ThemeColor,
    #[serde(default = "default_confirm_prompt")]
    pub confirm_prompt: ThemeColor,
    #[serde(default = "default_confirm_yes")]
    pub confirm_yes: ThemeColor,
    #[serde(default = "default_confirm_no")]
    pub confirm_no: ThemeColor,
    #[serde(default = "default_history_border")]
    pub history_border: ThemeColor,
    #[serde(default = "default_history_entry")]
    pub history_entry: ThemeColor,
    #[serde(default = "default_history_scroll_hint")]
    pub history_scroll_hint: ThemeColor,
    #[serde(default = "default_move_border")]
    pub move_border: ThemeColor,
    #[serde(default = "default_selected_bg")]
    pub selected_bg: ThemeColor,
    #[serde(default = "default_selected_fg")]
    pub selected_fg: ThemeColor,
    #[serde(default = "default_filter_input")]
    pub filter_input: ThemeColor,
    #[serde(default = "default_separator")]
    pub separator: ThemeColor,
    #[serde(default = "default_no_results")]
    pub no_results: ThemeColor,
    #[serde(default = "default_field_text")]
    pub field_text: ThemeColor,
    #[serde(default = "default_title_text")]
    pub title_text: ThemeColor,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        ThemeConfig {
            active_border: default_active_border(),
            inactive_border: default_inactive_border(),
            task_completed: default_task_completed(),
            task_highlight_bg: default_task_highlight_bg(),
            task_highlight_fg: default_task_highlight_fg(),
            help_key: default_help_key(),
            status_success: default_status_success(),
            status_error: default_status_error(),
            status_info: default_status_info(),
            popup_bg: default_popup_bg(),
            popup_text: default_popup_text(),
            popup_label: default_popup_label(),
            popup_active_input: default_popup_active_input(),
            popup_inactive_input: default_popup_inactive_input(),
            popup_hint: default_popup_hint(),
            popup_help: default_popup_help(),
            popup_error: default_popup_error(),
            project_border: default_project_border(),
            new_project_border: default_new_project_border(),
            confirm_border: default_confirm_border(),
            confirm_prompt: default_confirm_prompt(),
            confirm_yes: default_confirm_yes(),
            confirm_no: default_confirm_no(),
            history_border: default_history_border(),
            history_entry: default_history_entry(),
            history_scroll_hint: default_history_scroll_hint(),
            move_border: default_move_border(),
            selected_bg: default_selected_bg(),
            selected_fg: default_selected_fg(),
            filter_input: default_filter_input(),
            separator: default_separator(),
            no_results: default_no_results(),
            field_text: default_field_text(),
            title_text: default_title_text(),
        }
    }
}

/// Resolved theme with ratatui Color values ready to use
pub struct Theme {
    pub active_border: Color,
    pub inactive_border: Color,
    pub task_completed: Color,
    pub task_highlight_bg: Color,
    pub task_highlight_fg: Color,
    pub help_key: Color,
    pub status_success: Color,
    pub status_error: Color,
    pub status_info: Color,
    pub popup_bg: Color,
    pub popup_text: Color,
    pub popup_label: Color,
    pub popup_active_input: Color,
    pub popup_inactive_input: Color,
    pub popup_hint: Color,
    pub popup_help: Color,
    pub popup_error: Color,
    pub project_border: Color,
    pub new_project_border: Color,
    pub confirm_border: Color,
    pub confirm_prompt: Color,
    pub confirm_yes: Color,
    pub confirm_no: Color,
    pub history_border: Color,
    pub history_entry: Color,
    pub history_scroll_hint: Color,
    pub move_border: Color,
    pub selected_bg: Color,
    pub selected_fg: Color,
    pub filter_input: Color,
    pub separator: Color,
    pub no_results: Color,
    pub field_text: Color,
    pub title_text: Color,
}

impl Theme {
    pub fn from_config(config: &ThemeConfig) -> Self {
        Theme {
            active_border: config.active_border.to_color(),
            inactive_border: config.inactive_border.to_color(),
            task_completed: config.task_completed.to_color(),
            task_highlight_bg: config.task_highlight_bg.to_color(),
            task_highlight_fg: config.task_highlight_fg.to_color(),
            help_key: config.help_key.to_color(),
            status_success: config.status_success.to_color(),
            status_error: config.status_error.to_color(),
            status_info: config.status_info.to_color(),
            popup_bg: config.popup_bg.to_color(),
            popup_text: config.popup_text.to_color(),
            popup_label: config.popup_label.to_color(),
            popup_active_input: config.popup_active_input.to_color(),
            popup_inactive_input: config.popup_inactive_input.to_color(),
            popup_hint: config.popup_hint.to_color(),
            popup_help: config.popup_help.to_color(),
            popup_error: config.popup_error.to_color(),
            project_border: config.project_border.to_color(),
            new_project_border: config.new_project_border.to_color(),
            confirm_border: config.confirm_border.to_color(),
            confirm_prompt: config.confirm_prompt.to_color(),
            confirm_yes: config.confirm_yes.to_color(),
            confirm_no: config.confirm_no.to_color(),
            history_border: config.history_border.to_color(),
            history_entry: config.history_entry.to_color(),
            history_scroll_hint: config.history_scroll_hint.to_color(),
            move_border: config.move_border.to_color(),
            selected_bg: config.selected_bg.to_color(),
            selected_fg: config.selected_fg.to_color(),
            filter_input: config.filter_input.to_color(),
            separator: config.separator.to_color(),
            no_results: config.no_results.to_color(),
            field_text: config.field_text.to_color(),
            title_text: config.title_text.to_color(),
        }
    }

    pub fn load() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let config_path = Path::new(&home).join(".ted").join("theme.json");

        let config = if config_path.exists() {
            match fs::read_to_string(&config_path) {
                Ok(content) => match serde_json::from_str::<ThemeConfig>(&content) {
                    Ok(cfg) => cfg,
                    Err(e) => {
                        eprintln!("Warning: failed to parse theme.json: {}", e);
                        ThemeConfig::default()
                    }
                },
                Err(e) => {
                    eprintln!("Warning: failed to read theme.json: {}", e);
                    ThemeConfig::default()
                }
            }
        } else {
            ThemeConfig::default()
        };

        Self::from_config(&config)
    }

    // --- Helper style constructors ---

    pub fn active_border_style(&self) -> Style {
        Style::default().fg(self.active_border)
    }

    pub fn inactive_border_style(&self) -> Style {
        Style::default().fg(self.inactive_border)
    }

    pub fn selected_style(&self) -> Style {
        Style::default()
            .bg(self.selected_bg)
            .fg(self.selected_fg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn popup_bg_style(&self) -> Style {
        Style::default().bg(self.popup_bg)
    }
}
