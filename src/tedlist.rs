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
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List as TuiList, ListItem, Paragraph},
};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use theme::Theme;

// ============================================================================
// Data types
// ============================================================================

#[derive(Clone)]
struct TodoFile {
    path: PathBuf,
    parsed: parser::ParsedTodo,
    project_tag: Option<String>,
}

#[derive(Clone)]
struct PlanFile {
    path: PathBuf,
    name: String,
    content: String,
}

impl TodoFile {
    fn is_complete(&self) -> bool {
        !self.parsed.tasks.is_empty() && self.parsed.tasks.iter().all(|t| t.completed)
    }

    fn status_indicator(&self) -> &'static str {
        if self.is_complete() { "\u{2713}" } else { " " }
    }

    fn completion_summary(&self) -> Option<String> {
        if self.parsed.tasks.is_empty() {
            return None;
        }
        let done = self.parsed.tasks.iter().filter(|t| t.completed).count();
        Some(format!("{}/{}", done, self.parsed.tasks.len()))
    }
}

struct ProjectGroup {
    project: filestorage::Project,
    todos: Vec<TodoFile>,
}

struct TodoStore {
    groups: Vec<ProjectGroup>,
    unassigned: Vec<TodoFile>,
    all_todos: Vec<TodoFile>,
}

impl TodoStore {
    fn load() -> io::Result<Self> {
        let storage = FileStorage::new();
        let projects = storage.get_projects().unwrap_or_default();
        let todos_dir = storage.get_todos_dir();

        let mut groups: Vec<ProjectGroup> = projects
            .into_iter()
            .map(|p| ProjectGroup { project: p, todos: Vec::new() })
            .collect();

        let mut unassigned = Vec::new();

        if let Ok(entries) = fs::read_dir(todos_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("md") {
                    continue;
                }
                let Ok(parsed) = parse_markdown_file(&path) else { continue };
                let pid = parsed.project_id.clone();
                let project_tag = if pid.is_empty() {
                    None
                } else {
                    groups.iter().find(|g| g.project.id == pid).map(|g| g.project.name.clone())
                };
                let todo = TodoFile { path, parsed, project_tag };
                if pid.is_empty() {
                    unassigned.push(todo);
                } else if let Some(idx) = groups.iter().position(|g| g.project.id == pid) {
                    groups[idx].todos.push(todo);
                } else {
                    unassigned.push(todo);
                }
            }
        }

        sort_groups_alpha(&mut groups);
        for g in &mut groups {
            g.todos.sort_by(|a, b| a.parsed.name.cmp(&b.parsed.name));
        }
        unassigned.sort_by(|a, b| a.parsed.name.cmp(&b.parsed.name));

        let mut all_todos: Vec<TodoFile> = groups.iter().flat_map(|g| g.todos.iter().cloned()).collect();
        all_todos.extend(unassigned.iter().cloned());
        all_todos.sort_by(|a, b| a.parsed.name.cmp(&b.parsed.name));

        Ok(TodoStore { groups, unassigned, all_todos })
    }

    fn visible_groups(&self, show_empty_projects: bool) -> Vec<&ProjectGroup> {
        if show_empty_projects {
            self.groups.iter().collect()
        } else {
            self.groups.iter().filter(|g| !g.todos.is_empty()).collect()
        }
    }

    fn entries(&self, show_empty_projects: bool) -> Vec<(String, usize)> {
        let mut items = vec![("All Todos".to_string(), self.all_todos.len())];
        for g in self.visible_groups(show_empty_projects) {
            items.push((g.project.name.clone(), g.todos.len()));
        }
        if !self.unassigned.is_empty() {
            items.push(("Unassigned".to_string(), self.unassigned.len()));
        }
        items
    }

    fn entries_count(&self, show_empty_projects: bool) -> usize {
        1 + self.visible_groups(show_empty_projects).len() + if self.unassigned.is_empty() { 0 } else { 1 }
    }

    fn todos_for(&self, project_idx: usize, show_empty_projects: bool) -> &[TodoFile] {
        if project_idx == 0 {
            &self.all_todos
        } else if project_idx <= self.visible_groups(show_empty_projects).len() {
            &self.visible_groups(show_empty_projects)[project_idx - 1].todos
        } else {
            &self.unassigned
        }
    }

    fn name_for(&self, project_idx: usize, show_empty_projects: bool) -> &str {
        if project_idx == 0 {
            "All Todos"
        } else if project_idx <= self.visible_groups(show_empty_projects).len() {
            &self.visible_groups(show_empty_projects)[project_idx - 1].project.name
        } else {
            "Unassigned"
        }
    }
}

