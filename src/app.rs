use crate::filestorage;
use crate::markdown;
use crate::parser;
use crate::theme::Theme;
use crate::ui;
use crate::utils;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use filestorage::{FileStorage, Project};
use markdown::TodoData;
use parser::parse_markdown_file;
use ratatui::Terminal;
use serde::Deserialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use utils::count_display_lines_with_wrapping;

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct JsonInput {
    name: Option<String>,
    project_id: Option<String>,
    info: Option<String>,
    goal: Option<String>,
    tasks: Option<Vec<String>>,
    note: Option<String>,
}

impl JsonInput {
    pub fn print_schema() {
        eprintln!("JSON Schema:");
        eprintln!("{{");
        eprintln!("  \"name\": \"string (optional)\",");
        eprintln!("  \"project_id\": \"string (optional)\",");
        eprintln!("  \"info\": \"string (optional)\",");
        eprintln!("  \"goal\": \"string (optional)\",");
        eprintln!("  \"tasks\": [\"task1\", \"task2\"] (optional array of strings),");
        eprintln!("  \"note\": \"string (optional)\"");
        eprintln!("}}");
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputField {
    Name,
    ProjectId,
    Info,
    Goal,
    Tasks,
    TaskList,
    Note,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NewProjectStep {
    Name,
    Description,
    Shorthand,
}

#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub has_subdirs: bool,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub text: String,
    pub completed: bool,
}

pub struct AppState {
    pub current_field: InputField,
    pub selected_task_index: Option<usize>,
    pub selected_project_index: Option<usize>,
    pub show_project_selector: bool,
    pub project_filter: String,
    pub show_complete_confirmation: bool,
    pub show_history_viewer: bool,
    pub show_new_project_dialog: bool,
    pub show_move_browser: bool,
    pub move_browser_path: PathBuf,
    pub move_browser_entries: Vec<DirEntry>,
    pub move_browser_selected: usize,
    pub move_browser_filter: String,
    pub new_project_step: NewProjectStep,
    pub new_project_name: String,
    pub new_project_description: String,
    pub new_project_shorthand: String,
    pub new_project_error: Option<String>,
    pub history_scroll_offset: usize,
    pub goal_scroll_offset: usize,
    pub note_scroll_offset: usize,
    pub quit: bool,
    pub status_message: Option<String>,
}

impl AppState {
    fn new() -> Self {
        AppState {
            current_field: InputField::Name,
            selected_task_index: None,
            selected_project_index: None,
            show_project_selector: false,
            project_filter: String::new(),
            show_complete_confirmation: false,
            show_history_viewer: false,
            show_new_project_dialog: false,
            show_move_browser: false,
            move_browser_path: PathBuf::new(),
            move_browser_entries: Vec::new(),
            move_browser_selected: 0,
            move_browser_filter: String::new(),
            new_project_step: NewProjectStep::Name,
            new_project_name: String::new(),
            new_project_description: String::new(),
            new_project_shorthand: String::new(),
            new_project_error: None,
            history_scroll_offset: 0,
            goal_scroll_offset: 0,
            note_scroll_offset: 0,
            quit: false,
            status_message: None,
        }
    }
}

pub struct TodoContent {
    pub name: String,
    pub project_id: String,
    pub info: String,
    pub goal: String,
    pub tasks: Vec<Task>,
    pub current_task_input: String,
    pub note: String,
    pub history: String,
    pub saved_filepath: Option<String>,
    pub original_id: Option<String>,
    pub original_created: Option<String>,
    pub original_project_id: Option<String>,
    pub completed_timestamp: Option<String>,
}

impl TodoContent {
    fn new() -> Self {
        TodoContent {
            name: String::new(),
            project_id: String::new(),
            info: String::new(),
            goal: String::new(),
            tasks: Vec::new(),
            current_task_input: String::new(),
            note: String::new(),
            history: String::new(),
            saved_filepath: None,
            original_id: None,
            original_created: None,
            original_project_id: None,
            completed_timestamp: None,
        }
    }

    fn clear(&mut self) {
        self.name.clear();
        self.project_id.clear();
        self.info.clear();
        self.goal.clear();
        self.tasks.clear();
        self.current_task_input.clear();
        self.note.clear();
        self.history.clear();
        self.original_id = None;
        self.original_created = None;
        self.original_project_id = None;
        self.saved_filepath = None;
        self.completed_timestamp = None;
    }
}

pub struct App {
    pub state: AppState,
    pub content: TodoContent,
    pub config: AppConfig,
}

pub struct AppConfig {
    pub output_dir: String,
    pub projects: Vec<Project>,
    pub theme: Theme,
}

impl AppConfig {
    pub fn new() -> Self {
        let file_storage = FileStorage::new();
        let projects = file_storage.get_projects().unwrap_or_default();
        let output_dir = file_storage.get_todos_dir().to_string_lossy().to_string();
        let theme = Theme::load();

        AppConfig {
            output_dir,
            projects,
            theme,
        }
    }

    pub fn into_app(self) -> App {
        App {
            state: AppState::new(),
            content: TodoContent::new(),
            config: self,
        }
    }
}

// ============================================================================
// App Implementation
// ============================================================================

impl App {
    // --- Constructor Methods ---

    pub fn new() -> App {
        AppConfig::new().into_app()
    }

    pub fn from_file(filepath: &str) -> io::Result<App> {
        let path = PathBuf::from(filepath);
        let parsed = parse_markdown_file(&path)?;

        let mut app = AppConfig::new().into_app();

        // Set parsed values
        app.content.name = parsed.name;
        app.content.project_id = parsed.project_id.clone();
        app.content.info = parsed.info;
        app.content.goal = parsed.goal;
        app.content.tasks = parsed
            .tasks
            .into_iter()
            .map(|t| Task {
                text: t.text,
                completed: t.completed,
            })
            .collect();
        app.content.note = parsed.note;
        app.content.history = parsed.history;
        app.content.saved_filepath = Some(filepath.to_string());
        app.content.original_id = Some(parsed.id);
        app.content.original_created = Some(parsed.created);
        app.content.original_project_id = Some(parsed.project_id);
        app.state.status_message = Some(format!("Loaded: {}", filepath));

        Ok(app)
    }

    pub fn from_json(json_str: &str) -> io::Result<App> {
        let json_input: JsonInput = serde_json::from_str(json_str).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidInput, format!("Invalid JSON: {}", e))
        })?;

