#![allow(dead_code)]

mod filestorage;
mod parser;
mod theme;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use filestorage::FileStorage;
use parser::parse_markdown_file;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List as TuiList, ListItem, Paragraph},
};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use theme::Theme;

#[derive(Clone)]
struct TodoFile {
    path: PathBuf,
    parsed: parser::ParsedTodo,
}

impl TodoFile {
    fn status_indicator(&self) -> &'static str {
        if self.parsed.tasks.is_empty() {
            " "
        } else if self.parsed.tasks.iter().all(|t| t.completed) {
            "\u{2713}"
        } else {
            " "
        }
    }

    fn completion_summary(&self) -> String {
        let total = self.parsed.tasks.len();
        if total == 0 {
            String::new()
        } else {
            let done = self.parsed.tasks.iter().filter(|t| t.completed).count();
            format!("{}/{}", done, total)
        }
    }
}

struct ProjectGroup {
    project: filestorage::Project,
    todos: Vec<TodoFile>,
}

struct AppData {
    groups: Vec<ProjectGroup>,
    unassigned: Vec<TodoFile>,
    all_todos: Vec<TodoFile>,
}

impl AppData {
    fn load() -> io::Result<Self> {
        let storage = FileStorage::new();
        let projects = storage.get_projects().unwrap_or_default();
        let todos_dir = storage.get_todos_dir();

        let mut groups: Vec<ProjectGroup> = projects
            .into_iter()
            .map(|p| ProjectGroup {
                project: p,
                todos: Vec::new(),
            })
            .collect();

        let mut unassigned = Vec::new();

        if let Ok(entries) = fs::read_dir(&todos_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("md") {
                    continue;
                }
                if let Ok(parsed) = parse_markdown_file(&path) {
                    let todo = TodoFile { path, parsed };
                    let pid = todo.parsed.project_id.clone();
                    if pid.is_empty() {
                        unassigned.push(todo);
                    } else {
                        let pos = groups.iter().position(|g| g.project.id == pid);
                        if let Some(idx) = pos {
                            groups[idx].todos.push(todo);
                        } else {
                            unassigned.push(todo);
                        }
                    }
                }
            }
        }

        groups.sort_by(|a, b| a.project.name.cmp(&b.project.name));
        for g in &mut groups {
            g.todos.sort_by(|a, b| a.parsed.name.cmp(&b.parsed.name));
        }
        unassigned.sort_by(|a, b| a.parsed.name.cmp(&b.parsed.name));

        let mut all_todos = Vec::new();
        for g in &groups {
            all_todos.extend(g.todos.iter().cloned());
        }
        all_todos.extend(unassigned.iter().cloned());
        all_todos.sort_by(|a, b| a.parsed.name.cmp(&b.parsed.name));

        Ok(AppData {
            groups,
            unassigned,
            all_todos,
        })
    }

    fn visible_groups(&self, show_empty: bool) -> Vec<&ProjectGroup> {
        if show_empty {
            self.groups.iter().collect()
        } else {
            self.groups.iter().filter(|g| !g.todos.is_empty()).collect()
        }
    }

    fn project_names_with_counts(&self, show_empty: bool) -> Vec<(String, usize)> {
        let mut items = Vec::new();
        items.push(("All Todos".to_string(), self.all_todos.len()));
        for g in self.visible_groups(show_empty) {
            items.push((g.project.name.clone(), g.todos.len()));
        }
        if !self.unassigned.is_empty() {
            items.push(("Unassigned".to_string(), self.unassigned.len()));
        }
        items
    }

    fn project_count(&self, show_empty: bool) -> usize {
        let mut c = 1 + self.visible_groups(show_empty).len();
        if !self.unassigned.is_empty() {
            c += 1;
        }
        c
    }

    fn current_todos(&self, project_idx: usize, show_empty: bool) -> &[TodoFile] {
        if project_idx == 0 {
            &self.all_todos
        } else if project_idx <= self.visible_groups(show_empty).len() {
            &self.visible_groups(show_empty)[project_idx - 1].todos
        } else {
            &self.unassigned
        }
    }

    fn current_project_name(&self, project_idx: usize, show_empty: bool) -> &str {
        if project_idx == 0 {
            "All Todos"
        } else if project_idx <= self.visible_groups(show_empty).len() {
            &self.visible_groups(show_empty)[project_idx - 1].project.name
        } else {
            "Unassigned"
        }
    }
}

