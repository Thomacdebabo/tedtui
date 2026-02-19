use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ParsedTodo {
    pub name: String,
    pub project_id: String,
    pub info: String,
    pub goal: String,
    pub tasks: Vec<Task>,
    pub note: String,
    pub history: String,
    pub id: String,
    pub created: String,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub text: String,
    pub completed: bool,
}

pub fn parse_markdown_file(path: &Path) -> io::Result<ParsedTodo> {
    let content = fs::read_to_string(path)?;

    let mut name = String::new();
    let mut project_id = String::new();
    let mut info = String::new();
    let mut goal = String::new();
    let mut tasks = Vec::new();
    let mut note = String::new();
    let mut history = String::new();
    let mut id = String::new();
    let mut created = String::new();

    let mut in_frontmatter = false;
    let mut in_tasks_section = false;
    let mut in_note_section = false;
    let mut in_history_section = false;
    let mut found_heading = false;

    for line in content.lines() {
        if line.trim() == "---" {
            in_frontmatter = !in_frontmatter;
            continue;
        }

        if in_frontmatter {
            // Parse frontmatter
            if let Some(value) = line.strip_prefix("id:") {
                id = value
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
            } else if let Some(value) = line.strip_prefix("project_id:") {
                let val = value.trim().trim_matches('\'');
                if val.starts_with("[[") && val.ends_with("]]") {
                    project_id = val.trim_matches('[').trim_matches(']').to_string();
                } else if val != "''" && val != "null" {
                    project_id = val.to_string();
                }
            } else if let Some(value) = line.strip_prefix("info:") {
                let val = value.trim().trim_matches('\'').trim_matches('"');
                if val != "''" && val != "null" {
                    info = val.to_string();
                }
            } else if let Some(value) = line.strip_prefix("created:") {
                created = value.trim().to_string();
            }
            continue;
        }

        // Parse heading (name)
        if line.starts_with("# ") && !found_heading {
            name = line.trim_start_matches("# ").trim().to_string();
            found_heading = true;
            continue;
        }

        // Check for section headers
        if line.trim() == "# Tasks" {
            in_tasks_section = true;
            in_note_section = false;
            in_history_section = false;
            continue;
        } else if line.trim() == "# note" {
            in_note_section = true;
            in_tasks_section = false;
            in_history_section = false;
            continue;
        } else if line.trim() == "# History" || line.trim() == "# Info" {
            // Support both new "History" and old "Info" section names
            in_history_section = true;
            in_tasks_section = false;
            in_note_section = false;
            continue;
        } else if line.starts_with("# ") {
            in_tasks_section = false;
            in_note_section = false;
            in_history_section = false;
            continue;
        }

        if in_tasks_section {
            // Parse tasks (both - [ ] and - [x])
            if let Some(task_text) = line
                .strip_prefix("- [x] ")
                .or_else(|| line.strip_prefix("- [X] "))
            {
                tasks.push(Task {
                    text: task_text.trim().to_string(),
                    completed: true,
                });
            } else if let Some(task_text) = line.strip_prefix("- [ ] ") {
                tasks.push(Task {
                    text: task_text.trim().to_string(),
                    completed: false,
                });
            }
        } else if in_note_section {
            if !line.is_empty() {
                if !note.is_empty() {
                    note.push('\n');
                }
                note.push_str(line);
            }
        } else if in_history_section {
            if !line.is_empty() {
                if !history.is_empty() {
                    history.push('\n');
                }
                history.push_str(line);
            }
        } else if found_heading
            && !in_tasks_section
            && !in_note_section
            && !in_history_section
            && !line.starts_with("# ")
        {
            // This is the goal/description section
            if !line.trim().is_empty() {
                if !goal.is_empty() {
                    goal.push('\n');
                }
                goal.push_str(line);
            }
        }
    }

    Ok(ParsedTodo {
        name,
        project_id,
        info: info.trim().to_string(),
        goal: goal.trim().to_string(),
        tasks,
        note: note.trim().to_string(),
        history: history.trim().to_string(),
        id,
        created,
    })
}