        let mut app = AppConfig::new().into_app();

        // Set values from JSON
        if let Some(name) = json_input.name {
            app.content.name = name;
        }
        if let Some(project_id) = json_input.project_id {
            app.content.project_id = project_id;
        }
        if let Some(info) = json_input.info {
            app.content.info = info;
        }
        if let Some(goal) = json_input.goal {
            app.content.goal = goal;
        }
        if let Some(tasks) = json_input.tasks {
            app.content.tasks = tasks
                .into_iter()
                .map(|text| Task {
                    text,
                    completed: false,
                })
                .collect();
        }
        if let Some(note) = json_input.note {
            app.content.note = note;
        }

        app.state.status_message = Some("Loaded from JSON".to_string());

        Ok(app)
    }

    // --- File Management Methods ---

    fn calculate_target_filepath_and_id(
        &self,
        project_changed: bool,
        project_shorthand: Option<&String>,
    ) -> (Option<String>, Option<String>) {
        // If project didn't change, keep existing filepath and ID
        if !project_changed {
            return (
                self.content.saved_filepath.clone(),
                self.content.original_id.clone(),
            );
        }

        // Extract the original ID if available
        let orig_id = match &self.content.original_id {
            Some(id) => id,
            None => {
                return (
                    self.content.saved_filepath.clone(),
                    self.content.original_id.clone(),
                );
            }
        };

        // Extract numeric ID from original ID (e.g., "WGR123" -> 123, "T00061" -> 61)
        use regex::Regex;
        let numeric_id = Regex::new(r"^[A-Z]+(\d+)$")
            .ok()
            .and_then(|re| re.captures(orig_id))
            .and_then(|caps| caps.get(1))
            .and_then(|m| m.as_str().parse::<u32>().ok());

        let id_num = match numeric_id {
            Some(num) => num,
            None => {
                return (
                    self.content.saved_filepath.clone(),
                    self.content.original_id.clone(),
                );
            }
        };

        // Generate new ID with new shorthand
        let new_id = match project_shorthand {
            Some(shorthand) => format!("{}{:03}", shorthand, id_num),
            None => format!("T{:05}", id_num),
        };

        // Sanitize name the same way FileStorage does (preserve unicode)
        let sanitized_name: String = self
            .content
            .name
            .chars()
            .map(|c| match c {
                ' ' => '_',
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                c if c.is_control() => '_',
                _ => c,
            })
            .collect::<String>()
            .to_lowercase();

        // Generate new filename with existing ID
        let new_filename = match project_shorthand {
            Some(shorthand) => format!("{}{:03}_{}.md", shorthand, id_num, sanitized_name),
            None => format!("T{:05}_{}.md", id_num, sanitized_name),
        };

        let new_path = PathBuf::from(&self.config.output_dir).join(&new_filename);
        (Some(new_path.to_string_lossy().to_string()), Some(new_id))
    }

    fn save_to_file(&mut self) {
        if self.content.name.is_empty() {
            self.state.status_message = Some("Error: Name is required!".to_string());
            return;
        }

        // Get selected project shorthand
        let project_shorthand = self
            .state
            .selected_project_index
            .and_then(|idx| self.config.projects.get(idx))
            .and_then(|p| p.shorthand.clone())
            .or_else(|| {
                if !self.content.project_id.is_empty() {
                    self.config
                        .projects
                        .iter()
                        .find(|p| p.id == self.content.project_id)
                        .and_then(|p| p.shorthand.clone())
                } else {
                    None
                }
            });

        // Check if project changed during edit
        let project_changed = self.content.saved_filepath.is_some()
            && self.content.original_project_id.as_ref() != Some(&self.content.project_id);

        let old_filepath = self.content.saved_filepath.clone();
        let (target_filepath, updated_id) =
            self.calculate_target_filepath_and_id(project_changed, project_shorthand.as_ref());

        // Add history entry for save
        use chrono::Local;
        let timestamp = Local::now().format("%m-%d-%Y %H:%M:%S").to_string();
        let action = if self.content.saved_filepath.is_some() {
            "Updated"
        } else {
            "Created"
        };
        let history_entry = format!("{}, {}", timestamp, action);
        if !self.content.history.is_empty() {
            self.content.history.push('\n');
        }
        self.content.history.push_str(&history_entry);

        let todo_data = TodoData {
            name: self.content.name.clone(),
            project_id: self.content.project_id.clone(),
            info: self.content.info.clone(),
            project_shorthand,
            goal: self.content.goal.clone(),
            tasks: self
                .content
                .tasks
                .iter()
                .map(|t| {
                    let prefix = if t.completed { "[x] " } else { "[ ] " };
                    format!("{}{}", prefix, t.text)
                })
                .collect(),
            note: self.content.note.clone(),
            history: self.content.history.clone(),
            existing_id: updated_id.clone(),
            existing_created: self.content.original_created.clone(),
            target_filepath: target_filepath.clone(),
            completed: self.content.completed_timestamp.clone(),
        };

        match todo_data.save_to_markdown(&self.config.output_dir) {
            Ok(filepath) => {
                // Delete old file if project changed
                if project_changed {
                    if let Some(ref old_path) = old_filepath {
                        if PathBuf::from(old_path).exists() {
                            let _ = fs::remove_file(old_path);
                        }
                    }
                    if let Some(new_id) = updated_id {
                        self.content.original_id = Some(new_id);
                    }
                    self.content.original_project_id = Some(self.content.project_id.clone());
                }

                let msg = if self.content.saved_filepath.is_some() {
                    if project_changed {
                        format!("✓ Updated and moved: {}", filepath)
                    } else {
                        format!("✓ Updated: {}", filepath)
                    }
                } else {
                    format!("✓ Saved to: {}", filepath)
                };
                self.state.status_message = Some(msg);

                // Update saved filepath
                self.content.saved_filepath = Some(filepath);

                // If creating a new file, clear form
                if old_filepath.is_none() {
                    self.clear_form();
                }
            }
            Err(e) => {
                self.state.status_message = Some(format!("✗ Error saving: {}", e));
            }
        }
    }

    pub fn clear_form(&mut self) {
        self.content.clear();
        self.state.selected_task_index = None;
        self.state.selected_project_index = None;
        self.state.current_field = InputField::Name;
    }

    // --- Project Selector Methods ---

    /// Fuzzy match: checks if all characters of the pattern appear in order in the target (case-insensitive)
    fn fuzzy_match(pattern: &str, target: &str) -> bool {
        if pattern.is_empty() {
            return true;
        }
        let target_lower = target.to_lowercase();
        let pattern_lower = pattern.to_lowercase();
        let mut pattern_chars = pattern_lower.chars();
        let mut current = pattern_chars.next();
        for ch in target_lower.chars() {
            if let Some(p) = current {
                if ch == p {
                    current = pattern_chars.next();
                }
            } else {
                break;
            }
        }
        current.is_none()
    }

    pub fn get_filtered_projects(&self) -> Vec<(usize, &Project)> {
        self.config
            .projects
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                let search_text = format!(
                    "{} {} {}",
                    p.id,
                    p.name,
                    p.shorthand.as_deref().unwrap_or("")
                );
                Self::fuzzy_match(&self.state.project_filter, &search_text)
            })
            .collect()
    }

    pub fn toggle_project_selector(&mut self) {
        self.state.show_project_selector = !self.state.show_project_selector;
        if self.state.show_project_selector {
            self.state.project_filter.clear();
            self.state.selected_project_index = Some(0);
        }
    }

    // --- New Project Dialog Methods ---

    pub fn open_new_project_dialog(&mut self) {
        self.state.show_project_selector = false;
        self.state.show_new_project_dialog = true;
        self.state.new_project_step = NewProjectStep::Name;
        self.state.new_project_name.clear();
        self.state.new_project_description.clear();
        self.state.new_project_shorthand.clear();
        self.state.new_project_error = None;
    }

    pub fn close_new_project_dialog(&mut self) {
        self.state.show_new_project_dialog = false;
        self.state.new_project_error = None;
    }

    pub fn advance_new_project_step(&mut self) {
        match self.state.new_project_step {
            NewProjectStep::Name => {
                if self.state.new_project_name.trim().is_empty() {
                    self.state.new_project_error = Some("Name cannot be empty.".to_string());
                } else {
                    self.state.new_project_error = None;
                    self.state.new_project_step = NewProjectStep::Description;
                }
            }
            NewProjectStep::Description => {
                self.state.new_project_error = None;
                self.state.new_project_step = NewProjectStep::Shorthand;
            }
            NewProjectStep::Shorthand => {
                self.submit_new_project();
            }
        }
    }

    pub fn submit_new_project(&mut self) {
        let shorthand = self.state.new_project_shorthand.trim().to_uppercase();

        if !shorthand.is_empty() && (shorthand.len() < 3 || shorthand.len() > 8) {
            self.state.new_project_error =
                Some("Shorthand must be between 3 and 8 characters (or leave empty).".to_string());
            return;
        }

        let name = self.state.new_project_name.trim().to_string();
        let description = self.state.new_project_description.trim().to_string();
        let file_storage = filestorage::FileStorage::new();

        match file_storage.create_project(&name, &description, &shorthand) {
            Ok(filepath) => {
                // Refresh project list
                self.config.projects = file_storage.get_projects().unwrap_or_default();
                self.state.show_new_project_dialog = false;
                self.state.new_project_error = None;
                self.state.status_message = Some(format!(
                    "✓ Project '{}' created: {}",
                    name,
                    filepath.file_name().unwrap_or_default().to_string_lossy()
                ));
            }
            Err(e) => {
                self.state.new_project_error = Some(format!("Error creating project: {}", e));
            }
        }
    }

    pub fn toggle_history_viewer(&mut self) {
        self.state.show_history_viewer = !self.state.show_history_viewer;
        if !self.state.show_history_viewer {
            self.state.history_scroll_offset = 0;
        }
    }

    // --- Move Browser Methods ---

    fn get_ted_root() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Path::new(&home).join(".ted")
    }

    pub fn open_move_browser(&mut self) {
        if self.content.saved_filepath.is_none() {
            self.state.status_message = Some("Save the file first before moving.".to_string());
            return;
        }
        let ted_root = Self::get_ted_root();
        self.state.move_browser_path = ted_root.clone();
        self.state.move_browser_entries = Self::read_dir_entries(&ted_root);
        self.state.move_browser_filter.clear();
        self.state.move_browser_selected = 0;
        self.state.show_move_browser = true;
    }

    pub fn close_move_browser(&mut self) {
        self.state.show_move_browser = false;
    }

    pub fn get_filtered_move_entries(&self) -> Vec<(usize, &DirEntry)> {
        self.state
            .move_browser_entries
            .iter()
            .enumerate()
            .filter(|(_, e)| Self::fuzzy_match(&self.state.move_browser_filter, &e.name))
            .collect()
    }

    fn read_dir_entries(dir: &Path) -> Vec<DirEntry> {
        let mut entries = Vec::new();
        if let Ok(read_dir) = fs::read_dir(dir) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    // Skip hidden directories like .obsidian, .debris
                    if name.starts_with('.') {
                        continue;
                    }
                    let has_subdirs = fs::read_dir(&path)
                        .map(|rd| {
                            rd.flatten().any(|e| {
                                e.path().is_dir()
                                    && !e.file_name().to_string_lossy().starts_with('.')
                            })
                        })
                        .unwrap_or(false);
                    entries.push(DirEntry {
                        name,
                        has_subdirs,
                        path,
                    });
                }
            }
        }
        entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        entries
    }

    pub fn move_browser_down(&mut self) {
        let count = self.get_filtered_move_entries().len();
        if count > 0 {
            if self.state.move_browser_selected < count - 1 {
                self.state.move_browser_selected += 1;
            } else {
                self.state.move_browser_selected = 0;
            }
        }
    }

    pub fn move_browser_up(&mut self) {
        let count = self.get_filtered_move_entries().len();
        if count > 0 {
            if self.state.move_browser_selected > 0 {
                self.state.move_browser_selected -= 1;
            } else {
                self.state.move_browser_selected = count - 1;
            }
        }
    }

    pub fn move_browser_enter_dir(&mut self) {
        let filtered = self.get_filtered_move_entries();
        if let Some(&(_, entry)) = filtered.get(self.state.move_browser_selected) {
            if entry.has_subdirs {
                let new_path = entry.path.clone();
                self.state.move_browser_entries = Self::read_dir_entries(&new_path);
                self.state.move_browser_path = new_path;
                self.state.move_browser_filter.clear();
                self.state.move_browser_selected = 0;
            }
        }
    }

    pub fn move_browser_go_back(&mut self) {
        let ted_root = Self::get_ted_root();
        if self.state.move_browser_path != ted_root {
            if let Some(parent) = self.state.move_browser_path.parent() {
                let parent_path = parent.to_path_buf();
                self.state.move_browser_entries = Self::read_dir_entries(&parent_path);
                let old_name = self
                    .state
                    .move_browser_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                self.state.move_browser_filter.clear();
                let filtered = self.get_filtered_move_entries();
                self.state.move_browser_selected = filtered
                    .iter()
                    .position(|(_, e)| e.name == old_name)
                    .unwrap_or(0);
                self.state.move_browser_path = parent_path;
            }
        }
    }

    pub fn move_browser_confirm(&mut self) {
        let filtered = self.get_filtered_move_entries();
        let dest_dir = if filtered.is_empty() {
            self.state.move_browser_path.clone()
        } else if let Some(&(_, entry)) = filtered.get(self.state.move_browser_selected) {
            entry.path.clone()
        } else {
            return;
        };

        let source_path_str = match &self.content.saved_filepath {
            Some(p) => p.clone(),
            None => return,
        };

        let source = PathBuf::from(&source_path_str);
        if !source.exists() {
            self.state.status_message = Some("Error: Source file not found.".to_string());
            self.state.show_move_browser = false;
            return;
        }

        let filename = match source.file_name() {
            Some(f) => f.to_string_lossy().to_string(),
            None => {
                self.state.status_message = Some("Error: Invalid filename.".to_string());
                self.state.show_move_browser = false;
                return;
            }
        };

        // Create destination directory if needed
        if let Err(e) = fs::create_dir_all(&dest_dir) {
            self.state.status_message = Some(format!("Error creating directory: {}", e));
            self.state.show_move_browser = false;
            return;
        }

        let dest = dest_dir.join(&filename);

        match fs::rename(&source, &dest) {
            Ok(()) => {
                self.content.saved_filepath = Some(dest.to_string_lossy().to_string());
                let ted_root = Self::get_ted_root();
                let display_path = dest
                    .strip_prefix(&ted_root)
                    .map(|p| format!(".ted/{}", p.display()))
                    .unwrap_or_else(|_| dest.display().to_string());
                self.state.status_message = Some(format!("✓ Moved to: {}", display_path));
            }
            Err(e) => {
                self.state.status_message = Some(format!("✗ Error moving file: {}", e));
            }
        }

        self.state.show_move_browser = false;
    }

    pub fn select_project(&mut self) {
        let filtered = self.get_filtered_projects();
        if let Some(sel) = self.state.selected_project_index {
            if let Some(&(_, project)) = filtered.get(sel) {
                self.content.project_id = project.id.clone();
            }
        }
        self.state.show_project_selector = false;
    }

    pub fn move_project_selection_up(&mut self) {
        let count = self.get_filtered_projects().len();
        if count > 0 {
            self.state.selected_project_index = Some(match self.state.selected_project_index {
                Some(i) if i > 0 => i - 1,
                Some(_) => count - 1,
                None => 0,
            });
        }
    }

    pub fn move_project_selection_down(&mut self) {
        let count = self.get_filtered_projects().len();
        if count > 0 {
            self.state.selected_project_index = Some(match self.state.selected_project_index {
                Some(i) if i < count - 1 => i + 1,
                Some(_) => 0,
                None => 0,
            });
        }
    }

    // --- Task Completion and Done Management ---

    pub fn has_incomplete_tasks(&self) -> bool {
        self.content.tasks.iter().any(|t| !t.completed)
    }

    pub fn mark_all_tasks_complete(&mut self) {
        for task in &mut self.content.tasks {
            task.completed = true;
        }
    }

    pub fn move_to_done(&mut self) -> io::Result<()> {
        if self.content.saved_filepath.is_none() {
            self.state.status_message = Some("Error: No file to move. Save first.".to_string());
            return Ok(());
        }

        let source_path = self.content.saved_filepath.as_ref().unwrap();
        let source = PathBuf::from(source_path);

        if !source.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Source file not found",
            ));
        }

        // Set completed timestamp
        use chrono::Local;
        self.content.completed_timestamp =
            Some(Local::now().format("%m-%d-%Y_%H:%M:%S").to_string());

        // Get filename from source
        let filename = source
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid filename"))?
            .to_string_lossy()
            .to_string();

        // Build destination path in done directory
        let file_storage = FileStorage::new();
        let done_dir = file_storage.get_done_dir();
        let dest = done_dir.join(&filename);

        // Save with completed timestamp to destination
        let project_shorthand = self
            .state
            .selected_project_index
            .and_then(|idx| self.config.projects.get(idx))
            .and_then(|p| p.shorthand.clone())
            .or_else(|| {
                if !self.content.project_id.is_empty() {
                    self.config
                        .projects
                        .iter()
                        .find(|p| p.id == self.content.project_id)
                        .and_then(|p| p.shorthand.clone())
                } else {
                    None
                }
            });

        let todo_data = TodoData {
            name: self.content.name.clone(),
            project_id: self.content.project_id.clone(),
            info: self.content.info.clone(),
            project_shorthand,
            goal: self.content.goal.clone(),
            tasks: self
                .content
                .tasks
                .iter()
                .map(|t| {
                    let prefix = if t.completed { "[x] " } else { "[ ] " };
                    format!("{}{}", prefix, t.text)
                })
                .collect(),
            note: self.content.note.clone(),
            history: self.content.history.clone(),
            existing_id: self.content.original_id.clone(),
            existing_created: self.content.original_created.clone(),
            target_filepath: Some(dest.to_string_lossy().to_string()),
            completed: self.content.completed_timestamp.clone(),
        };

        // Save to done directory
        todo_data.save_to_markdown(&done_dir.to_string_lossy())?;

        // Delete original file from todos
        fs::remove_file(&source)?;

        self.state.status_message = Some(format!("✓ Moved to done: {}", filename));
        self.content.saved_filepath = Some(dest.to_string_lossy().to_string());

        Ok(())
    }

    // --- Field Navigation Methods ---

    pub fn next_field(&mut self) {
        self.state.current_field = match self.state.current_field {
            InputField::Name => InputField::ProjectId,
            InputField::ProjectId => InputField::Info,
            InputField::Info => InputField::Goal,
            InputField::Goal => InputField::Tasks,
            InputField::Tasks => {
                // When leaving task input, clear selection
                self.state.selected_task_index = None;
                InputField::TaskList
            }
            InputField::TaskList => {
                // When leaving task list, clear selection
                self.state.selected_task_index = None;
                InputField::Note
            }
            InputField::Note => InputField::Name,
        };

        // When entering TaskList, select first task if available
        if self.state.current_field == InputField::TaskList && !self.content.tasks.is_empty() {
            self.state.selected_task_index = Some(0);
        }

        // When entering Goal or Note, auto-adjust scroll to show cursor
        if matches!(
            self.state.current_field,
            InputField::Goal | InputField::Note
        ) {
            self.auto_adjust_scroll();
        }
    }

    pub fn previous_field(&mut self) {
        self.state.current_field = match self.state.current_field {
            InputField::Name => InputField::Note,
            InputField::ProjectId => InputField::Name,
            InputField::Info => InputField::ProjectId,
            InputField::Goal => InputField::Info,
            InputField::Tasks => InputField::Goal,
            InputField::TaskList => {
                // When leaving task list, clear selection
                self.state.selected_task_index = None;
                InputField::Tasks
            }
            InputField::Note => {
                // When leaving note, clear selection
                self.state.selected_task_index = None;
                InputField::TaskList
            }
        };

        // When entering TaskList, select first task if available
        if self.state.current_field == InputField::TaskList && !self.content.tasks.is_empty() {
            self.state.selected_task_index = Some(0);
        }

        // When entering Goal or Note, auto-adjust scroll to show cursor
        if matches!(
            self.state.current_field,
            InputField::Goal | InputField::Note
        ) {
            self.auto_adjust_scroll();
        }
    }

    // --- Input Handling Methods ---

    fn get_current_input_mut(&mut self) -> &mut String {
        match self.state.current_field {
            InputField::Name => &mut self.content.name,
            InputField::ProjectId => &mut self.content.project_id,
            InputField::Info => &mut self.content.info,
            InputField::Goal => &mut self.content.goal,
            InputField::Tasks => &mut self.content.current_task_input,
            InputField::TaskList => &mut self.content.current_task_input,
            InputField::Note => &mut self.content.note,
        }
    }

    // --- Task Management Methods ---

    pub fn add_task(&mut self) {
        if !self.content.current_task_input.trim().is_empty() {
            self.content.tasks.push(Task {
                text: self.content.current_task_input.clone(),
                completed: false,
            });
            self.content.current_task_input.clear();
        }
    }

    fn toggle_task_completion(&mut self) {
        if let Some(index) = self.state.selected_task_index {
            if let Some(task) = self.content.tasks.get_mut(index) {
                let was_completed = task.completed;
                task.completed = !task.completed;

                // Add history entry if task was just completed
                if !was_completed && task.completed {
                    use chrono::Local;
                    let timestamp = Local::now().format("%m-%d-%Y %H:%M:%S").to_string();
                    let history_entry = format!("{}, Completed task: {}", timestamp, task.text);
                    if !self.content.history.is_empty() {
                        self.content.history.push('\n');
                    }
                    self.content.history.push_str(&history_entry);
                }
            }
        }
    }

    pub fn delete_selected_task(&mut self) {
        if let Some(index) = self.state.selected_task_index {
            if index < self.content.tasks.len() {
                self.content.tasks.remove(index);
                if self.content.tasks.is_empty() {
                    self.state.selected_task_index = None;
                } else if index >= self.content.tasks.len() {
                    self.state.selected_task_index = Some(self.content.tasks.len() - 1);
                }
            }
        }
    }

    pub fn move_task_selection_up(&mut self) {
        if !self.content.tasks.is_empty() {
            self.state.selected_task_index = Some(match self.state.selected_task_index {
                Some(i) if i > 0 => i - 1,
                Some(_) => self.content.tasks.len() - 1,
                None => 0,
            });
        }
    }

    pub fn move_task_selection_down(&mut self) {
        if !self.content.tasks.is_empty() {
            self.state.selected_task_index = Some(match self.state.selected_task_index {
                Some(i) if i < self.content.tasks.len() - 1 => i + 1,
                Some(_) => 0,
                None => 0,
            });
        }
    }

    // --- Scroll Management Methods ---

    pub fn auto_adjust_scroll(&mut self) {
        // Estimate available width (typical terminal minus margins and borders)
        // This is conservative; actual width may vary
        let estimated_width = 100; // Reasonable estimate for wrapping calculations

        match self.state.current_field {
            InputField::Goal => {
                let display_line_count =
                    count_display_lines_with_wrapping(&self.content.goal, estimated_width);
                let max_visible = 3;

                // If cursor (last line) is below visible area, scroll down
                if display_line_count > self.state.goal_scroll_offset + max_visible {
                    self.state.goal_scroll_offset = display_line_count.saturating_sub(max_visible);
                }
            }
            InputField::Note => {
                let display_line_count =
                    count_display_lines_with_wrapping(&self.content.note, estimated_width);
                let max_visible = 6;

                // If cursor (last line) is below visible area, scroll down
                if display_line_count > self.state.note_scroll_offset + max_visible {
                    self.state.note_scroll_offset = display_line_count.saturating_sub(max_visible);
                }
            }
            _ => {}
        }
    }
}