fn sort_groups_alpha(groups: &mut [ProjectGroup]) {
    groups.sort_by(|a, b| a.project.name.cmp(&b.project.name));
}

// ============================================================================
// App state
// ============================================================================

#[derive(Clone, Copy, PartialEq)]
enum PanelMode {
    Projects,
    Plans,
}

impl PanelMode {
    const ALL: &'static [PanelMode] = &[PanelMode::Projects, PanelMode::Plans];

    fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|m| *m == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }
}

#[derive(Clone, PartialEq)]
enum Focus {
    Sidebar,
    Content,
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
    store: TodoStore,
    plans: Vec<PlanFile>,
    panel_mode: PanelMode,
    focus: Focus,
    selected_left: usize,
    selected_project: usize,
    selected_todo: usize,
    selected_plan: usize,
    plan_scroll: usize,
    show_detail: bool,
    detail_scroll: usize,
    show_overview: bool,
    sort_mode: SortMode,
    show_empty_projects: bool,
    theme: Theme,
    quit: bool,
}

impl App {
    fn new() -> io::Result<Self> {
        Ok(App {
            store: TodoStore::load()?,
            plans: load_plans(),
            panel_mode: PanelMode::Projects,
            focus: Focus::Sidebar,
            selected_left: 0,
            selected_project: 0,
            selected_todo: 0,
            selected_plan: 0,
            plan_scroll: 0,
            show_detail: false,
            detail_scroll: 0,
            show_overview: false,
            sort_mode: SortMode::Alpha,
            show_empty_projects: false,
            theme: Theme::load(),
            quit: false,
        })
    }

    fn current_todos(&self) -> &[TodoFile] {
        self.store.todos_for(self.selected_project, self.show_empty_projects)
    }

    fn current_plan(&self) -> Option<&PlanFile> {
        self.plans.get(self.selected_plan)
    }

    fn resort(&mut self) {
        match self.sort_mode {
            SortMode::Alpha => sort_groups_alpha(&mut self.store.groups),
            SortMode::MostTodos => {
                self.store.groups.sort_by(|a, b| b.todos.len().cmp(&a.todos.len()));
            }
            SortMode::LeastTodos => {
                self.store.groups.sort_by(|a, b| a.todos.len().cmp(&b.todos.len()));
            }
        }
        let visible = self.store.visible_groups(self.show_empty_projects).len();
        if self.selected_project > visible {
            self.selected_project = visible;
        }
    }

    fn reload(&mut self) {
        self.show_detail = false;
        self.show_overview = false;
        let prev_project = self.store.name_for(self.selected_project, self.show_empty_projects).to_string();
        let prev_todo = self.current_todos().get(self.selected_todo).map(|t| t.parsed.name.clone());

        if let Ok(store) = TodoStore::load() {
            self.store = store;
        }
        self.plans = load_plans();
        self.resort();

        self.selected_project = self.store.entries(self.show_empty_projects)
            .iter()
            .position(|(n, _)| *n == prev_project)
            .unwrap_or(0);
        self.selected_todo = self.current_todos()
            .iter()
            .position(|t| Some(t.parsed.name.as_str()) == prev_todo.as_deref())
            .unwrap_or(0);
    }

    fn mode_items(&self, mode: PanelMode) -> usize {
        match mode {
            PanelMode::Projects => self.store.entries(self.show_empty_projects).len(),
            PanelMode::Plans => self.plans.len(),
        }
    }

