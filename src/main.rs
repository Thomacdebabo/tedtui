mod markdown;
mod filestorage;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use filestorage::{FileStorage, Project};
use markdown::TodoData;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;

#[derive(Debug, Clone, PartialEq)]
enum InputField {
    Name,
    ProjectId,
    Goal,
    Tasks,
    Note,
}

struct App {
    name: String,
    project_id: String,
    goal: String,
    tasks: Vec<String>,
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
        }
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

        let todo_data = TodoData {
            name: self.name.clone(),
            project_id: self.project_id.clone(),
            project_shorthand,
            goal: self.goal.clone(),
            tasks: self.tasks.clone(),
            note: self.note.clone(),
        };

        match todo_data.save_to_markdown(&self.output_dir) {
            Ok(filepath) => {
                self.status_message = Some(format!("✓ Saved to: {}", filepath));
                self.saved_filepath = Some(filepath);
                // Clear all fields for a new todo
                self.clear_form();
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

    fn next_field(&mut self) {
        self.current_field = match self.current_field {
            InputField::Name => InputField::ProjectId,
            InputField::ProjectId => InputField::Goal,
            InputField::Goal => InputField::Tasks,
            InputField::Tasks => InputField::Note,
            InputField::Note => InputField::Name,
        };
    }

    fn previous_field(&mut self) {
        self.current_field = match self.current_field {
            InputField::Name => InputField::Note,
            InputField::ProjectId => InputField::Name,
            InputField::Goal => InputField::ProjectId,
            InputField::Tasks => InputField::Goal,
            InputField::Note => InputField::Tasks,
        };
    }

    fn get_current_input_mut(&mut self) -> &mut String {
        match self.current_field {
            InputField::Name => &mut self.name,
            InputField::ProjectId => &mut self.project_id,
            InputField::Goal => &mut self.goal,
            InputField::Tasks => &mut self.current_task_input,
            InputField::Note => &mut self.note,
        }
    }

    fn add_task(&mut self) {
        if !self.current_task_input.trim().is_empty() {
            self.tasks.push(self.current_task_input.clone());
            self.current_task_input.clear();
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

fn main() -> Result<(), io::Error> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run it
    let app = App::new();
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
                KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if app.current_field == InputField::ProjectId {
                        app.toggle_project_selector();
                    }
                }
                KeyCode::Esc => {
                    app.quit = true;
                }
                KeyCode::Tab => {
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        app.previous_field();
                    } else {
                        app.next_field();
                    }
                }
                KeyCode::Enter => {
                    if app.current_field == InputField::Tasks {
                        app.add_task();
                    }
                }
                KeyCode::Backspace => {
                    let input = app.get_current_input_mut();
                    input.pop();
                }
                KeyCode::Char(c) => {
                    let input = app.get_current_input_mut();
                    input.push(c);
                }
                KeyCode::Up => {
                    if app.current_field == InputField::Tasks {
                        app.move_task_selection_up();
                    }
                }
                KeyCode::Down => {
                    if app.current_field == InputField::Tasks {
                        app.move_task_selection_down();
                    }
                }
                KeyCode::Delete => {
                    if app.current_field == InputField::Tasks {
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
            let style = if Some(i) == app.selected_task_index {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };
            ListItem::new(format!("  - [ ] {}", task)).style(style)
        })
        .collect();

    let tasks_list = List::new(task_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Tasks (↑↓ to select, Del to delete)"),
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
            Span::styled("Ctrl+P", Style::default().fg(Color::Cyan)),
            Span::raw(" - Select Project | "),
            Span::styled("Ctrl+S", Style::default().fg(Color::Cyan)),
            Span::raw(" - Save | "),
            Span::styled("Esc/Ctrl+C", Style::default().fg(Color::Cyan)),
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

    // Show cursor in the active field
    match app.current_field {
        InputField::Name => {
            let visible_len = app.name.len().min((chunks[0].width - 3) as usize);
            let cursor_x = if app.name.len() > (chunks[0].width - 3) as usize {
                chunks[0].width - 2
            } else {
                chunks[0].x + visible_len as u16 + 1
            };
            f.set_cursor_position((cursor_x, chunks[0].y + 1));
        }
        InputField::ProjectId => {
            let visible_len = app.project_id.len().min((chunks[1].width - 3) as usize);
            let cursor_x = if app.project_id.len() > (chunks[1].width - 3) as usize {
                chunks[1].width - 2
            } else {
                chunks[1].x + visible_len as u16 + 1
            };
            f.set_cursor_position((cursor_x, chunks[1].y + 1));
        }
        InputField::Goal => {
            let visible_len = app.goal.len().min((chunks[2].width - 3) as usize);
            let cursor_x = if app.goal.len() > (chunks[2].width - 3) as usize {
                chunks[2].width - 2
            } else {
                chunks[2].x + visible_len as u16 + 1
            };
            f.set_cursor_position((cursor_x, chunks[2].y + 1));
        }
        InputField::Tasks => {
            let visible_len = app.current_task_input.len().min((tasks_chunks[0].width - 3) as usize);
            let cursor_x = if app.current_task_input.len() > (tasks_chunks[0].width - 3) as usize {
                tasks_chunks[0].width - 2
            } else {
                tasks_chunks[0].x + visible_len as u16 + 1
            };
            f.set_cursor_position((cursor_x, tasks_chunks[0].y + 1));
        }
        InputField::Note => {
            let visible_len = app.note.len().min((chunks[4].width - 3) as usize);
            let cursor_x = if app.note.len() > (chunks[4].width - 3) as usize {
                chunks[4].width - 2
            } else {
                chunks[4].x + visible_len as u16 + 1
            };
            f.set_cursor_position((cursor_x, chunks[4].y + 1));
        }
    }
}
