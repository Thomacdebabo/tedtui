mod markdown;
mod filestorage;
mod parser;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use filestorage::{FileStorage, Project};
use markdown::TodoData;
use parser::parse_markdown_file;
use unicode_width::UnicodeWidthStr;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
enum InputField {
    Name,
    ProjectId,
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

struct App {
    name: String,
    project_id: String,
    goal: String,
    tasks: Vec<Task>,
    current_task_input: String,
    note: String,
    current_field: InputField,
    selected_task_index: Option<usize>,
    quit: bool,
    status_message: Option<String>,
    output_dir: String,
    projects: Vec<Project>,
    selected_project_index: Option<usize>,
    show_project_selector: bool,
    saved_filepath: Option<String>,
    original_id: Option<String>,
    original_created: Option<String>,
    original_project_id: Option<String>,
    show_complete_confirmation: bool,
    completed_timestamp: Option<String>,
}

impl App {
    fn new() -> App {
        let file_storage = FileStorage::new();
        let projects = file_storage.get_projects().unwrap_or_default();
        let output_dir = file_storage.get_todos_dir().to_string_lossy().to_string();
        
        App {
            name: String::new(),
            project_id: String::new(),
            goal: String::new(),
            tasks: Vec::new(),
            current_task_input: String::new(),
            note: String::new(),
            current_field: InputField::Name,
            selected_task_index: None,
            quit: false,
            status_message: None,
            output_dir,
            projects,
            selected_project_index: None,
            show_project_selector: false,
            saved_filepath: None,
            original_id: None,
            original_created: None,
            original_project_id: None,
            show_complete_confirmation: false,
            completed_timestamp: None,
        }
    }

    fn from_file(filepath: &str) -> io::Result<App> {
        let path = PathBuf::from(filepath);
        let parsed = parse_markdown_file(&path)?;
        
        let file_storage = FileStorage::new();
        let projects = file_storage.get_projects().unwrap_or_default();
        let output_dir = file_storage.get_todos_dir().to_string_lossy().to_string();
        
        // Store these before moving parsed values
        let project_id = parsed.project_id.clone();
        
        Ok(App {
            name: parsed.name,
            project_id: parsed.project_id,
            goal: parsed.goal,
            tasks: parsed.tasks.into_iter().map(|t| Task {
                text: t.text,
                completed: t.completed,
            }).collect(),
            current_task_input: String::new(),
            note: parsed.note,
            current_field: InputField::Name,
            selected_task_index: None,
            quit: false,
            status_message: Some(format!("Loaded: {}", filepath)),
            output_dir,
            projects,
            selected_project_index: None,
            show_project_selector: false,
            saved_filepath: Some(filepath.to_string()),
            original_id: Some(parsed.id),
            original_created: Some(parsed.created),
            original_project_id: Some(project_id),
            show_complete_confirmation: false,
            completed_timestamp: None,
        })
    }

