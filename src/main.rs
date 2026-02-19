// ============================================================================
// Module Imports and Dependencies
// ============================================================================

mod filestorage;
mod markdown;
mod parser;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use filestorage::{FileStorage, Project};
use markdown::TodoData;
use parser::parse_markdown_file;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use serde::Deserialize;
use std::fs;
use std::io;
use std::path::PathBuf;
use unicode_width::UnicodeWidthStr;

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Deserialize)]
struct JsonInput {
    name: Option<String>,
    project_id: Option<String>,
    info: Option<String>,
    goal: Option<String>,
    tasks: Option<Vec<String>>,
    note: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum InputField {
    Name,
    ProjectId,
    Info,
    Goal,
    Tasks,
    TaskList,
    Note,
}

#[derive(Debug, Clone)]
struct Task {
    text: String,
    completed: bool,
}

struct AppState {
    current_field: InputField,
    selected_task_index: Option<usize>,
    selected_project_index: Option<usize>,
    show_project_selector: bool,
    show_complete_confirmation: bool,
    show_history_viewer: bool,
    history_scroll_offset: usize,
    quit: bool,
    status_message: Option<String>,
}

impl AppState {
    fn new() -> Self {
        AppState {
            current_field: InputField::Name,
            selected_task_index: None,
            selected_project_index: None,
            show_project_selector: false,
            show_complete_confirmation: false,
            show_history_viewer: false,
            history_scroll_offset: 0,
            quit: false,
            status_message: None,
        }
    }
}

struct TodoContent {
    name: String,
    project_id: String,
    info: String,
    goal: String,
    tasks: Vec<Task>,
    current_task_input: String,
    note: String,
    history: String,
    saved_filepath: Option<String>,
    original_id: Option<String>,
    original_created: Option<String>,
    original_project_id: Option<String>,
    completed_timestamp: Option<String>,
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

struct App {
    state: AppState,
    content: TodoContent,
    config: AppConfig,
}

struct AppConfig {
    output_dir: String,
    projects: Vec<Project>,
}

impl AppConfig {
    fn new() -> Self {
        let file_storage = FileStorage::new();
        let projects = file_storage.get_projects().unwrap_or_default();
        let output_dir = file_storage.get_todos_dir().to_string_lossy().to_string();

        AppConfig {
            output_dir,
            projects,
        }
    }

