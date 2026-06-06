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
    widgets::{Block, Borders, Clear, List as TuiList, ListItem, ListState, Paragraph},
};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use serde_json;
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

impl TodoFile {
    fn is_complete(&self) -> bool {
        !self.parsed.tasks.is_empty() && self.parsed.tasks.iter().all(|t| t.completed)
    }

    fn status_indicator(&self) -> &'static str {
        if self.is_complete() { "\u{2713}" } else { " " }
    }

    fn completion_summary(&self) -> Option<String> {
        if self.parsed.tasks.is_empty() { return None }
        let done = self.parsed.tasks.iter().filter(|t| t.completed).count();
        Some(format!("{}/{}", done, self.parsed.tasks.len()))
    }
}

#[derive(Clone)]
struct PlanFile {
    path: PathBuf,
    name: String,
    content: String,
}

#[derive(Clone)]
struct InboxFile {
    path: PathBuf,
    name: String,
    content: String,
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

        if let Ok(entries) = collect_md_files(&todos_dir) {
            for path in entries {
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
        for g in &mut groups { g.todos.sort_by(|a, b| a.parsed.name.cmp(&b.parsed.name)) }
        unassigned.sort_by(|a, b| a.parsed.name.cmp(&b.parsed.name));

        let mut all_todos: Vec<TodoFile> = groups.iter().flat_map(|g| g.todos.iter().cloned()).collect();
        all_todos.extend(unassigned.iter().cloned());
        all_todos.sort_by(|a, b| a.parsed.name.cmp(&b.parsed.name));

        Ok(TodoStore { groups, unassigned, all_todos })
    }

    fn visible_groups(&self, show_empty_projects: bool) -> Vec<&ProjectGroup> {
        if show_empty_projects { self.groups.iter().collect() } else { self.groups.iter().filter(|g| !g.todos.is_empty()).collect() }
    }

    fn entries(&self, show_empty_projects: bool) -> Vec<(String, usize)> {
        let mut items = vec![("All Todos".to_string(), self.all_todos.len())];
        for g in self.visible_groups(show_empty_projects) {
            items.push((g.project.name.clone(), g.todos.len()));
        }
        if !self.unassigned.is_empty() { items.push(("Unassigned".to_string(), self.unassigned.len())) }
        items
    }

    fn todos_for(&self, project_idx: usize, show_empty_projects: bool) -> &[TodoFile] {
        if project_idx == 0 { &self.all_todos }
        else if project_idx <= self.visible_groups(show_empty_projects).len() { &self.visible_groups(show_empty_projects)[project_idx - 1].todos }
        else { &self.unassigned }
    }

    fn name_for(&self, project_idx: usize, show_empty_projects: bool) -> &str {
        if project_idx == 0 { "All Todos" }
        else if project_idx <= self.visible_groups(show_empty_projects).len() { &self.visible_groups(show_empty_projects)[project_idx - 1].project.name }
        else { "Unassigned" }
    }
}

fn sort_groups_alpha(groups: &mut [ProjectGroup]) {
    groups.sort_by(|a, b| a.project.name.cmp(&b.project.name));
}

struct BacklogStore {
    groups: Vec<ProjectGroup>,
    unassigned: Vec<TodoFile>,
    all: Vec<TodoFile>,
}

impl BacklogStore {
    fn load() -> Self {
        let storage = filestorage::FileStorage::new();
        let projects = storage.get_projects().unwrap_or_default();
        let mut groups: Vec<ProjectGroup> = projects.into_iter()
            .map(|p| ProjectGroup { project: p, todos: Vec::new() })
            .collect();
        let mut unassigned = Vec::new();
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let dir = Path::new(&home).join(".ted").join("backlog");
        let mut all = Vec::new();
        if let Ok(paths) = collect_md_files(&dir) {
            for path in &paths {
                if let Ok(parsed) = parse_markdown_file(&path) {
                    let pid = parsed.project_id.clone();
                    let project_tag = if pid.is_empty() {
                        None
                    } else {
                        groups.iter().find(|g| g.project.id == pid).map(|g| g.project.name.clone())
                    };
                    let todo = TodoFile { path: path.clone(), parsed, project_tag };
                    all.push(todo.clone());
                    if pid.is_empty() {
                        unassigned.push(todo);
                    } else if let Some(idx) = groups.iter().position(|g| g.project.id == pid) {
                        groups[idx].todos.push(todo);
                    } else {
                        unassigned.push(todo);
                    }
                }
            }
        }
        for g in &mut groups { g.todos.sort_by(|a, b| a.parsed.name.cmp(&b.parsed.name)) }
        unassigned.sort_by(|a, b| a.parsed.name.cmp(&b.parsed.name));
        all.sort_by(|a, b| a.parsed.name.cmp(&b.parsed.name));
        BacklogStore { groups, unassigned, all }
    }

    fn entries(&self) -> Vec<(String, usize)> {
        let mut items = vec![("Backlog".to_string(), self.all.len())];
        for g in &self.groups {
            items.push((g.project.name.clone(), g.todos.len()));
        }
        if !self.unassigned.is_empty() { items.push(("Unassigned".to_string(), self.unassigned.len())) }
        items
    }

    fn todos_for(&self, idx: usize) -> &[TodoFile] {
        if idx == 0 { &self.all }
        else if idx <= self.groups.len() { &self.groups[idx - 1].todos }
        else { &self.unassigned }
    }