    fn save_to_file(&mut self) {
        if self.name.is_empty() {
            self.status_message = Some("Error: Name is required!".to_string());
            return;
        }

        // Get selected project shorthand
        // First try from selected index, then lookup by project_id
        let project_shorthand = self.selected_project_index
            .and_then(|idx| self.projects.get(idx))
            .and_then(|p| p.shorthand.clone())
            .or_else(|| {
                // If no selection but project_id is set, look it up
                if !self.project_id.is_empty() {
                    self.projects.iter()
                        .find(|p| p.id == self.project_id)
                        .and_then(|p| p.shorthand.clone())
                } else {
                    None
                }
            });

        // Check if project changed during edit
        let project_changed = self.saved_filepath.is_some() 
            && self.original_project_id.as_ref() != Some(&self.project_id);
        
        let old_filepath = self.saved_filepath.clone();
        let (target_filepath, updated_id) = if project_changed {
            // Extract numeric ID from original ID (e.g., "WGR123" -> 123, "T00061" -> 61)
            if let Some(ref orig_id) = self.original_id {
                use regex::Regex;
                let re = Regex::new(r"^[A-Z]+(\d+)$").ok();
                let numeric_id = re.and_then(|r| r.captures(orig_id))
                    .and_then(|caps| caps.get(1))
                    .and_then(|m| m.as_str().parse::<u32>().ok());
                
                if let Some(id_num) = numeric_id {
                    // Generate new ID with new shorthand
                    let new_id = if let Some(ref shorthand) = project_shorthand {
                        format!("{}{:03}", shorthand, id_num)
                    } else {
                        format!("T{:05}", id_num)
                    };
                    
                    // Generate new filename with existing ID
                    // Sanitize name the same way FileStorage does (preserve unicode)
                    let sanitized_name: String = self.name.chars()
                        .map(|c| match c {
                            ' ' => '_',
                            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                            c if c.is_control() => '_',
                            _ => c,
                        })
                        .collect::<String>()
                        .to_lowercase();
                    
                    let new_filename = if let Some(ref shorthand) = project_shorthand {
                        format!("{}{:03}_{}.md", shorthand, id_num, sanitized_name)
                    } else {
                        format!("T{:05}_{}.md", id_num, sanitized_name)
                    };
                    
                    let new_path = PathBuf::from(&self.output_dir).join(&new_filename);
                    
                    (Some(new_path.to_string_lossy().to_string()), Some(new_id))
                } else {
                    (self.saved_filepath.clone(), self.original_id.clone())
                }
            } else {
                (self.saved_filepath.clone(), self.original_id.clone())
            }
        } else {
            (self.saved_filepath.clone(), self.original_id.clone())
        };

        let todo_data = TodoData {
            name: self.name.clone(),
            project_id: self.project_id.clone(),
            project_shorthand,
            goal: self.goal.clone(),
            tasks: self.tasks.iter().map(|t| {
                let prefix = if t.completed { "[x] " } else { "[ ] " };
                format!("{}{}", prefix, t.text)
            }).collect(),
            note: self.note.clone(),
            existing_id: updated_id.clone(),
            existing_created: self.original_created.clone(),
            target_filepath: target_filepath.clone(),
            completed: self.completed_timestamp.clone(),
        };

        match todo_data.save_to_markdown(&self.output_dir) {
            Ok(filepath) => {
                // Delete old file if project changed
                if project_changed {
                    if let Some(ref old_path) = old_filepath {
                        if PathBuf::from(old_path).exists() {
                            let _ = fs::remove_file(old_path);
                        }
                    }
                    if let Some(new_id) = updated_id {
                        self.original_id = Some(new_id);
                    }
                    self.original_project_id = Some(self.project_id.clone());
                }
                
                let msg = if self.saved_filepath.is_some() {
                    if project_changed {
                        format!("✓ Updated and moved: {}", filepath)
                    } else {
                        format!("✓ Updated: {}", filepath)
                    }
                } else {
                    format!("✓ Saved to: {}" , filepath)
                };
                self.status_message = Some(msg);
                
                // Update saved filepath
                self.saved_filepath = Some(filepath);
                
                // If creating a new file, clear form
                if old_filepath.is_none() {
                    self.clear_form();
                }
            }
            Err(e) => {
                self.status_message = Some(format!("✗ Error saving: {}", e));
            }
        }
    }

    fn clear_form(&mut self) {
        self.name.clear();
        self.project_id.clear();
        self.goal.clear();
        self.tasks.clear();
        self.current_task_input.clear();
        self.note.clear();
        self.selected_task_index = None;
        self.selected_project_index = None;
        self.current_field = InputField::Name;
        self.original_id = None;
        self.original_created = None;
        self.original_project_id = None;
        self.saved_filepath = None;
        self.completed_timestamp = None;
    }

    fn toggle_project_selector(&mut self) {
        self.show_project_selector = !self.show_project_selector;
        if self.show_project_selector && self.selected_project_index.is_none() && !self.projects.is_empty() {
            self.selected_project_index = Some(0);
        }
    }

    fn select_project(&mut self) {
        if let Some(idx) = self.selected_project_index {
            if let Some(project) = self.projects.get(idx) {
                self.project_id = project.id.clone();
            }
        }
        self.show_project_selector = false;
    }

    fn move_project_selection_up(&mut self) {
        if !self.projects.is_empty() {
            self.selected_project_index = Some(match self.selected_project_index {
                Some(i) if i > 0 => i - 1,
                Some(_) => self.projects.len() - 1,
                None => 0,
            });
        }
    }

    fn move_project_selection_down(&mut self) {
        if !self.projects.is_empty() {
            self.selected_project_index = Some(match self.selected_project_index {
                Some(i) if i < self.projects.len() - 1 => i + 1,
                Some(_) => 0,
                None => 0,
            });
        }
    }

    fn has_incomplete_tasks(&self) -> bool {
        self.tasks.iter().any(|t| !t.completed)
    }