#[derive(PartialEq)]
enum Focus {
    Projects,
    Todos,
}

#[derive(Clone, Copy, PartialEq)]
enum SortMode {
    Alpha,
    MostTodos,
    LeastTodos,
}

impl SortMode {
    fn next(self) -> Self {
        match self {
            SortMode::Alpha => SortMode::MostTodos,
            SortMode::MostTodos => SortMode::LeastTodos,
            SortMode::LeastTodos => SortMode::Alpha,
        }
    }

    fn label(self) -> &'static str {
        match self {
            SortMode::Alpha => "A-Z",
            SortMode::MostTodos => "Most",
            SortMode::LeastTodos => "Least",
        }
    }
}

struct App {
    data: AppData,
    focus: Focus,
    selected_project: usize,
    selected_todo: usize,
    detail_open: bool,
    detail_scroll: usize,
    overview_open: bool,
    sort_mode: SortMode,
    show_empty: bool,
    theme: Theme,
    quit: bool,
}

impl App {
    fn new() -> io::Result<Self> {
        let data = AppData::load()?;
        let theme = Theme::load();
        Ok(App {
            data,
            focus: Focus::Projects,
            selected_project: 0,
            selected_todo: 0,
            detail_open: false,
            detail_scroll: 0,
            overview_open: false,
            sort_mode: SortMode::Alpha,
            show_empty: false,
            theme,
            quit: false,
        })
    }

    fn current_todos(&self) -> &[TodoFile] {
        self.data.current_todos(self.selected_project, self.show_empty)
    }

    fn adjust_selected_todo(&mut self) {
        let count = self.current_todos().len();
        if count == 0 {
            self.selected_todo = 0;
        } else if self.selected_todo >= count {
            self.selected_todo = count - 1;
        }
    }

    fn resort(&mut self) {
        match self.sort_mode {
            SortMode::Alpha => {
                self.data
                    .groups
                    .sort_by(|a, b| a.project.name.cmp(&b.project.name));
            }
            SortMode::MostTodos => {
                self.data
                    .groups
                    .sort_by(|a, b| b.todos.len().cmp(&a.todos.len()));
            }
            SortMode::LeastTodos => {
                self.data
                    .groups
                    .sort_by(|a, b| a.todos.len().cmp(&b.todos.len()));
            }
        }
        let visible_count = self.data.visible_groups(self.show_empty).len();
        if self.selected_project > visible_count {
            self.selected_project = visible_count;
        }
    }
}

fn render(app: &App, f: &mut Frame) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .margin(1)
        .split(f.area());

    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(main_chunks[0]);

    render_projects_panel(app, f, panels[0]);
    render_todo_list(app, f, panels[1]);
    render_help(app, f, main_chunks[1]);

    if app.detail_open {
        render_detail(app, f, f.area());
    }
    if app.overview_open {
        render_overview(app, f, f.area());
    }
}

fn render_projects_panel(app: &App, f: &mut Frame, area: Rect) {
    let items = app.data.project_names_with_counts(app.show_empty);
    let list_items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, (name, count))| {
            let text = format!(" {} ({})", name, count);
            let style = if app.focus == Focus::Projects && i == app.selected_project {
                app.theme.selected_style()
            } else {
                Style::default()
            };
            ListItem::new(text).style(style)
        })
        .collect();

    let empty_label = if app.show_empty { " +empty" } else { "" };
    let title = format!(" Projects ({}) [{}{}] ", items.len(), app.sort_mode.label(), empty_label);
    let list = TuiList::new(list_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(match app.focus {
                Focus::Projects => app.theme.active_border_style(),
                Focus::Todos => app.theme.inactive_border_style(),
            }),
    );

    f.render_widget(list, area);
}