// ============================================================================
// Event Handling
// ============================================================================

pub fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
) -> io::Result<()> {
    loop {
        terminal
            .draw(|f| ui::ui(f, &app))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{}", e)))?;

        if let Event::Key(key) = event::read()? {
            app.state.status_message = None; // Clear status message on any key press

            if app.state.show_complete_confirmation {
                handle_confirmation_dialog(&mut app, key.code);
                continue;
            }

            if app.state.show_new_project_dialog {
                handle_new_project_dialog(&mut app, key);
                continue;
            }

            if app.state.show_project_selector {
                handle_project_selector(&mut app, key);
                continue;
            }

            if app.state.show_history_viewer {
                // Check for Ctrl+H to toggle history viewer off
                if key.code == KeyCode::Char('h') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    app.toggle_history_viewer();
                } else {
                    handle_history_viewer(&mut app, key.code);
                }
                continue;
            }

            if app.state.show_move_browser {
                handle_move_browser(&mut app, key);
                continue;
            }

            handle_main_input(&mut app, key);
        }

        if app.state.quit {
            break;
        }
    }

    Ok(())
}

fn handle_confirmation_dialog(app: &mut App, key_code: KeyCode) {
    match key_code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            app.mark_all_tasks_complete();
            app.state.show_complete_confirmation = false;
            if let Err(e) = app.move_to_done() {
                app.state.status_message = Some(format!("Error moving to done: {}", e));
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.state.show_complete_confirmation = false;
            app.state.status_message = Some("Move cancelled.".to_string());
        }
        _ => {}
    }
}

