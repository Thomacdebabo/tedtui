use std::fs;
use std::io;
use std::path::Path;

/// Unescape YAML \xHH sequences to proper UTF-8 characters
fn unescape_yaml_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('x') => {
                    let hex: String = chars.by_ref().take(2).collect();
                    if let Ok(byte) = u32::from_str_radix(&hex, 16) {
                        if let Some(ch) = char::from_u32(byte) {
                            result.push(ch);
                            continue;
                        }
                    }
                    result.push('\\');
                    result.push('x');
                    result.push_str(&hex);
                }
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
}

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

enum ParseState {
    Initial,
    Frontmatter,
    Goal,
    Tasks,
    Note,
    History,
    Other,
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

    let mut state = ParseState::Initial;
    let mut found_heading = false;

    for line in content.lines() {
        if line.trim() == "---" {
            state = match state {
                ParseState::Frontmatter => {
                    if found_heading {
                        ParseState::Goal
                    } else {
                        ParseState::Initial
                    }
                }
                _ => ParseState::Frontmatter,
            };
            continue;
        }

        if matches!(state, ParseState::Frontmatter) {
            // Parse frontmatter
            if let Some(value) = line.strip_prefix("id:") {
                id = unescape_yaml_string(value.trim().trim_matches('"').trim_matches('\''));
            } else if let Some(value) = line.strip_prefix("project_id:") {
                let val = value.trim().trim_matches('\'');
                if val.starts_with("[[") && val.ends_with("]]") {
                    project_id = unescape_yaml_string(&val.trim_matches('[').trim_matches(']'));
                } else if val != "''" && val != "null" {
                    project_id = unescape_yaml_string(val);
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
            state = ParseState::Goal;
            continue;
        }

        // Check for section headers
        if line.trim() == "# Tasks" {
            state = ParseState::Tasks;
            continue;
        } else if line.trim() == "# note" || line.trim() == "# Note" {
            state = ParseState::Note;
            continue;
        } else if line.trim() == "# History" || line.trim() == "# Info" {
            // Support both new "History" and old "Info" section names
            state = ParseState::History;
            continue;
        } else if line.starts_with("# ") {
            state = ParseState::Other;
            continue;
        }

        match state {
            ParseState::Tasks => {
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
            }
            ParseState::Note => {
                if !line.is_empty() {
                    if !note.is_empty() {
                        note.push('\n');
                    }
                    note.push_str(line);
                }
            }
            ParseState::History => {
                if !line.is_empty() {
                    if !history.is_empty() {
                        history.push('\n');
                    }
                    history.push_str(line);
                }
            }
            ParseState::Goal => {
                // This is the goal/description section
                if !line.trim().is_empty() {
                    if !goal.is_empty() {
                        goal.push('\n');
                    }
                    goal.push_str(line);
                }
            }
            _ => {}
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