fn render_todo_list(app: &App, f: &mut Frame, area: Rect) {
    let todos = app.current_todos();
    let title = format!(" {} ({}) ", app.data.current_project_name(app.selected_project, app.show_empty), todos.len());

    if todos.is_empty() {
        let para = Paragraph::new(" No todos yet ")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(match app.focus {
                        Focus::Todos => app.theme.active_border_style(),
                        Focus::Projects => app.theme.inactive_border_style(),
                    }),
            );
        f.render_widget(para, area);
        return;
    }

    let list_items: Vec<ListItem> = todos
        .iter()
        .enumerate()
        .map(|(i, todo)| {
            let summary = todo.completion_summary();
            let suffix = if summary.is_empty() {
                String::new()
            } else {
                format!(" [{}]", summary)
            };
            let prefix = match todo.status_indicator() {
                "\u{2713}" => "\u{2713} ",
                _ => "  ",
            };
            let text = format!("{}{}{}", prefix, todo.parsed.name, suffix);
            let style = if app.focus == Focus::Todos && i == app.selected_todo {
                app.theme.selected_style()
            } else if todo.status_indicator() == "\u{2713}" {
                Style::default().fg(app.theme.task_completed)
            } else {
                Style::default()
            };
            ListItem::new(text).style(style)
        })
        .collect();

    let list = TuiList::new(list_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(match app.focus {
                Focus::Todos => app.theme.active_border_style(),
                Focus::Projects => app.theme.inactive_border_style(),
            }),
    );

    f.render_widget(list, area);
}

fn render_help(app: &App, f: &mut Frame, area: Rect) {
    let t = &app.theme;
    let key_style = Style::default().fg(t.help_key);
    let text = Line::from(vec![
        Span::styled("Tab", key_style),
        Span::raw(" Focus  "),
        Span::styled("\u{2191}\u{2193}", key_style),
        Span::raw(" Navigate  "),
        Span::styled("Enter", key_style),
        Span::raw(" Detail  "),
        Span::styled("Ctrl+Enter", key_style),
        Span::raw(" Open  "),
        Span::styled("o", key_style),
        Span::raw(" Overview  "),
        Span::styled("s", key_style),
        Span::raw(" Sort  "),
        Span::styled("h", key_style),
        Span::raw(" Hide  "),
        Span::styled("q", key_style),
        Span::raw(" / "),
        Span::styled("Esc", key_style),
        Span::raw(" Quit"),
    ]);
    let para = Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(" Help "));
    f.render_widget(para, area);
}