    fn into_app(self) -> App {
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

    fn new() -> App {
        AppConfig::new().into_app()
    }

    fn from_file(filepath: &str) -> io::Result<App> {
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

    fn from_json(json_str: &str) -> io::Result<App> {
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

    fn clear_form(&mut self) {
        self.content.clear();
        self.state.selected_task_index = None;
        self.state.selected_project_index = None;
        self.state.current_field = InputField::Name;
    }

    // --- Project Selector Methods ---

    fn toggle_project_selector(&mut self) {
        self.state.show_project_selector = !self.state.show_project_selector;
        if self.state.show_project_selector
            && self.state.selected_project_index.is_none()
            && !self.config.projects.is_empty()
        {
            self.state.selected_project_index = Some(0);
        }
    }

    fn toggle_history_viewer(&mut self) {
        self.state.show_history_viewer = !self.state.show_history_viewer;
        if !self.state.show_history_viewer {
            self.state.history_scroll_offset = 0;
        }
    }

    fn select_project(&mut self) {
        if let Some(idx) = self.state.selected_project_index {
            if let Some(project) = self.config.projects.get(idx) {
                self.content.project_id = project.id.clone();
            }
        }
        self.state.show_project_selector = false;
    }

    fn move_project_selection_up(&mut self) {
        if !self.config.projects.is_empty() {
            self.state.selected_project_index = Some(match self.state.selected_project_index {
                Some(i) if i > 0 => i - 1,
                Some(_) => self.config.projects.len() - 1,
                None => 0,
            });
        }
    }

    fn move_project_selection_down(&mut self) {
        if !self.config.projects.is_empty() {
            self.state.selected_project_index = Some(match self.state.selected_project_index {
                Some(i) if i < self.config.projects.len() - 1 => i + 1,
                Some(_) => 0,
                None => 0,
            });
        }
    }

    // --- Task Completion and Done Management ---

    fn has_incomplete_tasks(&self) -> bool {
        self.content.tasks.iter().any(|t| !t.completed)
    }

    fn mark_all_tasks_complete(&mut self) {
        for task in &mut self.content.tasks {
            task.completed = true;
        }
    }

    fn move_to_done(&mut self) -> io::Result<()> {
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

    fn next_field(&mut self) {
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
    }

    fn previous_field(&mut self) {
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

    fn add_task(&mut self) {
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

    fn delete_selected_task(&mut self) {
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

    fn move_task_selection_up(&mut self) {
        if !self.content.tasks.is_empty() {
            self.state.selected_task_index = Some(match self.state.selected_task_index {
                Some(i) if i > 0 => i - 1,
                Some(_) => self.content.tasks.len() - 1,
                None => 0,
            });
        }
    }

    fn move_task_selection_down(&mut self) {
        if !self.content.tasks.is_empty() {
            self.state.selected_task_index = Some(match self.state.selected_task_index {
                Some(i) if i < self.content.tasks.len() - 1 => i + 1,
                Some(_) => 0,
                None => 0,
            });
        }
    }
}

// ============================================================================
// Utility Helper Functions
// ============================================================================

fn find_file_by_id(id_num: u32) -> Option<PathBuf> {
    let file_storage = FileStorage::new();
    let todos_dir = file_storage.get_todos_dir();

    // Try to find file in todos directory
    if let Ok(entries) = fs::read_dir(&todos_dir) {
        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();
            // Match patterns like T00123_ or ADM123_ or any PREFIX123_
            if let Some(extracted_id) = extract_id_from_filename(&filename) {
                if extracted_id == id_num {
                    return Some(entry.path());
                }
            }
        }
    }

    // Also check done directory
    let done_dir = file_storage.get_todos_dir().parent()?.join("done");
    if let Ok(entries) = fs::read_dir(&done_dir) {
        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();
            if let Some(extracted_id) = extract_id_from_filename(&filename) {
                if extracted_id == id_num {
                    return Some(entry.path());
                }
            }
        }
    }

    None
}

fn extract_id_from_filename(filename: &str) -> Option<u32> {
    use regex::Regex;
    let re = Regex::new(r"^[A-Z]+(\d+)_").ok()?;
    re.captures(filename)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse::<u32>().ok())
}

// ============================================================================
// Main Entry Point
// ============================================================================

fn main() -> Result<(), io::Error> {
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();

    let app = if args.len() > 1 {
        let arg = &args[1];

        // Check for --json flag
        if arg == "--json" {
            if args.len() < 3 {
                eprintln!("Usage: tedtui --json '<json_string>'");
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Missing JSON string",
                ));
            }
            let json_str = &args[2];
            match App::from_json(json_str) {
                Ok(app) => app,
                Err(e) => {
                    eprintln!("Error parsing JSON: {}", e);
                    return Err(e);
                }
            }
        }
        // Check if it's a file path
        else if arg.ends_with(".md") {
            match App::from_file(arg) {
                Ok(app) => app,
                Err(e) => {
                    eprintln!("Error loading file: {}", e);
                    return Err(e);
                }
            }
        }
        // Check if it's a number (ID)
        else if let Ok(id_num) = arg.parse::<u32>() {
            if let Some(filepath) = find_file_by_id(id_num) {
                match App::from_file(&filepath.to_string_lossy()) {
                    Ok(app) => app,
                    Err(e) => {
                        eprintln!("Error loading file: {}", e);
                        return Err(e);
                    }
                }
            } else {
                eprintln!("No file found with ID: {}", id_num);
                return Err(io::Error::new(io::ErrorKind::NotFound, "File not found"));
            }
        }
        // Invalid argument
        else {
            eprintln!("Usage: tedtui [--json '<json_string>'|file.md|ID]");
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid argument",
            ));
        }
    } else {
        App::new()
    };

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the app
    let res = run_app(&mut terminal, app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {:?}", err);
    }