    fn left_len(&self) -> usize {
        let mut n = 0;
        for mode in PanelMode::ALL {
            n += 1; // header
            if *mode == self.panel_mode {
                n += self.mode_items(*mode);
            }
        }
        n
    }

    fn is_header(&self, idx: usize) -> Option<PanelMode> {
        let mut i = 0;
        for mode in PanelMode::ALL {
            if idx == i { return Some(*mode) }
            i += 1;
            if *mode == self.panel_mode {
                i += self.mode_items(*mode);
            }
        }
        None
    }

    fn sub_index(&self, idx: usize) -> Option<usize> {
        let mut i = 0;
        for mode in PanelMode::ALL {
            i += 1; // header
            if *mode == self.panel_mode {
                let count = self.mode_items(*mode);
                if idx >= i && idx < i + count {
                    return Some(idx - i);
                }
                i += count;
            }
        }
        None
    }

    fn header_position(&self, mode: PanelMode) -> usize {
        let mut idx = 0;
        for m in PanelMode::ALL {
            if *m == mode { return idx }
            idx += 1;
            if *m == self.panel_mode {
                idx += self.mode_items(*m);
            }
        }
        0
    }

    fn select_left(&mut self, idx: usize) {
        if idx >= self.left_len() { return }
        if let Some(mode) = self.is_header(idx) {
            self.panel_mode = mode;
            self.selected_left = self.header_position(mode);
            return;
        }
        if let Some(sub) = self.sub_index(idx) {
            match self.panel_mode {
                PanelMode::Projects => {
                    self.selected_project = sub;
                    self.selected_todo = 0;
                }
                PanelMode::Plans => {
                    self.selected_plan = sub;
                    self.plan_scroll = 0;
                }
            }
        }
    }

    fn mode_sub_labels(&self, mode: PanelMode) -> Vec<String> {
        match mode {
            PanelMode::Projects => self.store.entries(self.show_empty_projects).iter().map(|(n, c)| format!("{} ({})", n, c)).collect(),
            PanelMode::Plans => self.plans.iter().map(|p| p.name.clone()).collect(),
        }
    }
}

fn load_plans() -> Vec<PlanFile> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let plans_dir = Path::new(&home).join(".ted").join("plans");
    let mut plans = Vec::new();
    if let Ok(entries) = fs::read_dir(plans_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }
            let name = entry.file_name().to_string_lossy().replace(".md", "");
            let content = fs::read_to_string(&path).unwrap_or_default();
            plans.push(PlanFile { path, name, content });
        }
    }
    plans.sort_by(|a, b| a.name.cmp(&b.name));
    plans
}

// ============================================================================
// Helpers
// ============================================================================

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    Rect { x, y, width, height }
}

fn push_text(lines: &mut Vec<Line>, label: &str, content: &str, bold: Style, normal: Style) {
    if content.is_empty() {
        return;
    }
    lines.push(Line::from(Span::styled(format!(" {}:", label), bold)));
    for line in content.lines() {
        lines.push(Line::from(Span::styled(format!(" {}", line), normal)));
    }
    lines.push(Line::from(""));
}

// ============================================================================
// Rendering
// ============================================================================

fn render(app: &App, f: &mut Frame) {
    let main = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .margin(1)
        .split(f.area());

    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(main[0]);

    render_left_panel(app, f, horiz[0]);
    render_right_panel(app, f, horiz[1]);
    render_help(app, f, main[1]);

    if app.show_detail {
        render_detail(app, f, f.area());
    }
    if app.show_overview {
        render_overview(app, f, f.area());
    }
}

