use chrono::Local;
use std::fs;
use std::io;
use std::path::Path;

use crate::filestorage::FileStorage;

pub struct TodoData {
    pub name: String,
    pub project_id: String,
    pub project_shorthand: Option<String>,
    pub goal: String,
    pub tasks: Vec<String>,
    pub note: String,
}

impl TodoData {
    pub fn save_to_markdown(&self, output_dir: &str) -> io::Result<String> {
        let file_storage = FileStorage::new();
        
        // Generate unique ID and filename
        let next_id = file_storage.get_next_id()?;
        let filename = file_storage.generate_filename(&self.name, self.project_shorthand.as_deref());
        
        // Generate the ID string for frontmatter
        let id = if let Some(ref shorthand) = self.project_shorthand {
            format!("{}{:03}", shorthand, next_id)
        } else {
            format!("T{:05}", next_id)
        };
        
        // Generate timestamp
        let timestamp = Local::now().format("%m-%d-%Y_%H:%M:%S").to_string();
        
        let filepath = Path::new(output_dir).join(&filename);
        
        // Format project_id with [[ ]]
        let formatted_project_id = FileStorage::format_project_id(&self.project_id);
        
        // Build markdown content
        let mut content = String::new();
        
        // Frontmatter
        content.push_str("---\n");
        content.push_str("completed: null\n");
        content.push_str(&format!("created: {}\n", timestamp));
        content.push_str(&format!("id: {}\n", id));
        content.push_str("info: ''\n");
        content.push_str(&format!("project_id: {}\n", formatted_project_id));
        content.push_str("tags: []\n");
        content.push_str("---\n");
        
        // Name as heading
        content.push_str(&format!("# {}\n", self.name));
        
        // Goal/Description
        if !self.goal.is_empty() {
            content.push_str(&format!("{}\n", self.goal));
        }
        
        // Tasks section
        content.push_str("# Tasks \n");
        for task in &self.tasks {
            content.push_str(&format!("- [ ] {}\n", task));
        }
        
        // Info section
        content.push_str("# Info \n\n");
        
        // Note section
        content.push_str("# note\n");
        if !self.note.is_empty() {
            content.push_str(&self.note);
            content.push('\n');
        }
        
        // Create directory if it doesn't exist
        if let Some(parent) = filepath.parent() {
            fs::create_dir_all(parent)?;
        }
        
        // Write to file
        fs::write(&filepath, content)?;
        
        Ok(filepath.to_string_lossy().to_string())
    }
}