    Ok(())
}

// ============================================================================
// Event Handling
// ============================================================================

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
) -> io::Result<()> {
    loop {
        terminal
            .draw(|f| ui(f, &app))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{}", e)))?;

        if let Event::Key(key) = event::read()? {
            app.state.status_message = None; // Clear status message on any key press

            if app.state.show_complete_confirmation {
                handle_confirmation_dialog(&mut app, key.code);
                continue;
            }

            if app.state.show_project_selector {
                handle_project_selector(&mut app, key.code);
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

fn handle_project_selector(app: &mut App, key_code: KeyCode) {
    match key_code {
        KeyCode::Esc => app.state.show_project_selector = false,
        KeyCode::Up => app.move_project_selection_up(),
        KeyCode::Down => app.move_project_selection_down(),
        KeyCode::Enter => app.select_project(),
        _ => {}
    }
}

fn handle_history_viewer(app: &mut App, key_code: KeyCode) {
    let history_entries: Vec<&str> = app.content.history.lines().filter(|line| !line.trim().is_empty()).collect();
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
        KeyCode::Esc => app.state.quit = true,
        KeyCode::Tab => app.next_field(),
        KeyCode::BackTab => app.previous_field(),
        KeyCode::Enter => {
            if app.state.current_field == InputField::Tasks {
                app.add_task();
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
            }
        }
        KeyCode::Up if app.state.current_field == InputField::TaskList => {
            app.move_task_selection_up();
        }
        KeyCode::Down if app.state.current_field == InputField::TaskList => {
            app.move_task_selection_down();
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

// ============================================================================
// UI Rendering
// ============================================================================

enum WidgetItem<'a> {
    Paragraph(Paragraph<'a>, ratatui::layout::Rect),
    List(List<'a>, ratatui::layout::Rect),
    StatefulList(List<'a>, ListState, ratatui::layout::Rect),
    Clear(ratatui::layout::Rect),
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(4), // Name
            Constraint::Length(4), // Project ID and Info
            Constraint::Length(5), // Goal
            Constraint::Min(10),   // Tasks
            Constraint::Length(8), // Note
            Constraint::Length(3), // Help
            Constraint::Length(2), // Status
        ])
        .split(f.area());

    // Collect all widgets
    let mut widgets: Vec<WidgetItem> = Vec::new();

    // Create and collect main widgets
    let name_field = create_name_field(app, chunks[0]);
    widgets.push(WidgetItem::Paragraph(name_field, chunks[0]));

    let (project_field, info_field, project_info_chunks) =
        create_project_info_fields(app, chunks[1]);
    widgets.push(WidgetItem::Paragraph(project_field, project_info_chunks[0]));
    widgets.push(WidgetItem::Paragraph(info_field, project_info_chunks[1]));

    let goal_field = create_goal_field(app);
    widgets.push(WidgetItem::Paragraph(goal_field, chunks[2]));

    let (task_input_field, tasks_list, tasks_list_state, tasks_chunks) =
        create_tasks_section(app, chunks[3]);
    widgets.push(WidgetItem::Paragraph(task_input_field, tasks_chunks[0]));
    widgets.push(WidgetItem::StatefulList(
        tasks_list,
        tasks_list_state,
        tasks_chunks[1],
    ));

    let note_field = create_note_field(app);
    widgets.push(WidgetItem::Paragraph(note_field, chunks[4]));

    let help = create_help();
    widgets.push(WidgetItem::Paragraph(help, chunks[5]));

    if let Some(status_message) = create_status_message(app) {
        widgets.push(WidgetItem::Paragraph(status_message, chunks[6]));
    }

    // Render overlays (these are rendered immediately due to borrowing constraints)
    if app.state.show_project_selector {
        let (project_selector, popup_area) = create_project_selector_overlay(app, f.area());
        widgets.push(WidgetItem::Clear(popup_area));
        widgets.push(WidgetItem::List(project_selector, popup_area));
    }
    if app.state.show_complete_confirmation {
        let (confirmation, popup_area) = create_completion_confirmation_overlay(app, f.area());
        widgets.push(WidgetItem::Clear(popup_area));
        widgets.push(WidgetItem::Paragraph(confirmation, popup_area));
    }
    if app.state.show_history_viewer {
        let (history_viewer, popup_area) = create_history_viewer_overlay(app, f.area());
        widgets.push(WidgetItem::Clear(popup_area));
        widgets.push(WidgetItem::Paragraph(history_viewer, popup_area));
    }

    // Render all widgets
    for widget in widgets {
        match widget {
            WidgetItem::Paragraph(w, area) => f.render_widget(w, area),
            WidgetItem::List(w, area) => f.render_widget(w, area),
            WidgetItem::StatefulList(w, mut state, area) => {
                f.render_stateful_widget(w, area, &mut state)
            }
            WidgetItem::Clear(area) => f.render_widget(Clear, area),
        }
    }

    render_cursor(f, app, &chunks, &project_info_chunks, &tasks_chunks);
}

fn create_input_block<'a>(title: &'a str, is_active: bool) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(if is_active {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        })
}

fn truncate_text_for_display(text: &str, max_width: usize) -> &str {
    if text.len() > max_width {
        &text[text.len().saturating_sub(max_width)..]
    } else {
        text
    }
}

fn create_name_field<'a>(app: &'a App, area: ratatui::layout::Rect) -> Paragraph<'a> {
    let is_active = app.state.current_field == InputField::Name;
    let name_block = create_input_block("Name", is_active);
    let name_text = truncate_text_for_display(&app.content.name, area.width as usize - 4);
    Paragraph::new(name_text).block(name_block)
}

fn create_project_info_fields<'a>(
    app: &'a App,
    area: ratatui::layout::Rect,
) -> (Paragraph<'a>, Paragraph<'a>, Vec<ratatui::layout::Rect>) {
    let project_info_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Project ID
    let project_title = if app.state.current_field == InputField::ProjectId {
        "Project ID (Ctrl+P to select)"
    } else {
        "Project ID"
    };
    let is_active = app.state.current_field == InputField::ProjectId;
    let project_block = create_input_block(project_title, is_active);
    let project_text = truncate_text_for_display(
        &app.content.project_id,
        project_info_chunks[0].width as usize - 4,
    );
    let project_paragraph = Paragraph::new(project_text).block(project_block);

    // Info
    let is_active = app.state.current_field == InputField::Info;
    let info_block = create_input_block("Info", is_active);
    let info_text =
        truncate_text_for_display(&app.content.info, project_info_chunks[1].width as usize - 4);
    let info_paragraph = Paragraph::new(info_text).block(info_block);

    (
        project_paragraph,
        info_paragraph,
        project_info_chunks.to_vec(),
    )
}

fn create_goal_field<'a>(app: &'a App) -> Paragraph<'a> {
    let is_active = app.state.current_field == InputField::Goal;
    let goal_block = create_input_block("Goal / Short Description", is_active);
    Paragraph::new(app.content.goal.as_str())
        .block(goal_block)
        .wrap(Wrap { trim: false })
}