fn left_items(app: &App) -> Vec<(String, bool)> {
    let mut items = Vec::new();
    let focused = app.focus == Focus::Sidebar;
    let sel = app.selected_left;
    let mut idx = 0usize;

    for mode in PanelMode::ALL {
        let expanded = *mode == app.panel_mode;
        let header = match mode {
            PanelMode::Projects => "Projects".to_string(),
            PanelMode::Plans => "Plans".to_string(),
        };
        items.push((format!("{} {}", if expanded { "\u{25bc}" } else { "\u{25b6}" }, header), idx == sel && focused));
        idx += 1;

        if expanded {
            for sub in app.mode_sub_labels(*mode) {
                items.push((format!("    {}", sub), idx == sel && focused));
                idx += 1;
            }
        }
    }

    items
}



fn render_left_panel(app: &App, f: &mut Frame, area: Rect) {
    let focused = app.focus == Focus::Sidebar;
    let border = if focused { app.theme.active_border_style() } else { app.theme.inactive_border_style() };

    let all_items = left_items(app);
    let list_items: Vec<ListItem> = all_items.iter().map(|(text, is_sel)| {
        let style = if *is_sel {
            app.theme.selected_style()
        } else if text.contains("Projects") || text.contains("Plans") {
            if (text.contains("Projects") && app.panel_mode == PanelMode::Projects)
                || (text.contains("Plans") && app.panel_mode == PanelMode::Plans)
            {
                Style::default().fg(Color::Rgb(255, 165, 0))
            } else {
                Style::default()
            }
        } else {
            Style::default()
        };
        ListItem::new(text.as_str()).style(style)
    }).collect();

    f.render_widget(TuiList::new(list_items).block(Block::default().borders(Borders::ALL).title(" Outline ").border_style(border)), area);
}

fn render_right_panel(app: &App, f: &mut Frame, area: Rect) {
    match app.panel_mode {
        PanelMode::Projects => render_todos_content(app, f, area),
        PanelMode::Plans => render_plans_content(app, f, area),
    }
}

fn render_todos_content(app: &App, f: &mut Frame, area: Rect) {
    let todos = app.current_todos();
    let name = app.store.name_for(app.selected_project, app.show_empty_projects);
    let title = format!(" {} ({}) ", name, todos.len());
    let right_active = app.focus == Focus::Content;
    let block = || Block::default().borders(Borders::ALL).title(title.as_str()).border_style(if right_active { app.theme.active_border_style() } else { app.theme.inactive_border_style() });

    if todos.is_empty() {
        f.render_widget(Paragraph::new(" No todos yet ").block(block()), area);
        return;
    }

    let show_tag = app.selected_project == 0;
    let items: Vec<ListItem> = todos.iter().enumerate().map(|(i, todo)| {
        let ch = todo.status_indicator();
        let check = if ch == "\u{2713}" { "\u{2713} " } else { "  " };
        let suffix = todo.completion_summary().map(|s| format!(" [{}]", s)).unwrap_or_default();
        let highlight = right_active && i == app.selected_todo;
        let line = if show_tag {
            let tag = todo.project_tag.as_deref().unwrap_or("?");
            let tag_padded = if tag.len() > 10 { let truncated: String = tag.chars().take(7).collect(); format!("{}...", truncated) } else { format!("{:<10}", tag) };
            Line::from(vec![Span::raw(" "), Span::styled(format!("[{}]", tag_padded), Style::default().fg(Color::Rgb(255, 165, 0))), Span::raw(format!(" {}{}", check, todo.parsed.name)), Span::raw(suffix)])
        } else {
            Line::from(format!(" {}{}{}", check, todo.parsed.name, suffix))
        };
        let style = if highlight { app.theme.selected_style() } else if todo.is_complete() { Style::default().fg(app.theme.task_completed) } else { Style::default() };
        ListItem::new(line).style(style)
    }).collect();
    f.render_widget(TuiList::new(items).block(block()), area);
}