fn render_detail(app: &App, f: &mut Frame, area: Rect) {
    let todos = app.current_todos();
    if todos.is_empty() {
        return;
    }

    let todo = &todos[app.selected_todo];

    let popup_width = area.width.saturating_sub(8).min(100);
    let popup_height = area.height.saturating_sub(4).min(40);
    let popup_x = (area.width - popup_width) / 2;
    let popup_y = (area.height - popup_height) / 2;

    let popup_area = Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width,
        height: popup_height,
    };

    f.render_widget(Clear, popup_area);

    let t = &app.theme;
    let bold = Style::default()
        .fg(t.field_text)
        .add_modifier(Modifier::BOLD);
    let normal = Style::default().fg(t.field_text);

    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(vec![
        Span::raw(" "),
        Span::styled("Name: ", bold),
        Span::styled(&todo.parsed.name, normal),
    ]));
    lines.push(Line::from(""));

    if !todo.parsed.project_id.is_empty() {
        lines.push(Line::from(vec![
            Span::raw(" "),
            Span::styled("Project: ", bold),
            Span::styled(&todo.parsed.project_id, normal),
        ]));
        lines.push(Line::from(""));
    }

    if !todo.parsed.info.is_empty() {
        lines.push(Line::from(vec![
            Span::raw(" "),
            Span::styled("Info: ", bold),
            Span::styled(&todo.parsed.info, normal),
        ]));
        lines.push(Line::from(""));
    }

    if !todo.parsed.goal.is_empty() {
        lines.push(Line::from(Span::styled(" Goal:", bold)));
        for line in todo.parsed.goal.lines() {
            lines.push(Line::from(Span::styled(format!(" {}", line), normal)));
        }
        lines.push(Line::from(""));
    }

    let done_count = todo.parsed.tasks.iter().filter(|t| t.completed).count();
    lines.push(Line::from(Span::styled(
        format!(" Tasks ({}/{}):", done_count, todo.parsed.tasks.len()),
        bold,
    )));
    for task in &todo.parsed.tasks {
        let checkbox = if task.completed { "\u{2713}" } else { " " };
        let style = if task.completed {
            Style::default().fg(t.task_completed)
        } else {
            normal
        };
        lines.push(Line::from(Span::styled(
            format!("   [{}] {}", checkbox, task.text),
            style,
        )));
    }
    lines.push(Line::from(""));

    if !todo.parsed.note.is_empty() {
        lines.push(Line::from(Span::styled(" Note:", bold)));
        for line in todo.parsed.note.lines() {
            lines.push(Line::from(Span::styled(format!(" {}", line), normal)));
        }
        lines.push(Line::from(""));
    }

    if !todo.parsed.history.is_empty() {
        lines.push(Line::from(Span::styled(" History:", bold)));
        let visible_history: Vec<&str> = todo
            .parsed
            .history
            .lines()
            .filter(|l| !l.trim().is_empty())
            .collect();
        for line in &visible_history {
            lines.push(Line::from(Span::styled(
                format!(" {}", line),
                Style::default().fg(t.history_entry),
            )));
        }
    }

    let visible_lines: Vec<Line> = lines
        .iter()
        .skip(app.detail_scroll)
        .take((popup_height - 2) as usize)
        .cloned()
        .collect();

    let para = Paragraph::new(visible_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(
                    " Detail {} (Esc close, \u{2191}\u{2193} scroll) ",
                    todo.parsed.name
                ))
                .border_style(Style::default().fg(t.active_border)),
        )
        .style(Style::default().bg(t.popup_bg));

    f.render_widget(para, popup_area);
}

fn render_overview(app: &App, f: &mut Frame, area: Rect) {
    let todos = app.current_todos();

    let popup_width = area.width.saturating_sub(16).min(80).max(40);
    let popup_height = 16u16;
    let popup_x = (area.width - popup_width) / 2;
    let popup_y = (area.height - popup_height) / 2;

    let popup_area = Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width,
        height: popup_height,
    };

    f.render_widget(Clear, popup_area);

    let t = &app.theme;
    let bold = Style::default()
        .fg(t.popup_text)
        .add_modifier(Modifier::BOLD);
    let normal = Style::default().fg(t.popup_text);

    let mut completed = 0usize;
    let mut in_progress = 0usize;
    let mut no_tasks = 0usize;
    let mut total_tasks = 0usize;
    let mut done_tasks = 0usize;

    for todo in todos {
        if todo.parsed.tasks.is_empty() {
            no_tasks += 1;
        } else if todo.parsed.tasks.iter().all(|t| t.completed) {
            completed += 1;
        } else {
            in_progress += 1;
        }
        total_tasks += todo.parsed.tasks.len();
        done_tasks += todo.parsed.tasks.iter().filter(|t| t.completed).count();
    }

    let task_summary = if total_tasks > 0 {
        format!("{}/{}", done_tasks, total_tasks)
    } else {
        "n/a".to_string()
    };

    let name = app.data.current_project_name(app.selected_project, app.show_empty);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(name, bold),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  Total todos:     "),
            Span::styled(todos.len().to_string(), bold),
        ]),
        Line::from(vec![
            Span::raw("  Completed:       "),
            Span::styled(completed.to_string(), Style::default().fg(t.status_success)),
        ]),
        Line::from(vec![
            Span::raw("  In progress:     "),
            Span::styled(in_progress.to_string(), Style::default().fg(t.status_info)),
        ]),
        Line::from(vec![
            Span::raw("  No tasks:        "),
            Span::styled(no_tasks.to_string(), normal),
        ]),
        Line::from(vec![
            Span::raw("  Tasks done/total: "),
            Span::styled(task_summary, bold),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Press any key to close",
            Style::default().fg(t.popup_hint),
        )),
    ];

    let para = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Overview ")
                .border_style(Style::default().fg(t.project_border)),
        )
        .style(t.popup_bg_style());

    f.render_widget(para, popup_area);
}