fn create_tasks_section<'a>(
    app: &'a App,
    area: ratatui::layout::Rect,
) -> (
    Paragraph<'a>,
    List<'a>,
    ListState,
    Vec<ratatui::layout::Rect>,
) {
    let tasks_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(area);

    // Task input
    let is_active = app.state.current_field == InputField::Tasks;
    let task_input_block = create_input_block("Add Task (Enter to add)", is_active);
    let task_text = truncate_text_for_display(
        &app.content.current_task_input,
        tasks_chunks[0].width as usize - 4,
    );
    let task_input_paragraph = Paragraph::new(task_text).block(task_input_block);

    // Task list
    let task_items: Vec<ListItem> = app
        .content
        .tasks
        .iter()
        .map(|task| {
            let checkbox = if task.completed { "[x]" } else { "[ ]" };
            let text = format!("  - {} {}", checkbox, task.text);
            let style = if task.completed {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            };
            ListItem::new(text).style(style)
        })
        .collect();

    let is_active = app.state.current_field == InputField::TaskList;
    let tasks_list = List::new(task_items)
        .block(create_input_block(
            "Task List (Tab to enter, ↑↓ select, Space toggle, Del delete)",
            is_active,
        ))
        .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White));

    let mut list_state = ListState::default();
    list_state.select(app.state.selected_task_index);

    (
        task_input_paragraph,
        tasks_list,
        list_state,
        tasks_chunks.to_vec(),
    )
}