fn render_plans_content(app: &App, f: &mut Frame, area: Rect) {
    let right_active = app.focus == Focus::Content;
    let border = if right_active { app.theme.active_border_style() } else { app.theme.inactive_border_style() };

    let Some(plan) = app.current_plan() else {
        f.render_widget(Paragraph::new(" Select a plan ").block(Block::default().borders(Borders::ALL).title(" Plan ").border_style(border)), area);
        return;
    };

    let lines: Vec<Line> = plan.content.lines().skip(app.plan_scroll).take((area.height - 2) as usize).map(|line| {
        if line.starts_with("# ") { Line::from(Span::styled(line.to_string(), Style::default().add_modifier(Modifier::BOLD))) }
        else if line.starts_with("## ") { Line::from(Span::styled(line.to_string(), Style::default().fg(Color::Rgb(255, 165, 0)))) }
        else if line.starts_with("- ") { Line::from(Span::styled(format!("  \u{2022} {}", &line[2..]), Style::default())) }
        else { Line::from(line.to_string()) }
    }).collect();
    f.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(format!(" {} ", plan.name)).border_style(border)).wrap(ratatui::widgets::Wrap { trim: false }),
        area,
    );
}

fn render_help(app: &App, f: &mut Frame, area: Rect) {
    let t = &app.theme;
    let k = |s| key_label(s, t);
    let para = Paragraph::new(Line::from(vec![
        k("Tab"), Span::raw(" Focus  "),
        k("p"), Span::raw(" Mode  "),
        k("\u{2191}\u{2193}"), Span::raw(" Nav  "),
        k("Enter"), Span::raw(" Detail  "),
        k("Ctrl+E"), Span::raw(" Edit  "),
        k("Ctrl+N"), Span::raw(" New  "),
        k("o"), Span::raw(" Ovw  "),
        k("s"), Span::raw(" Sort  "),
        k("h"), Span::raw(" Hide  "),
        k("q"), Span::raw(" / "),
        k("Esc"), Span::raw(" Quit"),
    ]))
    .block(Block::default().borders(Borders::ALL).title(" Help "));
    f.render_widget(para, area);
}

fn key_label(text: &str, t: &Theme) -> Span<'static> {
    Span::styled(text.to_string(), Style::default().fg(t.help_key))
}

fn render_detail(app: &App, f: &mut Frame, area: Rect) {
    let todos = app.current_todos();
    if todos.is_empty() {
        return;
    }
    let todo = &todos[app.selected_todo];

    let popup = centered_rect(area, area.width.saturating_sub(8).min(100), area.height.saturating_sub(4).min(40));
    f.render_widget(Clear, popup);

    let t = &app.theme;
    let bold = Style::default().fg(t.field_text).add_modifier(Modifier::BOLD);
    let normal = Style::default().fg(t.field_text);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![Span::raw(" "), Span::styled("Name: ", bold), Span::styled(&todo.parsed.name, normal)]));
    lines.push(Line::from(""));
    push_text(&mut lines, "Project", &todo.parsed.project_id, bold, normal);
    push_text(&mut lines, "Info", &todo.parsed.info, bold, normal);
    push_text(&mut lines, "Goal", &todo.parsed.goal, bold, normal);

    let done = todo.parsed.tasks.iter().filter(|t| t.completed).count();
    lines.push(Line::from(Span::styled(format!(" Tasks ({}/{}):", done, todo.parsed.tasks.len()), bold)));
    for task in &todo.parsed.tasks {
        let ch = if task.completed { "\u{2713}" } else { " " };
        let style = if task.completed { Style::default().fg(t.task_completed) } else { normal };
        lines.push(Line::from(Span::styled(format!("   [{}] {}", ch, task.text), style)));
    }
    lines.push(Line::from(""));

    push_text(&mut lines, "Note", &todo.parsed.note, bold, normal);

    if !todo.parsed.history.is_empty() {
        lines.push(Line::from(Span::styled(" History:", bold)));
        for line in todo.parsed.history.lines().filter(|l| !l.trim().is_empty()) {
            lines.push(Line::from(Span::styled(format!(" {}", line), Style::default().fg(t.history_entry))));
        }
    }

    let visible: Vec<Line> = lines.iter().skip(app.detail_scroll).take((popup.height - 2) as usize).cloned().collect();
    f.render_widget(
        Paragraph::new(visible)
            .block(Block::default().borders(Borders::ALL).title(format!(" Detail {} (Esc close, \u{2191}\u{2193} scroll) ", todo.parsed.name)).border_style(Style::default().fg(t.active_border)))
            .style(Style::default().bg(t.popup_bg)),
        popup,
    );
}