fn handle_project_selector(app: &mut App, key: event::KeyEvent) {
    match key.code {
        KeyCode::Esc => app.state.show_project_selector = false,
        KeyCode::Up => app.move_project_selection_up(),
        KeyCode::Down => app.move_project_selection_down(),
        KeyCode::Enter => app.select_project(),
        KeyCode::Char('n') | KeyCode::Char('N')
            if key.modifiers.contains(KeyModifiers::CONTROL) =>
        {
            app.open_new_project_dialog();
        }
        KeyCode::Backspace => {
            app.state.project_filter.pop();
            app.state.selected_project_index = Some(0);
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.state.project_filter.push(c);
            app.state.selected_project_index = Some(0);
        }
        _ => {}
    }
}

fn handle_new_project_dialog(app: &mut App, key: event::KeyEvent) {
    match key.code {
        KeyCode::Esc => app.close_new_project_dialog(),
        KeyCode::Enter | KeyCode::Tab => app.advance_new_project_step(),
        KeyCode::BackTab => {
            app.state.new_project_error = None;
            app.state.new_project_step = match app.state.new_project_step {
                NewProjectStep::Name => NewProjectStep::Name,
                NewProjectStep::Description => NewProjectStep::Name,
                NewProjectStep::Shorthand => NewProjectStep::Description,
            };
        }
        KeyCode::Backspace => {
            let field = app.state.new_project_step.clone();
            match field {
                NewProjectStep::Name => {
                    app.state.new_project_name.pop();
                }
                NewProjectStep::Description => {
                    app.state.new_project_description.pop();
                }
                NewProjectStep::Shorthand => {
                    app.state.new_project_shorthand.pop();
                }
            }
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            let field = app.state.new_project_step.clone();
            match field {
                NewProjectStep::Name => app.state.new_project_name.push(c),
                NewProjectStep::Description => app.state.new_project_description.push(c),
                NewProjectStep::Shorthand => app.state.new_project_shorthand.push(c),
            }
        }
        _ => {}
    }
}