    fn name_for(&self, idx: usize) -> &str {
        if idx == 0 { "Backlog" }
        else if idx <= self.groups.len() { &self.groups[idx - 1].project.name }
        else { "Unassigned" }
    }
}

// ============================================================================
// App state
// ============================================================================

type PanelIdx = usize;
const PROJ: PanelIdx = 0;
const PLANS: PanelIdx = 1;
const INBOX: PanelIdx = 2;
const BACKLOG: PanelIdx = 3;
const PANEL_COUNT: PanelIdx = 4;
const PANEL_NAMES: &[&str] = &["Projects", "Plans", "Inbox", "Backlog"];

#[derive(Clone, Copy)]
struct PanelState {
    sidx: usize,
    cidx: usize,
}

impl PanelState {
    fn new() -> Self { PanelState { sidx: 0, cidx: 0 } }
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

}

// ============================================================================
// Per-view actions
// ============================================================================

type ActionFn = fn(&mut App) -> Option<ViewAction>;

#[derive(Clone)]
enum ViewAction {
    None,
    OpenEditor(PathBuf),
    NewTodo,
    OpenInTedtui(InboxFile),
    EditInTedtui(PathBuf),
    DeleteFile(PathBuf),
    CompleteFile(PathBuf),
    RunBackground(String),
    RunObsidian(String),
    MoveFile(PathBuf, PathBuf),
    Action(ActionFn),
}

fn help_keys(app: &App) -> Vec<(&'static str, &'static str)> {
    match app.panel_mode {
        PROJ => vec![
            ("Enter", "Detail"),
            ("Ctrl+E", "Edit"),
            ("Ctrl+N", "New"),
            ("Ctrl+D", "Done"),
            ("Ctrl+B", "Bklog"),
            ("o", "Ovw"),
            ("s", "Sort"),
            ("h", "Hide"),
        ],
        PLANS => vec![
            ("Ctrl+E", "Edit"),
        ],
        INBOX => vec![
            ("Ctrl+E", "Open in tedtui"),
            ("d", "Delete"),
            ("u", "Run inbox"),
            ("o", "Obsidian"),
        ],
        BACKLOG => vec![
            ("Enter", "Detail"),
            ("Ctrl+E", "Edit"),
            ("Ctrl+B", "Todos"),
        ],
        _ => vec![],
    }
}

fn overview_action(app: &mut App) -> Option<ViewAction> {
    if app.filtered_content_len() > 0 { app.show_overview = true }
    None
}

fn sort_action(app: &mut App) -> Option<ViewAction> {
    app.sort_mode = app.sort_mode.next();
    app.resort();
    None
}

fn toggle_empty_action(app: &mut App) -> Option<ViewAction> {
    toggle_empty(app);
    None
}

fn inbox_delete_action(app: &mut App) -> Option<ViewAction> {
    if app.focus == Focus::Content {
        if let Some(file) = app.current_inbox() {
            app.confirm_delete = Some(file.path.clone());
        }
    }
    None
}

fn inbox_update_action(_app: &mut App) -> Option<ViewAction> {
    Some(ViewAction::RunBackground("ted inbox".to_string()))
}

fn inbox_obsidian_action(app: &mut App) -> Option<ViewAction> {
    let file = app.current_inbox()?;
    let name = file.content.lines()
        .find(|l| l.starts_with("# "))
        .map(|l| l.trim_start_matches("# ").trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| file.name.clone());

    let escaped = file.content
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`");

    Some(ViewAction::RunObsidian(format!(
        "obsidian vault=\"TomtomsVault\" create name=\"{}\" content=\"{}\" path=\"Zettelkasten/\" open", name, escaped
    )))
}

// ============================================================================
// Global key bindings (always active regardless of panel)
// ============================================================================

fn quit_action(app: &mut App) -> Option<ViewAction> { app.quit = true; None }

fn toggle_focus_action(app: &mut App) -> Option<ViewAction> {
    app.focus = if app.focus == Focus::Sidebar { Focus::Content } else { Focus::Sidebar };
    None
}

fn cycle_panel_action(app: &mut App) -> Option<ViewAction> {
    app.panel_mode = (app.panel_mode + 1) % PANEL_COUNT;
    app.selected_left = app.header_position(app.panel_mode);
    None
}

fn global_up_action(app: &mut App) -> Option<ViewAction> {
    if app.focus == Focus::Sidebar && app.selected_left > 0 {
        app.selected_left -= 1; app.select_left(app.selected_left);
    } else if app.focus == Focus::Content {
        let p = app.panel_mode;
        if p == PLANS || p == INBOX { app.p().cidx = app.pi().cidx.saturating_sub(1) }
        else if app.pi().cidx > 0 { app.p().cidx -= 1 }
    }
    None
}

fn global_down_action(app: &mut App) -> Option<ViewAction> {
    if app.focus == Focus::Sidebar {
        let max = app.left_len();
        if app.selected_left + 1 < max { app.selected_left += 1; app.select_left(app.selected_left) }
    } else if app.focus == Focus::Content {
        let p = app.panel_mode;
        if p == PLANS || p == INBOX { app.p().cidx += 1 }
        else if app.pi().cidx + 1 < app.filtered_content_len() { app.p().cidx += 1 }
    }
    None
}

fn global_enter_action(app: &mut App) -> Option<ViewAction> {
    if app.focus == Focus::Sidebar { app.select_left(app.selected_left); app.focus = Focus::Content }
    else if (app.panel_mode == PROJ || app.panel_mode == BACKLOG) && app.filtered_content_len() > 0 {
        app.show_detail = true; app.detail_scroll = 0;
    }
    None
}

fn global_left_action(app: &mut App) -> Option<ViewAction> {
    if app.focus == Focus::Content { app.focus = Focus::Sidebar }
    None
}

fn global_edit_action(app: &mut App) -> Option<ViewAction> {
    match app.panel_mode {
        PROJ => app.current_todos().get(app.real_content_idx()).map(|t| ViewAction::EditInTedtui(t.path.clone())),
        PLANS => app.current_plan().map(|p| ViewAction::OpenEditor(p.path.clone())),
        INBOX => app.current_inbox().map(|f| ViewAction::OpenInTedtui(f.clone())),
        BACKLOG => app.current_todos().get(app.real_content_idx()).map(|t| ViewAction::EditInTedtui(t.path.clone())),
        _ => None,
    }
}

fn global_new_todo_action(_app: &mut App) -> Option<ViewAction> {
    Some(ViewAction::NewTodo)
}

fn complete_todo_action(app: &mut App) -> Option<ViewAction> {
    if app.panel_mode != PROJ || app.focus != Focus::Content { return None }
    let todo = app.current_todos().get(app.real_content_idx())?;
    if todo.parsed.tasks.iter().all(|t| t.completed) {
        Some(ViewAction::CompleteFile(todo.path.clone()))
    } else {
        app.confirm_complete = Some(todo.path.clone());
        None
    }
}

fn move_to_backlog_action(app: &mut App) -> Option<ViewAction> {
    if app.focus != Focus::Content { return None }
    if app.panel_mode != PROJ && app.panel_mode != BACKLOG { return None }
    let todo = app.current_todos().get(app.real_content_idx())?;
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let is_backlog = app.panel_mode == BACKLOG;
    let target_dir = Path::new(&home).join(".ted").join(if is_backlog { "todos" } else { "backlog" });
    Some(ViewAction::MoveFile(todo.path.clone(), target_dir))
}

fn search_action(app: &mut App) -> Option<ViewAction> {
    app.search_active = true;
    app.search_typing = true;
    None
}

fn global_bindings() -> Vec<(KeyCode, KeyModifiers, ActionFn)> {
    vec![
        (KeyCode::Char('q'), KeyModifiers::NONE, quit_action),
        (KeyCode::Esc, KeyModifiers::NONE, quit_action),
        (KeyCode::Char('c'), KeyModifiers::CONTROL, quit_action),
        (KeyCode::Tab, KeyModifiers::NONE, toggle_focus_action),
        (KeyCode::BackTab, KeyModifiers::NONE, toggle_focus_action),
        (KeyCode::Char('p'), KeyModifiers::NONE, cycle_panel_action),
        (KeyCode::Up, KeyModifiers::NONE, global_up_action),
        (KeyCode::Down, KeyModifiers::NONE, global_down_action),
        (KeyCode::Enter, KeyModifiers::NONE, global_enter_action),
        (KeyCode::Right, KeyModifiers::NONE, global_enter_action),
        (KeyCode::Left, KeyModifiers::NONE, global_left_action),
        (KeyCode::Char('e'), KeyModifiers::CONTROL, global_edit_action),
        (KeyCode::Char('n'), KeyModifiers::CONTROL, global_new_todo_action),
        (KeyCode::Char('d'), KeyModifiers::CONTROL, complete_todo_action),
        (KeyCode::Char('b'), KeyModifiers::CONTROL, move_to_backlog_action),
        (KeyCode::Char('/'), KeyModifiers::NONE, search_action),
        (KeyCode::Char('f'), KeyModifiers::CONTROL, search_action),
    ]
}

// ============================================================================
// Panel-specific key bindings
// ============================================================================

fn panel_actions(app: &App) -> Vec<(KeyCode, ViewAction)> {
    match app.panel_mode {
        PROJ => vec![
            (KeyCode::Char('o'), ViewAction::Action(overview_action)),
            (KeyCode::Char('O'), ViewAction::Action(overview_action)),
            (KeyCode::Char('s'), ViewAction::Action(sort_action)),
            (KeyCode::Char('S'), ViewAction::Action(sort_action)),
            (KeyCode::Char('h'), ViewAction::Action(toggle_empty_action)),
            (KeyCode::Char('H'), ViewAction::Action(toggle_empty_action)),
        ],
        PLANS => vec![],
        INBOX => vec![
            (KeyCode::Char('d'), ViewAction::Action(inbox_delete_action)),
            (KeyCode::Char('D'), ViewAction::Action(inbox_delete_action)),
            (KeyCode::Char('u'), ViewAction::Action(inbox_update_action)),
            (KeyCode::Char('U'), ViewAction::Action(inbox_update_action)),
            (KeyCode::Char('o'), ViewAction::Action(inbox_obsidian_action)),
            (KeyCode::Char('O'), ViewAction::Action(inbox_obsidian_action)),
        ],
        BACKLOG => vec![],
        _ => vec![],
    }
}

// ============================================================================
// App state
// ============================================================================

struct App {
    store: TodoStore,
    backlog_store: BacklogStore,
    plans: Vec<PlanFile>,
    inbox_files: Vec<InboxFile>,
    panels: [PanelState; PANEL_COUNT],
    panel_mode: PanelIdx,
    focus: Focus,
    selected_left: usize,
    confirm_delete: Option<PathBuf>,
    confirm_complete: Option<PathBuf>,
    show_detail: bool,
    detail_scroll: usize,
    show_overview: bool,
    sort_mode: SortMode,
    show_empty_projects: bool,
    todo_list_state: ListState,
    search_query: String,
    search_active: bool,
    search_typing: bool,
    theme: Theme,
    quit: bool,
}

impl App {
    fn new() -> io::Result<Self> {
        Ok(App {
            store: TodoStore::load()?,
            backlog_store: BacklogStore::load(),
            plans: load_plans(),
            inbox_files: load_inbox(),
            panels: [PanelState::new(); PANEL_COUNT],
            panel_mode: PROJ,
            focus: Focus::Sidebar,
            selected_left: 0,
            confirm_delete: None,
            confirm_complete: None,
            show_detail: false,
            detail_scroll: 0,
            show_overview: false,
            sort_mode: SortMode::Alpha,
            show_empty_projects: false,
            todo_list_state: ListState::default(),
            search_query: String::new(),
            search_active: false,
            search_typing: false,
            theme: Theme::load(),
            quit: false,
        })
    }

    fn p(&mut self) -> &mut PanelState { &mut self.panels[self.panel_mode] }
    fn pi(&self) -> &PanelState { &self.panels[self.panel_mode] }

    fn real_sidx(&self, mode: PanelIdx) -> usize {
        if !self.searching_sidebar() || mode != self.panel_mode { return self.panels[mode].sidx }
        match mode {
            PROJ => {
                let entries = self.store.entries(self.show_empty_projects);
                let filtered: Vec<usize> = entries.iter().enumerate()
                    .filter(|(_, (n, _))| self.search_matches(n)).map(|(i, _)| i).collect();
                filtered.get(self.panels[PROJ].sidx).copied().unwrap_or(0)
            },
            PLANS => {
                let filtered: Vec<usize> = self.plans.iter().enumerate()
                    .filter(|(_, p)| self.search_matches(&p.name) || self.search_matches(&p.content))
                    .map(|(i, _)| i).collect();
                filtered.get(self.panels[PLANS].sidx).copied().unwrap_or(0)
            },
            INBOX => {
                let filtered: Vec<usize> = self.inbox_files.iter().enumerate()
                    .filter(|(_, f)| self.search_matches(&f.name) || self.search_matches(&f.content))
                    .map(|(i, _)| i).collect();
                filtered.get(self.panels[INBOX].sidx).copied().unwrap_or(0)
            },
            BACKLOG => {
                let entries = self.backlog_store.entries();
                let filtered: Vec<usize> = entries.iter().enumerate()
                    .filter(|(_, (n, _))| self.search_matches(n)).map(|(i, _)| i).collect();
                filtered.get(self.panels[BACKLOG].sidx).copied().unwrap_or(0)
            },
            _ => self.panels[mode].sidx,
        }
    }

    fn current_todos(&self) -> &[TodoFile] {
        if self.panel_mode == BACKLOG {
            self.backlog_store.todos_for(self.real_sidx(BACKLOG))
        } else {
            self.store.todos_for(self.real_sidx(PROJ), self.show_empty_projects)
        }
    }
    fn current_name(&self) -> &str {
        if self.panel_mode == BACKLOG {
            self.backlog_store.name_for(self.real_sidx(BACKLOG))
        } else {
            self.store.name_for(self.real_sidx(PROJ), self.show_empty_projects)
        }
    }
    fn current_plan(&self) -> Option<&PlanFile> { self.plans.get(self.real_sidx(PLANS)) }
    fn current_inbox(&self) -> Option<&InboxFile> { self.inbox_files.get(self.real_sidx(INBOX)) }

    fn search_matches(&self, text: &str) -> bool {
        if !self.search_active || self.search_query.is_empty() { return true }
        let lower = text.to_lowercase();
        let q_lower = self.search_query.to_lowercase();
        let mut chars = q_lower.chars();
        let Some(mut c) = chars.next() else { return true };
        for ch in lower.chars() {
            if ch == c {
                c = match chars.next() { Some(next) => next, None => return true };
            }
        }
        false
    }

    fn todo_matches_search(&self, t: &TodoFile) -> bool {
        let q = &self.search_query;
        !self.searching_content() || q.is_empty()
        || self.search_matches(&t.parsed.name)
        || self.search_matches(&t.parsed.project_id)
        || self.search_matches(&t.parsed.info)
        || self.search_matches(&t.parsed.goal)
        || t.parsed.tasks.iter().any(|task| self.search_matches(&task.text))
        || self.search_matches(&t.parsed.note)
        || self.search_matches(&t.parsed.history)
    }

    fn real_content_idx(&self) -> usize {
        let cidx = self.pi().cidx;
        if !self.searching_content() { return cidx }
        self.current_todos().iter()
            .enumerate()
            .filter(|(_, t)| self.todo_matches_search(t))
            .nth(cidx)
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    fn filtered_content_len(&self) -> usize {
        if !self.searching_content() { return self.current_todos().len() }
        self.current_todos().iter().filter(|t| self.todo_matches_search(t)).count()
    }

    fn resort(&mut self) {
        match self.sort_mode {
            SortMode::Alpha => sort_groups_alpha(&mut self.store.groups),
            SortMode::MostTodos => { self.store.groups.sort_by(|a, b| b.todos.len().cmp(&a.todos.len())); }
            SortMode::LeastTodos => { self.store.groups.sort_by(|a, b| a.todos.len().cmp(&b.todos.len())); }
        }
        let visible = self.store.visible_groups(self.show_empty_projects).len();
        if self.panels[PROJ].sidx > visible { self.panels[PROJ].sidx = visible }
    }

    fn reload(&mut self) {
        self.show_detail = false;
        self.show_overview = false;
        let prev_project = self.store.name_for(self.panels[PROJ].sidx, self.show_empty_projects).to_string();
        let cidx = self.pi().cidx;
        let prev_todo = self.current_todos().get(self.real_content_idx()).map(|t| t.parsed.name.clone());
        if let Ok(store) = TodoStore::load() { self.store = store }
        self.backlog_store = BacklogStore::load();
        self.plans = load_plans();
        self.inbox_files = load_inbox();
        self.resort();
        self.panels[PROJ].sidx = self.store.entries(self.show_empty_projects).iter().position(|(n, _)| *n == prev_project).unwrap_or(0);
        self.p().cidx = self.current_todos().iter().position(|t| Some(t.parsed.name.as_str()) == prev_todo.as_deref()).unwrap_or_else(|| cidx.min(self.current_todos().len().saturating_sub(1)));
    }

    fn searching_sidebar(&self) -> bool {
        self.focus == Focus::Sidebar && self.search_active && !self.search_query.is_empty()
    }
    fn searching_content(&self) -> bool {
        self.focus == Focus::Content && self.search_active && !self.search_query.is_empty()
    }

    fn mode_items(&self, mode: PanelIdx) -> usize {
        let sidebar_searching = self.searching_sidebar() && mode == self.panel_mode;
        match mode {
            PROJ => {
                let all = self.store.entries(self.show_empty_projects);
                if sidebar_searching { all.iter().filter(|(n, _)| self.search_matches(n)).count() }
                else { all.len() }
            },
            PLANS => {
                if sidebar_searching {
                    self.plans.iter().filter(|p| self.search_matches(&p.name) || self.search_matches(&p.content)).count()
                } else { self.plans.len() }
            },
            INBOX => {
                if sidebar_searching {
                    self.inbox_files.iter().filter(|f| self.search_matches(&f.name) || self.search_matches(&f.content)).count()
                } else { self.inbox_files.len() }
            },
            BACKLOG => {
                let all = self.backlog_store.entries();
                if sidebar_searching { all.iter().filter(|(n, _)| self.search_matches(n)).count() }
                else { all.len() }
            },
            _ => 0,
        }
    }

    fn left_len(&self) -> usize {
        let mut n = 0;
        for m in 0..PANEL_COUNT { n += 1; if m == self.panel_mode { n += self.mode_items(m) } }
        n
    }

    fn is_header(&self, idx: usize) -> Option<PanelIdx> {
        let mut i = 0;
        for m in 0..PANEL_COUNT {
            if idx == i { return Some(m) }
            i += 1;
            if m == self.panel_mode { i += self.mode_items(m) }
        }
        None
    }

    fn sub_index(&self, idx: usize) -> Option<usize> {
        let mut i = 0;
        for m in 0..PANEL_COUNT {
            i += 1;
            if m == self.panel_mode {
                let count = self.mode_items(m);
                if idx >= i && idx < i + count { return Some(idx - i) }
                i += count;
            }
        }
        None
    }

    fn header_position(&self, mode: PanelIdx) -> usize {
        let mut idx = 0;
        for m in 0..PANEL_COUNT {
            if m == mode { return idx }
            idx += 1;
            if m == self.panel_mode { idx += self.mode_items(m) }
        }
        0
    }

    fn select_left(&mut self, idx: usize) {
        if idx >= self.left_len() { return }
        if let Some(m) = self.is_header(idx) {
            self.panel_mode = m;
            self.selected_left = self.header_position(m);
            return;
        }
        if let Some(sub) = self.sub_index(idx) {
            self.p().sidx = sub;
            self.p().cidx = 0;
        }
    }

    fn mode_sub_labels(&self, mode: PanelIdx) -> Vec<String> {
        let sidebar_searching = self.searching_sidebar() && mode == self.panel_mode;
        match mode {
            PROJ => self.store.entries(self.show_empty_projects).iter()
                .filter(|(n, _)| !sidebar_searching || self.search_matches(n))
                .map(|(n, c)| format!("{} ({})", n, c)).collect(),
            PLANS => self.plans.iter()
                .filter(|p| !sidebar_searching || self.search_matches(&p.name) || self.search_matches(&p.content))
                .map(|p| p.name.clone()).collect(),
            INBOX => self.inbox_files.iter()
                .filter(|f| !sidebar_searching || self.search_matches(&f.name) || self.search_matches(&f.content))
                .map(|f| f.name.clone()).collect(),
            BACKLOG => self.backlog_store.entries().iter()
                .filter(|(n, _)| !sidebar_searching || self.search_matches(n))
                .map(|(n, c)| format!("{} ({})", n, c)).collect(),
            _ => vec![],
        }
    }
}

fn load_plans() -> Vec<PlanFile> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = Path::new(&home).join(".ted").join("plans");
    let mut files = Vec::new();
    if let Ok(paths) = collect_md_files(&dir) {
        for path in &paths {
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            let content = fs::read_to_string(path).unwrap_or_default();
            files.push(PlanFile { path: path.clone(), name, content });
        }
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    files
}

fn load_inbox() -> Vec<InboxFile> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = Path::new(&home).join(".ted").join("inbox");
    let mut files = Vec::new();
    if let Ok(paths) = collect_md_files(&dir) {
        for path in &paths {
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            let content = fs::read_to_string(path).unwrap_or_default();
            files.push(InboxFile { path: path.clone(), name, content });
        }
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    files
}

// ============================================================================
// Helpers
// ============================================================================

fn collect_md_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !dir.is_dir() { return Ok(files) }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_md_files(&path)?);
        } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
            files.push(path);
        }
    }
    Ok(files)
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    Rect { x, y, width, height }
}

fn push_text(lines: &mut Vec<Line>, label: &str, content: &str, bold: Style, normal: Style) {
    if content.is_empty() { return }
    lines.push(Line::from(Span::styled(format!(" {}:", label), bold)));
    for line in content.lines() { lines.push(Line::from(Span::styled(format!(" {}", line), normal))) }
    lines.push(Line::from(""));
}

// ============================================================================
// Rendering
// ============================================================================

fn render(app: &mut App, f: &mut Frame) {
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

    if app.search_active { render_search_bar(app, f, main[1]) }
    else { render_help(app, f, main[1]) }

    if app.show_detail { render_detail(app, f, f.area()) }
    if app.show_overview { render_overview(app, f, f.area()) }
    if app.confirm_delete.is_some() { render_confirm_delete(app, f, f.area()) }
    if app.confirm_complete.is_some() { render_confirm_complete(app, f, f.area()) }
}

// --- Left panel ---

fn left_items(app: &App) -> Vec<(String, bool)> {
    let mut items = Vec::new();
    let focused = app.focus == Focus::Sidebar;
    let sel = app.selected_left;
    let mut idx = 0usize;

    for m in 0..PANEL_COUNT {
        let expanded = m == app.panel_mode;
        let arrow = if expanded { "\u{25bc}" } else { "\u{25b6}" };
        items.push((format!("{} {}", arrow, PANEL_NAMES[m]), idx == sel && focused));
        idx += 1;
        if expanded {
            for sub in app.mode_sub_labels(m) {
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
        let is_header = text.starts_with('\u{25b6}') || text.starts_with('\u{25bc}');
        let is_active_header = is_header && text.chars().skip(1).collect::<String>().trim() == PANEL_NAMES[app.panel_mode];
        let style = if *is_sel {
            app.theme.selected_style()
        } else if is_active_header {
            Style::default().fg(Color::Rgb(255, 165, 0))
        } else {
            Style::default()
        };
        ListItem::new(text.as_str()).style(style)
    }).collect();

    f.render_widget(TuiList::new(list_items).block(Block::default().borders(Borders::ALL).title(" Outline ").border_style(border)), area);
}

// --- Right panel ---

fn render_right_panel(app: &mut App, f: &mut Frame, area: Rect) {
    match app.panel_mode {
        PROJ => render_todos_content(app, f, area),
        PLANS => render_file_content(app, f, area, " Plan ", app.current_plan().map(|p| (p.name.as_str(), p.content.as_str())), app.panels[PLANS].cidx),
        INBOX => render_file_content(app, f, area, " Inbox ", app.current_inbox().map(|i| (i.name.as_str(), i.content.as_str())), app.panels[INBOX].cidx),
        BACKLOG => render_todos_content(app, f, area),
        _ => {}
    }
}

fn render_todos_content(app: &mut App, f: &mut Frame, area: Rect) {
    let cidx = app.pi().cidx;
    let total = app.current_todos();
    let name = app.current_name().to_string();
    let todos: Vec<&TodoFile> = if app.searching_content() {
        total.iter().filter(|t| app.todo_matches_search(t)).collect()
    } else {
        total.iter().collect()
    };
    let title = format!(" {} ({}/{}) ", name, todos.len(), total.len());
    let right_active = app.focus == Focus::Content;
    let border_st = if right_active { app.theme.active_border_style() } else { app.theme.inactive_border_style() };

    if todos.is_empty() { f.render_widget(Paragraph::new(" No todos yet ").block(Block::default().borders(Borders::ALL).title(title.as_str()).border_style(border_st)), area); return }

    let show_tag = app.pi().sidx == 0;
    let items: Vec<ListItem> = todos.iter().enumerate().map(|(i, todo)| {
        let ch = todo.status_indicator();
        let check = if ch == "\u{2713}" { "\u{2713} " } else { "  " };
        let suffix = todo.completion_summary().map(|s| format!(" [{}]", s)).unwrap_or_default();
        let line = if show_tag {
            let tag = todo.project_tag.as_deref().unwrap_or("?");
            let tag_padded = if tag.len() > 10 { let truncated: String = tag.chars().take(7).collect(); format!("{}...", truncated) } else { format!("{:<10}", tag) };
            Line::from(vec![Span::raw(" "), Span::styled(format!("[{}]", tag_padded), Style::default().fg(Color::Rgb(255, 165, 0))), Span::raw(format!(" {}{}", check, todo.parsed.name)), Span::raw(suffix)])
        } else {
            Line::from(format!(" {}{}{}", check, todo.parsed.name, suffix))
        };
        let highlighted = right_active && i == cidx;
        let style = if highlighted { app.theme.selected_style() } else if todo.is_complete() { Style::default().fg(app.theme.task_completed) } else { Style::default() };
        ListItem::new(line).style(style)
    }).collect();
    let cidx = if cidx >= todos.len() { todos.len().saturating_sub(1) } else { cidx };
    app.p().cidx = cidx;
    app.todo_list_state.select(Some(cidx));
    f.render_stateful_widget(TuiList::new(items).block(Block::default().borders(Borders::ALL).title(title.as_str()).border_style(border_st)), area, &mut app.todo_list_state);
}

fn highlight_matches(line: &str, query: &str, base_style: Style, hl_style: Style) -> Line<'static> {
    if query.is_empty() {
        return Line::from(Span::styled(line.to_string(), base_style));
    }
    let lower = line.to_lowercase();
    let lower_q = query.to_lowercase();
    let mut spans = Vec::new();
    let mut last = 0;
    for (start, _) in lower.match_indices(&lower_q) {
        if start > last { spans.push(Span::styled(line[last..start].to_string(), base_style)); }
        spans.push(Span::styled(line[start..start + query.len()].to_string(), hl_style));
        last = start + query.len();
    }
    if last < line.len() { spans.push(Span::styled(line[last..].to_string(), base_style)); }
    Line::from(spans)
}

fn render_file_content(app: &App, f: &mut Frame, area: Rect, label: &str, file: Option<(&str, &str)>, scroll: usize) {
    let right_active = app.focus == Focus::Content;
    let border = if right_active { app.theme.active_border_style() } else { app.theme.inactive_border_style() };

    let Some((name, content)) = file else {
        f.render_widget(Paragraph::new(" Select a file ").block(Block::default().borders(Borders::ALL).title(label).border_style(border)), area);
        return;
    };

    let hl = Style::default().bg(Color::Rgb(255, 255, 0)).fg(Color::Rgb(0, 0, 0));
    let q = if app.search_active { &app.search_query } else { "" };

    let lines: Vec<Line> = content.lines().skip(scroll).take((area.height - 2) as usize).map(|line| {
        if line.starts_with("# ") { highlight_matches(line, q, Style::default().add_modifier(Modifier::BOLD), hl) }
        else if line.starts_with("## ") { highlight_matches(line, q, Style::default().fg(Color::Rgb(255, 165, 0)), hl) }
        else if line.starts_with("- ") { let s = format!("  \u{2022} {}", &line[2..]); highlight_matches(&s, q, Style::default(), hl) }
        else { highlight_matches(line, q, Style::default(), hl) }
    }).collect();
    f.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(format!(" {} {} ", label.trim(), name)).border_style(border)).wrap(ratatui::widgets::Wrap { trim: false }),
        area,
    );
}

// --- Help bar ---

fn key_label(text: &str, t: &Theme) -> Span<'static> {
    Span::styled(text.to_string(), Style::default().fg(t.help_key))
}

fn render_help(app: &App, f: &mut Frame, area: Rect) {
    let t = &app.theme;
    let k = |s| key_label(s, t);

    let mut spans: Vec<Span> = vec![
        k("Tab"), Span::raw(" Fcs  "),
        k("p"), Span::raw(" Mod  "),
        k("\u{2191}\u{2193}"), Span::raw(" Nav  "),
        k("Ctrl+B"), Span::raw(" Mv  "),
        k("Ctrl+F"), Span::raw(" Sch  "),
    ];
    for (key, desc) in help_keys(app) {
        spans.push(k(key));
        spans.push(Span::raw(format!(" {}  ", desc)));
    }
    spans.push(k("q"));
    spans.push(Span::raw(" / "));
    spans.push(k("Esc"));
    spans.push(Span::raw(" Quit"));

    f.render_widget(Paragraph::new(Line::from(spans)).block(Block::default().borders(Borders::ALL).title(" Help ")), area);
}

fn render_search_bar(app: &App, f: &mut Frame, area: Rect) {
    let t = &app.theme;
    let (display, title) = if app.search_typing {
        let d = if app.search_query.is_empty() { " (type to filter)".to_string() } else { format!(" {}", app.search_query) };
        (d, " Search (Tab\u{2192}apply) ")
    } else {
        let d = if app.search_query.is_empty() { String::new() } else { format!(" Filter: {} (Esc\u{2192}clear) ", app.search_query) };
        (d, " Filter ")
    };
    f.render_widget(
        Paragraph::new(display.as_str()).block(Block::default().borders(Borders::ALL).title(title).border_style(Style::default().fg(t.status_info))),
        area,
    );
}

// --- Detail overlay ---

fn render_detail(app: &App, f: &mut Frame, area: Rect) {
    let todos = app.current_todos();
    if app.filtered_content_len() == 0 { return }
    let todo = &todos[app.real_content_idx()];

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

// --- Overview overlay ---

fn render_overview(app: &App, f: &mut Frame, area: Rect) {
    let todos: Vec<&TodoFile> = if app.searching_content() {
        app.current_todos().iter().filter(|t| app.todo_matches_search(t)).collect()
    } else {
        app.current_todos().iter().collect()
    };
    let popup = centered_rect(area, area.width.saturating_sub(16).min(80).max(40), 16);
    f.render_widget(Clear, popup);

    let t = &app.theme;
    let bold = Style::default().fg(t.popup_text).add_modifier(Modifier::BOLD);
    let normal = Style::default().fg(t.popup_text);
    let green = Style::default().fg(t.status_success);
    let yellow = Style::default().fg(t.status_info);

    let mut completed = 0usize;
    let mut in_progress = 0usize;
    let mut no_tasks = 0usize;
    let mut total_tasks = 0usize;
    let mut done_tasks = 0usize;

    for todo in &todos {
        if todo.parsed.tasks.is_empty() { no_tasks += 1 }
        else if todo.is_complete() { completed += 1 }
        else { in_progress += 1 }
        total_tasks += todo.parsed.tasks.len();
        done_tasks += todo.parsed.tasks.iter().filter(|t| t.completed).count();
    }

    let task_summary = if total_tasks > 0 { format!("{}/{}", done_tasks, total_tasks) } else { "n/a".to_string() };
    let name = app.current_name();

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

fn render_confirm_delete(app: &App, f: &mut Frame, area: Rect) {
    let popup = centered_rect(area, 50, 7);
    f.render_widget(Clear, popup);
    let t = &app.theme;
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(" Delete this inbox file?", Style::default().fg(t.popup_text))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  y", Style::default().fg(t.status_success)),
            Span::raw("es  "),
            Span::styled("n", Style::default().fg(t.status_error)),
            Span::raw("o  "),
            Span::styled("Esc", Style::default().fg(t.help_key)),
            Span::raw(" cancel"),
        ]),
    ];
    f.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" Confirm ").border_style(Style::default().fg(t.project_border))).style(t.popup_bg_style()),
        popup,
    );
}

fn render_confirm_complete(app: &App, f: &mut Frame, area: Rect) {
    let popup = centered_rect(area, 50, 7);
    f.render_widget(Clear, popup);
    let t = &app.theme;
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(" Mark all tasks done and move to ~/.ted/done/?", Style::default().fg(t.popup_text))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  y", Style::default().fg(t.status_success)),
            Span::raw("es  "),
            Span::styled("n", Style::default().fg(t.status_error)),
            Span::raw("o  "),
            Span::styled("Esc", Style::default().fg(t.help_key)),
            Span::raw(" cancel"),
        ]),
    ];
    f.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" Complete ").border_style(Style::default().fg(t.project_border))).style(t.popup_bg_style()),
        popup,
    );
}

// ============================================================================
// Binary lookup
// ============================================================================

fn find_editor() -> PathBuf {
    std::env::var("EDITOR").or_else(|_| std::env::var("VISUAL")).map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("nvim"))
}

fn find_tedtui() -> Option<PathBuf> {
    let try_path = |p: PathBuf| if p.is_file() { Some(p) } else { None };
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if let Some(found) = try_path(dir.join("tedtui")) { return Some(found) }
        }
    }
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            if let Some(found) = try_path(dir.join("tedtui")) { return Some(found) }
        }
    }
    None
}

// ============================================================================
// Event handling
// ============================================================================

fn handle_events(app: &mut App) -> io::Result<ViewAction> {
    let Event::Key(key) = event::read()? else { return Ok(ViewAction::None) };

    if app.show_detail { handle_detail_key(app, key.code); return Ok(ViewAction::None) }
    if app.show_overview { app.show_overview = false; return Ok(ViewAction::None) }
    if let Some(path) = app.confirm_delete.take() {
        if matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y')) {
            return Ok(ViewAction::DeleteFile(path))
        }
        return Ok(ViewAction::None);
    }

    if let Some(path) = app.confirm_complete.take() {
        if matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y')) {
            return Ok(ViewAction::CompleteFile(path))
        }
        return Ok(ViewAction::None);
    }

    if app.search_typing {
        let was_dismissed = matches!(key.code, KeyCode::Esc);
        match key.code {
            KeyCode::Esc => { app.search_active = false; app.search_typing = false; app.search_query.clear(); }
            KeyCode::Tab | KeyCode::BackTab | KeyCode::Enter => { app.search_typing = false; }
            KeyCode::Backspace => { app.search_query.pop(); }
            KeyCode::Char(c) if key.modifiers == KeyModifiers::NONE => { app.search_query.push(c); }
            _ => {}
        }
        if app.searching_sidebar() {
            let count = app.mode_items(app.panel_mode);
            if app.p().sidx >= count { app.p().sidx = count.saturating_sub(1) }
        }
        if app.search_typing || was_dismissed {
            return Ok(ViewAction::None);
        }
    }
    if app.search_active {
        // filter is active but not typing — normal navigation works
        if key.code == KeyCode::Esc {
            app.search_active = false;
            app.search_query.clear();
            return Ok(ViewAction::None);
        }
        if let KeyCode::Char(c) = key.code {
            if key.modifiers == KeyModifiers::NONE {
                app.search_query.push(c);
                app.search_typing = true;
                return Ok(ViewAction::None);
            }
        }
    }

    for (kc, km, action) in global_bindings() {
        if key.code == kc && key.modifiers == km {
            if let Some(result) = action(app) {
                return Ok(result);
            }
            break;
        }
    }

    for (kc, action) in panel_actions(app) {
        
        if key.code != kc {
            continue;
        }

        if let ViewAction::Action(f) = action {
            if let Some(result) = f(app) {
                return Ok(result);
            }
        }
        break;
        
    }

    Ok(ViewAction::None)
}

fn handle_detail_key(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => { app.show_detail = false; app.detail_scroll = 0 }
        KeyCode::Up => app.detail_scroll = app.detail_scroll.saturating_sub(1),
        KeyCode::Down => app.detail_scroll += 1,
        _ => {}
    }
}

fn toggle_empty(app: &mut App) {
    app.show_empty_projects = !app.show_empty_projects;
    let visible = app.store.visible_groups(app.show_empty_projects).len();
    if app.panels[PROJ].sidx > visible { app.panels[PROJ].sidx = visible }
    app.panels[PROJ].cidx = 0;
}

// ============================================================================
// Terminal suspend/resume
// ============================================================================

fn suspend_terminal(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}

fn resume_terminal(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> io::Result<()> {
    enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen, EnableMouseCapture)?;
    terminal.clear()?;
    Ok(())
}

fn suspend_for_editor(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>, path: &Path) -> io::Result<()> {
    suspend_terminal(terminal)?;
    let editor = find_editor();
    let status = Command::new(&editor).arg(path).stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit()).status()?;
    if !status.success() { eprintln!("editor exited with: {}", status) }
    resume_terminal(terminal)
}

fn suspend_for_new_todo(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> io::Result<()> {
    suspend_terminal(terminal)?;
    if let Some(tedtui) = find_tedtui() {
        let status = Command::new(&tedtui).stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit()).status()?;
        if !status.success() { eprintln!("tedtui exited with: {}", status) }
    } else { eprintln!("tedtui binary not found") }
    resume_terminal(terminal)
}

fn suspend_for_tedtui(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>, file: &Path) -> io::Result<()> {
    suspend_terminal(terminal)?;
    if let Some(tedtui) = find_tedtui() {
        let status = Command::new(&tedtui).arg(file).stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit()).status()?;
        if !status.success() { eprintln!("tedtui exited with: {}", status) }
    } else { eprintln!("tedtui binary not found") }
    resume_terminal(terminal)
}

fn parse_inbox_content(content: &str, fallback_name: &str) -> (String, String, String) {
    let mut name = String::new();
    let mut goal = String::new();
    let mut timestamp = String::new();
    let mut in_fm = false;
    let mut past_fm = false;
    let mut found_heading = false;

    for line in content.lines() {
        if line.trim() == "---" {
            if in_fm { in_fm = false; past_fm = true }
            else if !past_fm { in_fm = true }
            continue;
        }
        if in_fm {
            if let Some(val) = line.strip_prefix("timestamp:") { timestamp = val.trim().to_string() }
            continue;
        }
        if past_fm && !found_heading && line.starts_with("# ") {
            name = line.trim_start_matches("# ").trim().to_string();
            found_heading = true;
            continue;
        }
        if found_heading {
            if !goal.is_empty() { goal.push('\n'); }
            goal.push_str(line);
        }
    }

    if name.is_empty() { name = fallback_name.to_string() }
    goal = goal.trim().to_string();
    let info = if !timestamp.is_empty() { format!("From inbox ({})", timestamp) } else { String::new() };
    (name, goal, info)
}

fn suspend_for_inbox_tedtui(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>, inbox: &InboxFile) -> io::Result<()> {
    suspend_terminal(terminal)?;

    let (name, goal, info) = parse_inbox_content(&inbox.content, &inbox.name);

    let json = serde_json::json!({ "name": name, "goal": goal, "info": info });
    let json_str = serde_json::to_string(&json).unwrap_or_else(|_| "{}".to_string());

    if let Some(tedtui) = find_tedtui() {
        let status = Command::new(&tedtui)
            .arg("--json")
            .arg(&json_str)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;
        if !status.success() { eprintln!("tedtui exited with: {}", status) }
    } else { eprintln!("tedtui binary not found") }

    resume_terminal(terminal)
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
        terminal.draw(|f| render(&mut app, f)).map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{}", e)))?;
        match handle_events(&mut app)? {
            ViewAction::OpenEditor(path) => { suspend_for_editor(&mut terminal, &path)?; app.reload() }
            ViewAction::OpenInTedtui(inbox) => { suspend_for_inbox_tedtui(&mut terminal, &inbox)?; app.reload() }
            ViewAction::EditInTedtui(tedtuifile) => {suspend_for_tedtui(&mut terminal, &tedtuifile)?; app.reload()}
            ViewAction::NewTodo => { suspend_for_new_todo(&mut terminal)?; app.reload() }
            ViewAction::DeleteFile(path) => {
                let _ = fs::remove_file(&path);
                app.reload();
            }
            ViewAction::MoveFile(src, target_dir) => {
                let dest = target_dir.join(src.file_name().unwrap_or_default());
                let _ = fs::create_dir_all(&target_dir);
                if fs::copy(&src, &dest).is_ok() {
                    let _ = fs::remove_file(&src);
                }
                app.reload();
            }
            ViewAction::CompleteFile(path) => {
                if let Ok(content) = fs::read_to_string(&path) {
                    let done_dir = Path::new(&std::env::var("HOME").unwrap_or_else(|_| ".".to_string())).join(".ted").join("done");
                    let dest = done_dir.join(path.file_name().unwrap_or_default());
                    let ts = chrono::Local::now().format("%m-%d-%Y_%H:%M:%S").to_string();
                    let mut lines: Vec<String> = content.lines().map(|l| {
                        if l.trim() == "completed: null" { format!("completed: {}", ts) }
                        else if l.starts_with("- [ ] ") { format!("- [x] {}", &l[6..]) }
                        else { l.to_string() }
                    }).collect();
                    let has_fm = lines.first().map(|l| l.trim() == "---").unwrap_or(false);
                    let has_completed = lines.iter().any(|l| l.trim().starts_with("completed:"));
                    if !has_completed {
                        if has_fm {
                            lines.insert(1, format!("completed: {}", ts));
                        } else {
                            lines = vec!["---".into(), format!("completed: {}", ts), "---".into()].into_iter().chain(lines).collect();
                        }
                    }
                    let _ = fs::create_dir_all(&done_dir);
                    let _ = fs::write(&dest, lines.join("\n"));
                    let _ = fs::remove_file(&path);
                }
                app.reload();
            }
            ViewAction::RunBackground(cmd) => {
                let _ = Command::new("sh").arg("-c").arg(&cmd).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
                app.reload();
            }
            ViewAction::RunObsidian(cmd) => {
                suspend_for_obsidian(&mut terminal, &cmd)?;
                app.reload();
            }
            ViewAction::None | ViewAction::Action(_) => {}
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}

fn suspend_for_obsidian(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>, cmd: &str) -> io::Result<()> {
    suspend_terminal(terminal)?;
    let status = Command::new("sh").arg("-c").arg(cmd).stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit()).status()?;
    if !status.success() { eprintln!("obsidian exited with: {}", status) }
    resume_terminal(terminal)
}