fn create_note_field<'a>(app: &'a App) -> Paragraph<'a> {
    let is_active = app.state.current_field == InputField::Note;
    let note_block = create_input_block("Note", is_active);
    Paragraph::new(app.content.note.as_str())
        .block(note_block)
        .wrap(Wrap { trim: false })
}

fn create_help() -> Paragraph<'static> {
    let help_text = Line::from(vec![
        Span::styled("Tab", Style::default().fg(Color::Cyan)),
        Span::raw(" / "),
        Span::styled("Shift+Tab", Style::default().fg(Color::Cyan)),
        Span::raw(" - Navigate | "),
        Span::styled("Space", Style::default().fg(Color::Cyan)),
        Span::raw(" - Toggle | "),
        Span::styled("Ctrl+P", Style::default().fg(Color::Cyan)),
        Span::raw(" - Projects | "),
        Span::styled("Ctrl+H", Style::default().fg(Color::Cyan)),
        Span::raw(" - History | "),
        Span::styled("Ctrl+S", Style::default().fg(Color::Cyan)),
        Span::raw(" - Save | "),
        Span::styled("Ctrl+D", Style::default().fg(Color::Cyan)),
        Span::raw(" - Done | "),
        Span::styled("Esc", Style::default().fg(Color::Cyan)),
        Span::raw(" - Quit"),
    ]);
    Paragraph::new(help_text).block(Block::default().borders(Borders::ALL).title("Help"))
}

fn create_status_message<'a>(app: &'a App) -> Option<Paragraph<'a>> {
    app.state.status_message.as_ref().map(|msg| {
        let status_color = if msg.contains("✓") {
            Color::Green
        } else if msg.contains("✗") {
            Color::Red
        } else {
            Color::Yellow
        };
        Paragraph::new(msg.as_str())
            .style(Style::default().fg(status_color))
            .wrap(Wrap { trim: false })
    })
}

fn create_project_selector_overlay<'a>(
    app: &'a App,
    frame_area: ratatui::layout::Rect,
) -> (List<'a>, ratatui::layout::Rect) {
    let popup_width = frame_area.width.saturating_sub(20).max(40);
    let popup_height = frame_area.height.saturating_sub(10).max(15).min(30);
    let popup_x = (frame_area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (frame_area.height.saturating_sub(popup_height)) / 2;

    let popup_area = ratatui::layout::Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width,
        height: popup_height,
    };

    let project_items: Vec<ListItem> = app
        .config
        .projects
        .iter()
        .enumerate()
        .map(|(i, project)| {
            let style = if Some(i) == app.state.selected_project_index {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default()
            };
            let text = if let Some(ref shorthand) = project.shorthand {
                format!("  {} - {} ({})", project.id, project.name, shorthand)
            } else {
                format!("  {} - {}", project.id, project.name)
            };
            ListItem::new(text).style(style)
        })
        .collect();

    let projects_list = List::new(project_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Select Project (↑↓ to navigate, Enter to select, Esc to cancel)")
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .style(Style::default().bg(Color::Black));

    (projects_list, popup_area)
}

fn create_completion_confirmation_overlay<'a>(
    app: &'a App,
    frame_area: ratatui::layout::Rect,
) -> (Paragraph<'a>, ratatui::layout::Rect) {
    let popup_width = 60;
    let popup_height = 7;
    let popup_x = (frame_area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (frame_area.height.saturating_sub(popup_height)) / 2;

    let popup_area = ratatui::layout::Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width,
        height: popup_height,
    };

    let incomplete_count = app.content.tasks.iter().filter(|t| !t.completed).count();
    let message = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("You have {} incomplete task(s).", incomplete_count),
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Mark all tasks as complete before moving to done?",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::White)),
            Span::styled("Y", Style::default().fg(Color::Green)),
            Span::styled(
                " to mark complete and move, ",
                Style::default().fg(Color::White),
            ),
            Span::styled("N", Style::default().fg(Color::Red)),
            Span::styled(" to cancel", Style::default().fg(Color::White)),
        ]),
    ];

    let confirmation = Paragraph::new(message)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Confirm Move to Done")
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .style(Style::default().bg(Color::Black))
        .wrap(Wrap { trim: false });

    (confirmation, popup_area)
}