    fn mark_all_tasks_complete(&mut self) {
        for task in &mut self.tasks {
            task.completed = true;
        }
    }

    fn move_to_done(&mut self) -> io::Result<()> {
        if self.saved_filepath.is_none() {
            self.status_message = Some("Error: No file to move. Save first.".to_string());
            return Ok(());
        }

        let source_path = self.saved_filepath.as_ref().unwrap();
        let source = PathBuf::from(source_path);
        
        if !source.exists() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "Source file not found"));
        }

        // Set completed timestamp
        use chrono::Local;
        self.completed_timestamp = Some(Local::now().format("%m-%d-%Y_%H:%M:%S").to_string());

        // Get filename from source
        let filename = source.file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid filename"))?
            .to_string_lossy()
            .to_string();

        // Build destination path in done directory
        let file_storage = FileStorage::new();
        let done_dir = file_storage.get_done_dir();
        let dest = done_dir.join(&filename);

        // Save with completed timestamp to destination
        let project_shorthand = self.selected_project_index
            .and_then(|idx| self.projects.get(idx))
            .and_then(|p| p.shorthand.clone())
            .or_else(|| {
                if !self.project_id.is_empty() {
                    self.projects.iter()
                        .find(|p| p.id == self.project_id)
                        .and_then(|p| p.shorthand.clone())
                } else {
                    None
                }
            });

        let todo_data = TodoData {
            name: self.name.clone(),
            project_id: self.project_id.clone(),
            project_shorthand,
            goal: self.goal.clone(),
            tasks: self.tasks.iter().map(|t| {
                let prefix = if t.completed { "[x] " } else { "[ ] " };
                format!("{}{}", prefix, t.text)
            }).collect(),
            note: self.note.clone(),
            existing_id: self.original_id.clone(),
            existing_created: self.original_created.clone(),
            target_filepath: Some(dest.to_string_lossy().to_string()),
            completed: self.completed_timestamp.clone(),
        };

        // Save to done directory
        todo_data.save_to_markdown(&done_dir.to_string_lossy())?;

        // Delete original file from todos
        fs::remove_file(&source)?;

        self.status_message = Some(format!("✓ Moved to done: {}", filename));
        self.saved_filepath = Some(dest.to_string_lossy().to_string());
        
        Ok(())
    }

    fn next_field(&mut self) {
        self.current_field = match self.current_field {
            InputField::Name => InputField::ProjectId,
            InputField::ProjectId => InputField::Goal,
            InputField::Goal => InputField::Tasks,
            InputField::Tasks => {
                // When leaving task input, clear selection
                self.selected_task_index = None;
                InputField::TaskList
            }
            InputField::TaskList => {
                // When leaving task list, clear selection
                self.selected_task_index = None;
                InputField::Note
            }
            InputField::Note => InputField::Name,
        };
        
        // When entering TaskList, select first task if available
        if self.current_field == InputField::TaskList && !self.tasks.is_empty() {
            self.selected_task_index = Some(0);
        }
    }

    fn previous_field(&mut self) {
        self.current_field = match self.current_field {
            InputField::Name => InputField::Note,
            InputField::ProjectId => InputField::Name,
            InputField::Goal => InputField::ProjectId,
            InputField::Tasks => InputField::Goal,
            InputField::TaskList => {
                // When leaving task list, clear selection
                self.selected_task_index = None;
                InputField::Tasks
            }
            InputField::Note => {
                // When leaving note, clear selection
                self.selected_task_index = None;
                InputField::TaskList
            }
        };
        
        // When entering TaskList, select first task if available
        if self.current_field == InputField::TaskList && !self.tasks.is_empty() {
            self.selected_task_index = Some(0);
        }
    }

    fn get_current_input_mut(&mut self) -> &mut String {
        match self.current_field {
            InputField::Name => &mut self.name,
            InputField::ProjectId => &mut self.project_id,
            InputField::Goal => &mut self.goal,
            InputField::Tasks => &mut self.current_task_input,
            InputField::TaskList => &mut self.current_task_input, // Not really used, but needs to return something
            InputField::Note => &mut self.note,
        }
    }

    fn add_task(&mut self) {
        if !self.current_task_input.trim().is_empty() {
            self.tasks.push(Task {
                text: self.current_task_input.clone(),
                completed: false,
            });
            self.current_task_input.clear();
        }
    }

    fn toggle_task_completion(&mut self) {
        if let Some(index) = self.selected_task_index {
            if let Some(task) = self.tasks.get_mut(index) {
                task.completed = !task.completed;
            }
        }
    }

    fn delete_selected_task(&mut self) {
        if let Some(index) = self.selected_task_index {
            if index < self.tasks.len() {
                self.tasks.remove(index);
                if self.tasks.is_empty() {
                    self.selected_task_index = None;
                } else if index >= self.tasks.len() {
                    self.selected_task_index = Some(self.tasks.len() - 1);
                }
            }
        }
    }

    fn move_task_selection_up(&mut self) {
        if !self.tasks.is_empty() {
            self.selected_task_index = Some(match self.selected_task_index {
                Some(i) if i > 0 => i - 1,
                Some(_) => self.tasks.len() - 1,
                None => 0,
            });
        }
    }

    fn move_task_selection_down(&mut self) {
        if !self.tasks.is_empty() {
            self.selected_task_index = Some(match self.selected_task_index {
                Some(i) if i < self.tasks.len() - 1 => i + 1,
                Some(_) => 0,
                None => 0,
            });
        }
    }
}

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
    if let Some(caps) = re.captures(filename) {
        if let Some(id_str) = caps.get(1) {
            return id_str.as_str().parse::<u32>().ok();
        }
    }
    None
}