fn handle_history_viewer(app: &mut App, key_code: KeyCode) {
    let history_entries: Vec<&str> = app
        .content
        .history
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let max_scroll = history_entries.len().saturating_sub(1);

    match key_code {
        KeyCode::Esc => app.state.show_history_viewer = false,
        KeyCode::Up => {
            if app.state.history_scroll_offset > 0 {
                app.state.history_scroll_offset -= 1;
            }
        }
        KeyCode::Down => {
            if app.state.history_scroll_offset < max_scroll {
                app.state.history_scroll_offset += 1;
            }
        }
        _ => {}
    }
}

fn handle_move_browser(app: &mut App, key: event::KeyEvent) {
    match key.code {
        KeyCode::Esc => app.close_move_browser(),
        KeyCode::Up => app.move_browser_up(),
        KeyCode::Down => app.move_browser_down(),
        KeyCode::Right => app.move_browser_enter_dir(),
        KeyCode::Left => app.move_browser_go_back(),
        KeyCode::Enter => app.move_browser_confirm(),
        KeyCode::Backspace => {
            app.state.move_browser_filter.pop();
            app.state.move_browser_selected = 0;
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.state.move_browser_filter.push(c);
            app.state.move_browser_selected = 0;
        }
        _ => {}
    }
}

fn handle_main_input(app: &mut App, key: event::KeyEvent) {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.state.quit = true;
        }
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.save_to_file();
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_move_to_done(app);
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if matches!(
                app.state.current_field,
                InputField::ProjectId | InputField::Info
            ) {
                app.toggle_project_selector();
            }
        }
        KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.toggle_history_viewer();
        }
        KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.open_move_browser();
        }
        KeyCode::Esc => app.state.quit = true,
        KeyCode::Tab => app.next_field(),
        KeyCode::BackTab => app.previous_field(),
        KeyCode::Enter => {
            if app.state.current_field == InputField::Tasks {
                app.add_task();
            } else if matches!(app.state.current_field, InputField::Goal | InputField::Note) {
                app.get_current_input_mut().push('\n');
                app.auto_adjust_scroll();
            }
        }
        KeyCode::Backspace => {
            if app.state.current_field != InputField::TaskList {
                app.get_current_input_mut().pop();
            }
        }
        KeyCode::Char(c) => {
            if c == ' ' && app.state.current_field == InputField::TaskList {
                app.toggle_task_completion();
            } else if app.state.current_field != InputField::TaskList {
                app.get_current_input_mut().push(c);
                // Auto-adjust scroll when typing in multiline fields
                if matches!(app.state.current_field, InputField::Goal | InputField::Note) {
                    app.auto_adjust_scroll();
                }
            }
        }
        KeyCode::Up => {
            if app.state.current_field == InputField::TaskList {
                app.move_task_selection_up();
            } else if app.state.current_field == InputField::Goal {
                if app.state.goal_scroll_offset > 0 {
                    app.state.goal_scroll_offset -= 1;
                }
            } else if app.state.current_field == InputField::Note {
                if app.state.note_scroll_offset > 0 {
                    app.state.note_scroll_offset -= 1;
                }
            }
        }
        KeyCode::Down => {
            if app.state.current_field == InputField::TaskList {
                app.move_task_selection_down();
            } else if app.state.current_field == InputField::Goal {
                let estimated_width = 100;
                let display_line_count =
                    count_display_lines_with_wrapping(&app.content.goal, estimated_width);
                let max_scroll = display_line_count.saturating_sub(3); // 3 visible lines in Goal
                if app.state.goal_scroll_offset < max_scroll {
                    app.state.goal_scroll_offset += 1;
                }
            } else if app.state.current_field == InputField::Note {
                let estimated_width = 100;
                let display_line_count =
                    count_display_lines_with_wrapping(&app.content.note, estimated_width);
                let max_scroll = display_line_count.saturating_sub(6); // 6 visible lines in Note
                if app.state.note_scroll_offset < max_scroll {
                    app.state.note_scroll_offset += 1;
                }
            }
        }
        KeyCode::Delete if app.state.current_field == InputField::TaskList => {
            app.delete_selected_task();
        }
        _ => {}
    }
}

fn handle_move_to_done(app: &mut App) {
    if app.content.saved_filepath.is_some() {
        if app.has_incomplete_tasks() {
            app.state.show_complete_confirmation = true;
        } else if let Err(e) = app.move_to_done() {
            app.state.status_message = Some(format!("Error moving to done: {}", e));
        }
    } else {
        app.state.status_message = Some("Save the file first before moving to done.".to_string());
    }
}