fn create_history_viewer_overlay<'a>(
    app: &'a App,
    frame_area: ratatui::layout::Rect,
) -> (Paragraph<'a>, ratatui::layout::Rect) {
    let popup_width = frame_area.width.saturating_sub(20).max(60);
    let popup_height = frame_area.height.saturating_sub(10).max(15);
    let popup_x = (frame_area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (frame_area.height.saturating_sub(popup_height)) / 2;

    let popup_area = ratatui::layout::Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width,
        height: popup_height,
    };

    let history_entries: Vec<&str> = app
        .content
        .history
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();

    let visible_lines = (popup_height as usize).saturating_sub(3); // Account for borders and title
    let start_idx = app.state.history_scroll_offset;
    let end_idx = (start_idx + visible_lines).min(history_entries.len());

    let mut lines: Vec<Line> = vec![];
    
    if history_entries.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "No history entries yet.",
            Style::default().fg(Color::Gray),
        )));
    } else {
        for entry in history_entries.iter().skip(start_idx).take(visible_lines) {
            lines.push(Line::from(Span::styled(
                *entry,
                Style::default().fg(Color::White),
            )));
        }
        
        // Show scroll indicator if there's more content
        if end_idx < history_entries.len() {
            lines.push(Line::from(Span::styled(
                format!("... {} more entries (↓ to scroll)", history_entries.len() - end_idx),
                Style::default().fg(Color::Yellow),
            )));
        }
    }

    let title = if history_entries.is_empty() {
        "History (Esc to close)".to_string()
    } else {
        format!(
            "History ({}/{}) (↑↓ to scroll, Esc to close)",
            start_idx + 1,
            history_entries.len()
        )
    };

    let history_view = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .style(Style::default().bg(Color::Black))
        .wrap(Wrap { trim: false });

    (history_view, popup_area)
}

fn calculate_cursor_x(text: &str, area_x: u16, area_width: u16) -> u16 {
    let text_width = text.width();
    if text_width > (area_width - 3) as usize {
        area_x + area_width - 2
    } else {
        area_x + text_width as u16 + 1
    }
}

fn render_cursor(
    f: &mut Frame,
    app: &App,
    chunks: &[ratatui::layout::Rect],
    project_info_chunks: &[ratatui::layout::Rect],
    tasks_chunks: &[ratatui::layout::Rect],
) {
    match app.state.current_field {
        InputField::Name => {
            let cursor_x = calculate_cursor_x(&app.content.name, chunks[0].x, chunks[0].width);
            f.set_cursor_position((cursor_x, chunks[0].y + 1));
        }
        InputField::ProjectId => {
            let cursor_x = calculate_cursor_x(
                &app.content.project_id,
                project_info_chunks[0].x,
                project_info_chunks[0].width,
            );
            f.set_cursor_position((cursor_x, project_info_chunks[0].y + 1));
        }
        InputField::Info => {
            let cursor_x = calculate_cursor_x(
                &app.content.info,
                project_info_chunks[1].x,
                project_info_chunks[1].width,
            );
            f.set_cursor_position((cursor_x, project_info_chunks[1].y + 1));
        }
        InputField::Goal => {
            let cursor_x = calculate_cursor_x(&app.content.goal, chunks[2].x, chunks[2].width);
            f.set_cursor_position((cursor_x, chunks[2].y + 1));
        }
        InputField::Tasks => {
            let cursor_x = calculate_cursor_x(
                &app.content.current_task_input,
                tasks_chunks[0].x,
                tasks_chunks[0].width,
            );
            f.set_cursor_position((cursor_x, tasks_chunks[0].y + 1));
        }
        InputField::TaskList => {
            // Hide cursor in task list mode (selection shown with highlighting)
            f.set_cursor_position((0, 0));
        }
        InputField::Note => {
            let cursor_x = calculate_cursor_x(&app.content.note, chunks[4].x, chunks[4].width);
            f.set_cursor_position((cursor_x, chunks[4].y + 1));
        }
    }
}