fn main() -> Result<(), io::Error> {
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    
    let app = if args.len() > 1 {
        let arg = &args[1];
        
        // Check if it's a file path
        if arg.ends_with(".md") {
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
            eprintln!("Usage: tedtui [file.md|ID]");
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid argument"));
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

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &app)).map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{}", e)))?;

        if let Event::Key(key) = event::read()? {
            // Clear status message on any key press
            app.status_message = None;
            
            // Handle completion confirmation dialog
            if app.show_complete_confirmation {
                match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        app.mark_all_tasks_complete();
                        app.show_complete_confirmation = false;
                        if let Err(e) = app.move_to_done() {
                            app.status_message = Some(format!("Error moving to done: {}", e));
                        }
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                        app.show_complete_confirmation = false;
                        app.status_message = Some("Move cancelled.".to_string());
                    }
                    _ => {}
                }
                continue;
            }
            
            // Handle project selector navigation
            if app.show_project_selector {
                match key.code {
                    KeyCode::Esc => {
                        app.show_project_selector = false;
                    }
                    KeyCode::Up => {
                        app.move_project_selection_up();
                    }
                    KeyCode::Down => {
                        app.move_project_selection_down();
                    }
                    KeyCode::Enter => {
                        app.select_project();
                    }
                    _ => {}
                }
                continue;
            }
            
            match key.code {
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.quit = true;
                }
                KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.save_to_file();
                }
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if app.saved_filepath.is_some() {
                        if app.has_incomplete_tasks() {
                            app.show_complete_confirmation = true;
                        } else {
                            if let Err(e) = app.move_to_done() {
                                app.status_message = Some(format!("Error moving to done: {}", e));
                            }
                        }
                    } else {
                        app.status_message = Some("Save the file first before moving to done.".to_string());
                    }
                }
                KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if app.current_field == InputField::ProjectId {
                        app.toggle_project_selector();
                    }
                }
                KeyCode::Esc => {
                    app.quit = true;
                }
                KeyCode::Tab => {
                    app.next_field();
                }
                KeyCode::BackTab => {
                    app.previous_field();
                }
                KeyCode::Enter => {
                    if app.current_field == InputField::Tasks {
                        app.add_task();
                    }
                }
                KeyCode::Backspace => {
                    // Only allow backspace in input fields, not in TaskList
                    if app.current_field != InputField::TaskList {
                        let input = app.get_current_input_mut();
                        input.pop();
                    }
                }
                KeyCode::Char(c) => {
                    // Space toggles task completion only in TaskList
                    if c == ' ' && app.current_field == InputField::TaskList {
                        app.toggle_task_completion();
                    } else if app.current_field != InputField::TaskList {
                        // Allow typing in all fields except TaskList
                        let input = app.get_current_input_mut();
                        input.push(c);
                    }
                }
                KeyCode::Up => {
                    if app.current_field == InputField::TaskList {
                        app.move_task_selection_up();
                    }
                }
                KeyCode::Down => {
                    if app.current_field == InputField::TaskList {
                        app.move_task_selection_down();
                    }
                }
                KeyCode::Delete => {
                    if app.current_field == InputField::TaskList {
                        app.delete_selected_task();
                    }
                }
                _ => {}
            }
        }

        if app.quit {
            break;
        }
    }

    Ok(())
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(4), // Name
            Constraint::Length(4), // Project ID
            Constraint::Length(5), // Goal
            Constraint::Min(10),    // Tasks
            Constraint::Length(8), // Note
            Constraint::Length(3), // Help
            Constraint::Length(2), // Status
        ])
        .split(f.area());

    // Name input
    let name_block = Block::default()
        .borders(Borders::ALL)
        .title("Name")
        .border_style(if app.current_field == InputField::Name {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });
    let name_text = if app.name.len() > chunks[0].width as usize - 4 {
        &app.name[app.name.len().saturating_sub(chunks[0].width as usize - 4)..]
    } else {
        &app.name
    };
    let name_paragraph = Paragraph::new(name_text)
        .block(name_block)
        .style(Style::default());
    f.render_widget(name_paragraph, chunks[0]);

    // Project ID input
    let project_title = if app.current_field == InputField::ProjectId {
        "Project ID (Ctrl+P to select)"
    } else {
        "Project ID"
    };
    let project_block = Block::default()
        .borders(Borders::ALL)
        .title(project_title)
        .border_style(if app.current_field == InputField::ProjectId {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });
    let project_text = if app.project_id.len() > chunks[1].width as usize - 4 {
        &app.project_id[app.project_id.len().saturating_sub(chunks[1].width as usize - 4)..]
    } else {
        &app.project_id
    };
    let project_paragraph = Paragraph::new(project_text)
        .block(project_block)
        .style(Style::default());
    f.render_widget(project_paragraph, chunks[1]);

    // Goal input
    let goal_block = Block::default()
        .borders(Borders::ALL)
        .title("Goal / Short Description")
        .border_style(if app.current_field == InputField::Goal {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });
    let goal_paragraph = Paragraph::new(app.goal.as_str())
        .block(goal_block)
        .wrap(Wrap { trim: false })
        .style(Style::default());
    f.render_widget(goal_paragraph, chunks[2]);

    // Tasks section
    let tasks_area = chunks[3];
    let tasks_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(tasks_area);

    // Task input
    let task_input_block = Block::default()
        .borders(Borders::ALL)
        .title("Add Task (Enter to add)")
        .border_style(if app.current_field == InputField::Tasks {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });
    let task_text = if app.current_task_input.len() > tasks_chunks[0].width as usize - 4 {
        &app.current_task_input[app.current_task_input.len().saturating_sub(tasks_chunks[0].width as usize - 4)..]
    } else {
        &app.current_task_input
    };
    let task_input_paragraph = Paragraph::new(task_text)
        .block(task_input_block)
        .style(Style::default());
    f.render_widget(task_input_paragraph, tasks_chunks[0]);

    // Task list
    let task_items: Vec<ListItem> = app
        .tasks
        .iter()
        .enumerate()
        .map(|(i, task)| {
            let checkbox = if task.completed { "[x]" } else { "[ ]" };
            let text = format!("  - {} {}", checkbox, task.text);
            
            let style = if Some(i) == app.selected_task_index {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else if task.completed {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            };
            ListItem::new(text).style(style)
        })
        .collect();

    let tasks_list = List::new(task_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Task List (Tab to enter, ↑↓ select, Space toggle, Del delete)")
                .border_style(if app.current_field == InputField::TaskList {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                }),
        )
        .style(Style::default());
    f.render_widget(tasks_list, tasks_chunks[1]);

    // Note input
    let note_block = Block::default()
        .borders(Borders::ALL)
        .title("Note")
        .border_style(if app.current_field == InputField::Note {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });
    let note_paragraph = Paragraph::new(app.note.as_str())
        .block(note_block)
        .wrap(Wrap { trim: false })
        .style(Style::default());
    f.render_widget(note_paragraph, chunks[4]);

    // Help text
    let help_text = vec![
        Line::from(vec![
            Span::styled("Tab", Style::default().fg(Color::Cyan)),
            Span::raw(" / "),
            Span::styled("Shift+Tab", Style::default().fg(Color::Cyan)),
            Span::raw(" - Navigate | "),
            Span::styled("Space", Style::default().fg(Color::Cyan)),
            Span::raw(" - Toggle task | "),
            Span::styled("Ctrl+P", Style::default().fg(Color::Cyan)),
            Span::raw(" - Projects | "),
            Span::styled("Ctrl+S", Style::default().fg(Color::Cyan)),
            Span::raw(" - Save | "),
            Span::styled("Ctrl+D", Style::default().fg(Color::Cyan)),
            Span::raw(" - Done | "),
            Span::styled("Esc", Style::default().fg(Color::Cyan)),
            Span::raw(" - Quit"),
        ]),
    ];
    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .style(Style::default());
    f.render_widget(help, chunks[5]);

    // Status message
    if let Some(ref msg) = app.status_message {
        let status_color = if msg.contains("✓") {
            Color::Green
        } else if msg.contains("✗") {
            Color::Red
        } else {
            Color::Yellow
        };
        let status = Paragraph::new(msg.as_str())
            .style(Style::default().fg(status_color))
            .wrap(Wrap { trim: false });
        f.render_widget(status, chunks[6]);
    }

    // Project selector overlay
    if app.show_project_selector {
        // Calculate popup size
        let popup_width = f.area().width.saturating_sub(20).max(40);
        let popup_height = f.area().height.saturating_sub(10).max(15).min(30);
        let popup_x = (f.area().width.saturating_sub(popup_width)) / 2;
        let popup_y = (f.area().height.saturating_sub(popup_height)) / 2;
        
        let popup_area = ratatui::layout::Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width,
            height: popup_height,
        };
        
        // Create project list items
        let project_items: Vec<ListItem> = app
            .projects
            .iter()
            .enumerate()
            .map(|(i, project)| {
                let style = if Some(i) == app.selected_project_index {
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
        
        f.render_widget(projects_list, popup_area);
    }

    // Completion confirmation overlay
    if app.show_complete_confirmation {
        // Calculate popup size
        let popup_width = 60;
        let popup_height = 7;
        let popup_x = (f.area().width.saturating_sub(popup_width)) / 2;
        let popup_y = (f.area().height.saturating_sub(popup_height)) / 2;
        
        let popup_area = ratatui::layout::Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width,
            height: popup_height,
        };
        
        let incomplete_count = app.tasks.iter().filter(|t| !t.completed).count();
        let message = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("You have {} incomplete task(s).", incomplete_count),
                Style::default().fg(Color::White)
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Mark all tasks as complete before moving to done?",
                Style::default().fg(Color::Yellow)
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("Press ", Style::default().fg(Color::White)),
                Span::styled("Y", Style::default().fg(Color::Green)),
                Span::styled(" to mark complete and move, ", Style::default().fg(Color::White)),
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
        
        f.render_widget(confirmation, popup_area);
    }

    // Show cursor in the active field
    match app.current_field {
        InputField::Name => {
            let text_width = app.name.width();
            let cursor_x = if text_width > (chunks[0].width - 3) as usize {
                chunks[0].width - 2
            } else {
                chunks[0].x + text_width as u16 + 1
            };
            f.set_cursor_position((cursor_x, chunks[0].y + 1));
        }
        InputField::ProjectId => {
            let text_width = app.project_id.width();
            let cursor_x = if text_width > (chunks[1].width - 3) as usize {
                chunks[1].width - 2
            } else {
                chunks[1].x + text_width as u16 + 1
            };
            f.set_cursor_position((cursor_x, chunks[1].y + 1));
        }
        InputField::Goal => {
            let text_width = app.goal.width();
            let cursor_x = if text_width > (chunks[2].width - 3) as usize {
                chunks[2].width - 2
            } else {
                chunks[2].x + text_width as u16 + 1
            };
            f.set_cursor_position((cursor_x, chunks[2].y + 1));
        }
        InputField::Tasks => {
            let text_width = app.current_task_input.width();
            let cursor_x = if text_width > (tasks_chunks[0].width - 3) as usize {
                tasks_chunks[0].width - 2
            } else {
                tasks_chunks[0].x + text_width as u16 + 1
            };
            f.set_cursor_position((cursor_x, tasks_chunks[0].y + 1));
        }
        InputField::TaskList => {
            // In task list mode, hide cursor by placing it off-screen or at a neutral position
            // The selected task is shown with background highlighting instead
            f.set_cursor_position((0, 0));
        }
        InputField::Note => {
            let text_width = app.note.width();
            let cursor_x = if text_width > (chunks[4].width - 3) as usize {
                chunks[4].width - 2
            } else {
                chunks[4].x + text_width as u16 + 1
            };
            f.set_cursor_position((cursor_x, chunks[4].y + 1));
        }
    }
}