fn render_overview(app: &App, f: &mut Frame, area: Rect) {
    let todos = app.current_todos();
    let popup = centered_rect(area, area.width.saturating_sub(16).min(80).max(40), 16);
    f.render_widget(Clear, popup);

    let t = &app.theme;
    let bold = Style::default().fg(t.popup_text).add_modifier(Modifier::BOLD);
    let normal = Style::default().fg(t.popup_text);

    let mut completed = 0usize;
    let mut in_progress = 0usize;
    let mut no_tasks = 0usize;
    let mut total_tasks = 0usize;
    let mut done_tasks = 0usize;

    for todo in todos {
        if todo.parsed.tasks.is_empty() {
            no_tasks += 1;
        } else if todo.is_complete() {
            completed += 1;
        } else {
            in_progress += 1;
        }
        total_tasks += todo.parsed.tasks.len();
        done_tasks += todo.parsed.tasks.iter().filter(|t| t.completed).count();
    }

    let task_summary = if total_tasks > 0 { format!("{}/{}", done_tasks, total_tasks) } else { "n/a".to_string() };
    let name = app.store.name_for(app.selected_project, app.show_empty_projects);
    let green = Style::default().fg(t.status_success);
    let yellow = Style::default().fg(t.status_info);

    let lines = vec![
        Line::from(""),
        Line::from(vec![Span::raw("  "), Span::styled(name, bold)]),
        Line::from(""),
        Line::from(vec![Span::raw("  Total todos:     "), Span::styled(todos.len().to_string(), bold)]),
        Line::from(vec![Span::raw("  Completed:       "), Span::styled(completed.to_string(), green)]),
        Line::from(vec![Span::raw("  In progress:     "), Span::styled(in_progress.to_string(), yellow)]),
        Line::from(vec![Span::raw("  No tasks:        "), Span::styled(no_tasks.to_string(), normal)]),
        Line::from(vec![Span::raw("  Tasks done/total: "), Span::styled(task_summary, bold)]),
        Line::from(""),
        Line::from(Span::styled("  Press any key to close", Style::default().fg(t.popup_hint))),
    ];

    f.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(" Overview ").border_style(Style::default().fg(t.project_border)))
            .style(t.popup_bg_style()),
        popup,
    );
}

// ============================================================================
// Editor / tedtui binary lookup
// ============================================================================

fn find_editor() -> PathBuf {
    std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("nvim"))
}

fn find_tedtui() -> Option<PathBuf> {
    let try_path = |p: PathBuf| if p.is_file() { Some(p) } else { None };

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if let Some(found) = try_path(dir.join("tedtui")) {
                return Some(found);
            }
            #[cfg(windows)]
            if let Some(found) = try_path(dir.join("tedtui.exe")) {
                return Some(found);
            }
        }
    }
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            if let Some(found) = try_path(dir.join("tedtui")) {
                return Some(found);
            }
            #[cfg(windows)]
            if let Some(found) = try_path(dir.join("tedtui.exe")) {
                return Some(found);
            }
        }
    }
    None
}

// ============================================================================
// Event handling
// ============================================================================

enum Action {
    None,
    OpenEditor(PathBuf),
    NewTodo,
}

