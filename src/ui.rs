// ============================================================================
// UI Rendering
// ============================================================================

use crate::{App, InputField};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
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

    let goal_block = create_input_block("Goal / Short Description (↑↓ to scroll)", is_active);
    Paragraph::new(visible_text)
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

    // Get visible lines based on scroll offset
    let lines: Vec<&str> = app.content.note.lines().collect();
    let visible_lines: Vec<&str> = lines
        .iter()
        .skip(app.state.note_scroll_offset)
        .take(6) // Note has 6 visible lines
        .copied()
        .collect();
    let mut visible_text = visible_lines.join("\n");

    let note_block: Block<'_> = create_input_block("Note (↑↓ to scroll)", is_active);

    if is_active {
        visible_text.push('█');
    }
    Paragraph::new(visible_text)
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
                format!(
                    "... {} more entries (↓ to scroll)",
                    history_entries.len() - end_idx
                ),
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