fn find_tedtui() -> Option<PathBuf> {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let candidate = exe_dir.join("tedtui");
            if candidate.exists() {
                return Some(candidate);
            }
            #[cfg(windows)]
            {
                let candidate = exe_dir.join("tedtui.exe");
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join("tedtui");
            if candidate.is_file() {
                return Some(candidate);
            }
            #[cfg(windows)]
            {
                let candidate = dir.join("tedtui.exe");
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

fn handle_events(app: &mut App) -> io::Result<Option<PathBuf>> {
    if let Event::Key(key) = event::read()? {
        if app.detail_open {
            match key.code {
                KeyCode::Esc => {
                    app.detail_open = false;
                    app.detail_scroll = 0;
                }
                KeyCode::Up => {
                    app.detail_scroll = app.detail_scroll.saturating_sub(1);
                }
                KeyCode::Down => {
                    app.detail_scroll += 1;
                }
                _ => {}
            }
            return Ok(None);
        }

        if app.overview_open {
            app.overview_open = false;
            return Ok(None);
        }

        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.quit = true;
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                app.quit = true;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                app.focus = match app.focus {
                    Focus::Projects => Focus::Todos,
                    Focus::Todos => Focus::Projects,
                };
            }
            KeyCode::Up => match app.focus {
                Focus::Projects => {
                    if app.selected_project > 0 {
                        app.selected_project -= 1;
                        app.selected_todo = 0;
                    }
                }
                Focus::Todos => {
                    let count = app.current_todos().len();
                    if count > 0 && app.selected_todo > 0 {
                        app.selected_todo -= 1;
                    }
                }
            },
            KeyCode::Down => match app.focus {
                Focus::Projects => {
                    let count = app.data.project_count(app.show_empty);
                    if app.selected_project < count - 1 {
                        app.selected_project += 1;
                        app.selected_todo = 0;
                    }
                }
                Focus::Todos => {
                    let count = app.current_todos().len();
                    if count > 0 && app.selected_todo < count - 1 {
                        app.selected_todo += 1;
                    }
                }
            },
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if app.focus == Focus::Todos && !app.current_todos().is_empty() {
                    let path = app.current_todos()[app.selected_todo].path.clone();
                    return Ok(Some(path));
                }
            }
            KeyCode::Enter => {
                if app.focus == Focus::Todos && !app.current_todos().is_empty() {
                    app.detail_open = true;
                    app.detail_scroll = 0;
                }
            }
            KeyCode::Char('o') | KeyCode::Char('O') => {
                if !app.current_todos().is_empty() {
                    app.overview_open = true;
                }
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                app.sort_mode = app.sort_mode.next();
                app.resort();
            }
            KeyCode::Char('h') | KeyCode::Char('H') => {
                app.show_empty = !app.show_empty;
                let visible_count = app.data.visible_groups(app.show_empty).len();
                if app.selected_project > visible_count {
                    app.selected_project = visible_count;
                }
                app.selected_todo = 0;
            }
            _ => {}
        }
    }
    Ok(None)
}

fn main() -> io::Result<()> {
    let mut app = App::new()?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    while !app.quit {
        terminal
            .draw(|f| render(&app, f))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{}", e)))?;
        if let Some(open_path) = handle_events(&mut app)? {
            // Suspend tedlist
            disable_raw_mode()?;
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )?;
            terminal.show_cursor()?;

            // Launch tedtui and wait
            if let Some(tedtui_path) = find_tedtui() {
                let status = Command::new(&tedtui_path)
                    .arg(&open_path)
                    .stdin(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .status()?;
                if !status.success() {
                    eprintln!("tedtui exited with: {}", status);
                }
            } else {
                eprintln!("tedtui binary not found");
            }

            // Resume tedlist
            enable_raw_mode()?;
            execute!(
                terminal.backend_mut(),
                EnterAlternateScreen,
                EnableMouseCapture
            )?;
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
