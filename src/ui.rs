// ============================================================================
// UI Rendering
// ============================================================================

use crate::app::NewProjectStep;
use crate::{App, InputField};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use std::path::Path;
use unicode_width::UnicodeWidthStr;

pub enum WidgetItem<'a> {
    Paragraph(Paragraph<'a>, ratatui::layout::Rect),
    List(List<'a>, ratatui::layout::Rect),
    StatefulList(List<'a>, ListState, ratatui::layout::Rect),
    Clear(ratatui::layout::Rect),
}

pub fn ui(f: &mut Frame, app: &App) {
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

    let help = create_help(app);
    widgets.push(WidgetItem::Paragraph(help, chunks[5]));

    if let Some(status_message) = create_status_message(app) {
        widgets.push(WidgetItem::Paragraph(status_message, chunks[6]));
    }

    // Render overlays (these are rendered immediately due to borrowing constraints)
    if app.state.show_new_project_dialog {
        let (new_project_overlay, popup_area) = create_new_project_overlay(app, f.area());
        widgets.push(WidgetItem::Clear(popup_area));
        widgets.push(WidgetItem::Paragraph(new_project_overlay, popup_area));
    }
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
    if app.state.show_move_browser {
        let (move_browser, popup_area) = create_move_browser_overlay(app, f.area());
        widgets.push(WidgetItem::Clear(popup_area));
        widgets.push(WidgetItem::List(move_browser, popup_area));
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

fn create_input_block<'a>(title: &'a str, is_active: bool, app: &'a App) -> Block<'a> {
    let t = &app.config.theme;
    Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_style(Style::default().fg(t.title_text))
        .border_style(if is_active {
            t.active_border_style()
        } else {
            t.inactive_border_style()
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
    let name_block = create_input_block("Name", is_active, app);
    let name_text = truncate_text_for_display(&app.content.name, area.width as usize - 4);
    let styled_text = Span::styled(name_text, Style::default().fg(app.config.theme.field_text));
    Paragraph::new(styled_text).block(name_block)
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
    let project_block = create_input_block(project_title, is_active, app);
    let project_text = truncate_text_for_display(
        &app.content.project_id,
        project_info_chunks[0].width as usize - 4,
    );
    let text_style = Style::default().fg(app.config.theme.field_text);
    let project_paragraph =
        Paragraph::new(Span::styled(project_text, text_style)).block(project_block);

    // Info
    let is_active = app.state.current_field == InputField::Info;
    let info_block = create_input_block("Info", is_active, app);
    let info_text =
        truncate_text_for_display(&app.content.info, project_info_chunks[1].width as usize - 4);
    let info_paragraph = Paragraph::new(Span::styled(info_text, text_style)).block(info_block);

    (
        project_paragraph,
        info_paragraph,
        project_info_chunks.to_vec(),
    )
}

fn create_goal_field<'a>(app: &'a App) -> Paragraph<'a> {
    let is_active = app.state.current_field == InputField::Goal;

    // Get visible lines based on scroll offset
    let lines: Vec<&str> = app.content.goal.lines().collect();
    let visible_lines: Vec<&str> = lines
        .iter()
        .skip(app.state.goal_scroll_offset)
        .take(3) // Goal has 3 visible lines
        .copied()
        .collect();
    let mut visible_text = visible_lines.join("\n");

    if is_active {
        visible_text.push('█');
    }

    let goal_block = create_input_block("Goal / Short Description (↑↓ to scroll)", is_active, app);
    let text_style = Style::default().fg(app.config.theme.field_text);
    Paragraph::new(Span::styled(visible_text, text_style))
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
    let task_input_block = create_input_block("Add Task (Enter to add)", is_active, app);
    let task_text = truncate_text_for_display(
        &app.content.current_task_input,
        tasks_chunks[0].width as usize - 4,
    );
    let text_style = Style::default().fg(app.config.theme.field_text);
    let task_input_paragraph =
        Paragraph::new(Span::styled(task_text, text_style)).block(task_input_block);

    // Task list
    let t = &app.config.theme;
    let task_items: Vec<ListItem> = app
        .content
        .tasks
        .iter()
        .map(|task| {
            let checkbox = if task.completed { "[x]" } else { "[ ]" };
            let text = format!("  - {} {}", checkbox, task.text);
            let style = if task.completed {
                Style::default().fg(t.task_completed)
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
            app,
        ))
        .highlight_style(
            Style::default()
                .bg(t.task_highlight_bg)
                .fg(t.task_highlight_fg),
        );

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

    // Get visible lines based on scroll offset
    let lines: Vec<&str> = app.content.note.lines().collect();
    let visible_lines: Vec<&str> = lines
        .iter()
        .skip(app.state.note_scroll_offset)
        .take(6) // Note has 6 visible lines
        .copied()
        .collect();
    let mut visible_text = visible_lines.join("\n");

    let note_block: Block<'_> = create_input_block("Note (↑↓ to scroll)", is_active, app);

    if is_active {
        visible_text.push('█');
    }
    let text_style = Style::default().fg(app.config.theme.field_text);
    Paragraph::new(Span::styled(visible_text, text_style))
        .block(note_block)
        .wrap(Wrap { trim: false })
}

fn create_help(app: &App) -> Paragraph<'static> {
    let t = &app.config.theme;
    let key_style = Style::default().fg(t.help_key);
    let help_text = Line::from(vec![
        Span::styled("Tab", key_style),
        Span::raw(" / "),
        Span::styled("Shift+Tab", key_style),
        Span::raw(" - Navigate | "),
        Span::styled("Space", key_style),
        Span::raw(" - Toggle | "),
        Span::styled("Ctrl+P", key_style),
        Span::raw(" - Projects | "),
        Span::styled("Ctrl+H", key_style),
        Span::raw(" - History | "),
        Span::styled("Ctrl+S", key_style),
        Span::raw(" - Save | "),
        Span::styled("Ctrl+D", key_style),
        Span::raw(" - Done | "),
        Span::styled("Ctrl+G", key_style),
        Span::raw(" - Move | "),
        Span::styled("Esc", key_style),
        Span::raw(" - Quit"),
    ]);
    Paragraph::new(help_text).block(Block::default().borders(Borders::ALL).title("Help"))
}

fn create_status_message<'a>(app: &'a App) -> Option<Paragraph<'a>> {
    let t = &app.config.theme;
    app.state.status_message.as_ref().map(|msg| {
        let status_color = if msg.contains("✓") {
            t.status_success
        } else if msg.contains("✗") {
            t.status_error
        } else {
            t.status_info
        };
        Paragraph::new(msg.as_str())
            .style(Style::default().fg(status_color))
            .wrap(Wrap { trim: false })
    })
}

fn create_new_project_overlay<'a>(
    app: &'a App,
    frame_area: ratatui::layout::Rect,
) -> (Paragraph<'a>, ratatui::layout::Rect) {
    let popup_width = frame_area.width.saturating_sub(20).min(70).max(50);
    let popup_height = 14u16;
    let popup_x = (frame_area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (frame_area.height.saturating_sub(popup_height)) / 2;

    let popup_area = ratatui::layout::Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width,
        height: popup_height,
    };

    let t = &app.config.theme;
    let active = Style::default().fg(t.popup_active_input).bg(t.popup_bg);
    let inactive = Style::default().fg(t.popup_inactive_input).bg(t.popup_bg);
    let label = Style::default().fg(t.popup_label).bg(t.popup_bg);

    let name_style = if app.state.new_project_step == NewProjectStep::Name {
        active
    } else {
        inactive
    };
    let desc_style = if app.state.new_project_step == NewProjectStep::Description {
        active
    } else {
        inactive
    };
    let short_style = if app.state.new_project_step == NewProjectStep::Shorthand {
        active
    } else {
        inactive
    };

    let mut name_val = app.state.new_project_name.clone();
    let mut desc_val = app.state.new_project_description.clone();
    let mut short_val = app.state.new_project_shorthand.clone();

    if app.state.new_project_step == NewProjectStep::Name {
        name_val.push('\u{2588}');
    }
    if app.state.new_project_step == NewProjectStep::Description {
        desc_val.push('\u{2588}');
    }
    if app.state.new_project_step == NewProjectStep::Shorthand {
        short_val.push('\u{2588}');
    }

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Name        : ", label),
            Span::styled(name_val, name_style),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Description : ", label),
            Span::styled(desc_val, desc_style),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Shorthand   : ", label),
            Span::styled(short_val, short_style),
        ]),
        Line::from(Span::styled(
            "  (3-8 uppercase chars, optional)",
            Style::default().fg(t.popup_hint),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Tab/Enter │ next    Shift+Tab │ back    Esc │ cancel",
            Style::default().fg(t.popup_help),
        )),
    ];

    if let Some(ref err) = app.state.new_project_error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  \u{2717} {}", err),
            Style::default().fg(t.popup_error),
        )));
    }

    let overlay = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" New Project ")
                .border_style(Style::default().fg(t.new_project_border)),
        )
        .style(t.popup_bg_style());

    (overlay, popup_area)
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

    let filtered = app.get_filtered_projects();
    let t = &app.config.theme;
    let mut items: Vec<ListItem> = Vec::new();

    // Filter input line
    let filter_display = format!("  Filter: {}█", app.state.project_filter);
    items.push(ListItem::new(filter_display).style(Style::default().fg(t.filter_input)));
    items.push(ListItem::new("  ─────────────────").style(Style::default().fg(t.separator)));

    if filtered.is_empty() {
        items.push(ListItem::new("  (no matches)").style(Style::default().fg(t.no_results)));
    } else {
        for (i, (_, project)) in filtered.iter().enumerate() {
            let style = if Some(i) == app.state.selected_project_index {
                t.selected_style()
            } else {
                Style::default()
            };
            let text = if let Some(ref shorthand) = project.shorthand {
                format!("  {} - {} ({})", project.id, project.name, shorthand)
            } else {
                format!("  {} - {}", project.id, project.name)
            };
            items.push(ListItem::new(text).style(style));
        }
    }

    let projects_list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Select Project (↑↓ Navigate, Enter Select, Ctrl+N New, Esc Cancel)")
                .border_style(Style::default().fg(t.project_border)),
        )
        .style(t.popup_bg_style());

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

    let t = &app.config.theme;
    let incomplete_count = app.content.tasks.iter().filter(|t| !t.completed).count();
    let message = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("You have {} incomplete task(s).", incomplete_count),
            Style::default().fg(t.popup_text),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Mark all tasks as complete before moving to done?",
            Style::default().fg(t.confirm_prompt),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Press ", Style::default().fg(t.popup_text)),
            Span::styled("Y", Style::default().fg(t.confirm_yes)),
            Span::styled(
                " to mark complete and move, ",
                Style::default().fg(t.popup_text),
            ),
            Span::styled("N", Style::default().fg(t.confirm_no)),
            Span::styled(" to cancel", Style::default().fg(t.popup_text)),
        ]),
    ];

    let confirmation = Paragraph::new(message)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Confirm Move to Done")
                .border_style(Style::default().fg(t.confirm_border)),
        )
        .style(t.popup_bg_style())
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

    let t = &app.config.theme;
    let visible_lines = (popup_height as usize).saturating_sub(3); // Account for borders and title
    let start_idx = app.state.history_scroll_offset;
    let end_idx = (start_idx + visible_lines).min(history_entries.len());

    let mut lines: Vec<Line> = vec![];

    if history_entries.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "No history entries yet.",
            Style::default().fg(t.popup_hint),
        )));
    } else {
        for entry in history_entries.iter().skip(start_idx).take(visible_lines) {
            lines.push(Line::from(Span::styled(
                *entry,
                Style::default().fg(t.history_entry),
            )));
        }

        // Show scroll indicator if there's more content
        if end_idx < history_entries.len() {
            lines.push(Line::from(Span::styled(
                format!(
                    "... {} more entries (↓ to scroll)",
                    history_entries.len() - end_idx
                ),
                Style::default().fg(t.history_scroll_hint),
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
                .border_style(Style::default().fg(t.history_border)),
        )
        .style(t.popup_bg_style())
        .wrap(Wrap { trim: false });

    (history_view, popup_area)
}

fn create_move_browser_overlay<'a>(
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

    let ted_root_home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let ted_root = Path::new(&ted_root_home).join(".ted");
    let display_path = app
        .state
        .move_browser_path
        .strip_prefix(&ted_root)
        .map(|p| {
            let s = p.display().to_string();
            if s.is_empty() {
                ".ted".to_string()
            } else {
                format!(".ted/{}", s)
            }
        })
        .unwrap_or_else(|_| app.state.move_browser_path.display().to_string());

    let t = &app.config.theme;
    let filtered = app.get_filtered_move_entries();
    let mut items: Vec<ListItem> = Vec::new();

    // Filter input line
    let filter_display = format!("  Filter: {}█", app.state.move_browser_filter);
    items.push(ListItem::new(filter_display).style(Style::default().fg(t.filter_input)));
    items.push(ListItem::new("  ─────────────────").style(Style::default().fg(t.separator)));

    if app.state.move_browser_entries.is_empty() {
        items.push(ListItem::new("  (no subdirectories)").style(Style::default().fg(t.no_results)));
    } else if filtered.is_empty() {
        items.push(ListItem::new("  (no matches)").style(Style::default().fg(t.no_results)));
    } else {
        for (i, (_, entry)) in filtered.iter().enumerate() {
            let indicator = if entry.has_subdirs { " ▸" } else { "" };
            let text = format!("  {}{}", entry.name, indicator);
            let style = if i == app.state.move_browser_selected {
                t.selected_style()
            } else {
                Style::default()
            };
            items.push(ListItem::new(text).style(style));
        }
    };

    let title = format!(
        " Move to: {} (←→ navigate, Enter select, Esc cancel) ",
        display_path
    );

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(t.move_border)),
        )
        .style(t.popup_bg_style());

    (list, popup_area)
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
    let mut cursor_position: Option<(u16, u16)> = None;
    match app.state.current_field {
        InputField::Name => {
            let cursor_x = calculate_cursor_x(&app.content.name, chunks[0].x, chunks[0].width);
            cursor_position = Some((cursor_x, chunks[0].y + 1));
        }
        InputField::ProjectId => {
            let cursor_x = calculate_cursor_x(
                &app.content.project_id,
                project_info_chunks[0].x,
                project_info_chunks[0].width,
            );
            cursor_position = Some((cursor_x, project_info_chunks[0].y + 1));
        }
        InputField::Info => {
            let cursor_x = calculate_cursor_x(
                &app.content.info,
                project_info_chunks[1].x,
                project_info_chunks[1].width,
            );
            cursor_position = Some((cursor_x, project_info_chunks[1].y + 1));
        }
        InputField::Goal => { /*  cursor is just appended to text */ }
        InputField::Tasks => {
            let cursor_x = calculate_cursor_x(
                &app.content.current_task_input,
                tasks_chunks[0].x,
                tasks_chunks[0].width,
            );
            cursor_position = Some((cursor_x, tasks_chunks[0].y + 1));
        }
        InputField::TaskList => {
            // Position cursor at the selected task in the list
            if let Some(selected_idx) = app.state.selected_task_index {
                let cursor_x = tasks_chunks[1].x + 1;
                let cursor_y = tasks_chunks[1].y + 1 + selected_idx as u16;
                cursor_position = Some((cursor_x, cursor_y));
            } else {
                // No task selected, position at the start of the task list
                cursor_position = Some((tasks_chunks[1].x + 1, tasks_chunks[1].y + 1));
            }
        }
        InputField::Note => { /*  cursor is just appended to text */ }
    }
    if let Some(cursor) = cursor_position {
        f.set_cursor_position(cursor);
    }
}