fn handle_events(app: &mut App) -> io::Result<Action> {
    let Event::Key(key) = event::read()? else { return Ok(Action::None) };

    if app.show_detail {
        handle_detail_key(app, key.code);
        return Ok(Action::None);
    }
    if app.show_overview {
        app.show_overview = false;
        return Ok(Action::None);
    }
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => app.quit = true,
        KeyCode::Char('q') | KeyCode::Esc => app.quit = true,
        KeyCode::Tab | KeyCode::BackTab => {
            app.focus = match app.focus {
                Focus::Sidebar => Focus::Content,
                Focus::Content => Focus::Sidebar,
            };
        }
        KeyCode::Char('p') => {
            app.panel_mode = app.panel_mode.next();
        }
        KeyCode::Up => {
            if app.focus == Focus::Sidebar && app.selected_left > 0 {
                app.selected_left -= 1;
                app.select_left(app.selected_left);
            } else if app.focus == Focus::Content && app.selected_todo > 0 {
                app.selected_todo -= 1;
            }
        }
        KeyCode::Down => {
            if app.focus == Focus::Sidebar {
                let max = app.left_len();
                if app.selected_left + 1 < max {
                    app.selected_left += 1;
                    app.select_left(app.selected_left);
                }
            } else if app.focus == Focus::Content {
                let max = app.current_todos().len();
                if app.selected_todo + 1 < max {
                    app.selected_todo += 1;
                }
            }
        }
        KeyCode::Enter | KeyCode::Right => {
            if app.focus == Focus::Sidebar {
                app.select_left(app.selected_left);
                app.focus = Focus::Content;
            } else if !app.current_todos().is_empty() {
                app.show_detail = true;
                app.detail_scroll = 0;
            }
        }
        KeyCode::Left => {
            if app.focus == Focus::Content {
                app.focus = Focus::Sidebar;
            }
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return Ok(open_in_editor(app));
        }
        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return Ok(Action::NewTodo);
        }
        KeyCode::Char('o') | KeyCode::Char('O') => {
            if app.panel_mode == PanelMode::Projects && !app.current_todos().is_empty() {
                app.show_overview = true;
            }
        }
        KeyCode::Char('s') | KeyCode::Char('S') => {
            app.sort_mode = app.sort_mode.next();
            app.resort();
        }
        KeyCode::Char('h') | KeyCode::Char('H') => toggle_empty(app),
        _ => {}
    }
    Ok(Action::None)
}

fn handle_detail_key(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => { app.show_detail = false; app.detail_scroll = 0; }
        KeyCode::Up => app.detail_scroll = app.detail_scroll.saturating_sub(1),
        KeyCode::Down => app.detail_scroll += 1,
        _ => {}
    }
}

fn toggle_empty(app: &mut App) {
    app.show_empty_projects = !app.show_empty_projects;
    let visible = app.store.visible_groups(app.show_empty_projects).len();
    if app.selected_project > visible {
        app.selected_project = visible;
    }
    app.selected_todo = 0;
}

fn open_in_editor(app: &App) -> Action {
    match app.panel_mode {
        PanelMode::Projects => {
            app.current_todos().get(app.selected_todo).map(|t| Action::OpenEditor(t.path.clone())).unwrap_or(Action::None)
        }
        PanelMode::Plans => {
            app.current_plan().map(|p| Action::OpenEditor(p.path.clone())).unwrap_or(Action::None)
        }
    }
}

// ============================================================================
// Terminal suspend/resume for launching tedtui
// ============================================================================

fn suspend_for_editor(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>, path: &Path) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    let editor = find_editor();
    let status = Command::new(&editor)
        .arg(path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    if !status.success() {
        eprintln!("editor exited with: {}", status);
    }

    enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen, EnableMouseCapture)?;
    terminal.clear()?;
    Ok(())
}

fn suspend_for_new_todo(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    if let Some(tedtui) = find_tedtui() {
        let status = Command::new(&tedtui)
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

    enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen, EnableMouseCapture)?;
    terminal.clear()?;
    Ok(())
}

// ============================================================================
// Entry point
// ============================================================================

fn main() -> io::Result<()> {
    let mut app = App::new()?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    while !app.quit {
        terminal
            .draw(|f| render(&app, f))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{}", e)))?;
        match handle_events(&mut app)? {
            Action::OpenEditor(path) => {
                suspend_for_editor(&mut terminal, &path)?;
                app.reload();
            }
            Action::NewTodo => {
                suspend_for_new_todo(&mut terminal)?;
                app.reload();
            }
            Action::None => {}
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}
