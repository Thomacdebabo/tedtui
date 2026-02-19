use chrono::Local;
use std::fs;
use std::io;
use std::path::Path;

use crate::filestorage::FileStorage;

pub struct TodoData {
    pub name: String,
    pub project_id: String,
    pub info: String,
    pub project_shorthand: Option<String>,
    pub goal: String,
    pub tasks: Vec<String>,
    pub note: String,
    pub history: String,
    pub existing_id: Option<String>,
    pub existing_created: Option<String>,
    pub target_filepath: Option<String>,
    pub completed: Option<String>,
}

impl TodoData {
    pub fn save_to_markdown(&self, output_dir: &str) -> io::Result<String> {
        let file_storage = FileStorage::new();

        // Use existing filepath if editing, otherwise generate new
        let (filepath, id, timestamp) = if let Some(ref existing_path) = self.target_filepath {
            // Editing existing file
            let id = self
                .existing_id
                .clone()
                .unwrap_or_else(|| "UNKNOWN".to_string());
            let timestamp = self
                .existing_created
                .clone()
                .unwrap_or_else(|| Local::now().format("%m-%d-%Y_%H:%M:%S").to_string());
            (existing_path.clone(), id, timestamp)
        } else {
            // Creating new file
            let next_id = file_storage.get_next_id()?;
            let filename =
                file_storage.generate_filename(&self.name, self.project_shorthand.as_deref());

            // Generate the ID string for frontmatter
            let id = if let Some(ref shorthand) = self.project_shorthand {
                format!("{}{:03}", shorthand, next_id)
            } else {
                format!("T{:05}", next_id)
            };

            let timestamp = Local::now().format("%m-%d-%Y_%H:%M:%S").to_string();
            let filepath = Path::new(output_dir)
                .join(&filename)
                .to_string_lossy()
                .to_string();

            (filepath, id, timestamp)
        };

        // Format project_id with [[ ]]
        let formatted_project_id = FileStorage::format_project_id(&self.project_id);

        // Build markdown content
        let mut content = String::new();

        // Frontmatter
        content.push_str("---\n");
        if let Some(ref completed) = self.completed {
            content.push_str(&format!("completed: {}\n", completed));
        } else {
            content.push_str("completed: null\n");
        }
        content.push_str(&format!("created: {}\n", timestamp));
        content.push_str(&format!("id: {}\n", id));
        content.push_str(&format!("info: '{}'\n", self.info));
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
            content.push_str(&format!("- {}\n", task));
        }

        // Note section
        content.push_str("# note\n");
        if !self.note.is_empty() {
            content.push_str(&self.note);
            content.push('\n');
        }

        // History section (last)
        content.push_str("# History \n");
        if !self.history.is_empty() {
            content.push_str(&self.history);
            content.push('\n');
        }

        // Create directory if it doesn't exist
        let path = Path::new(&filepath);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write to file
        fs::write(&filepath, content)?;

        Ok(filepath)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_unicode_content_preservation() {
        // Test that unicode characters are preserved in markdown content
        let todo = TodoData {
            name: "Test mit Ümläuten".to_string(),
            project_id: String::new(),
            info: String::new(),
            project_shorthand: None,
            goal: "Schöne Grüße aus München".to_string(),
            tasks: vec![
                "[ ] Äpfel kaufen".to_string(),
                "[ ] Über Brücke gehen".to_string(),
                "[x] Tschüss sagen".to_string(),
            ],
            note: "Café, naïve, 日本語, emoji: 🎉".to_string(),
            history: String::new(),
            existing_id: None,
            existing_created: None,
            target_filepath: None,
            completed: None,
        };

        let temp_dir = env::temp_dir().join("tedtui_unicode_test");
        fs::create_dir_all(&temp_dir).unwrap();
        let output_dir = temp_dir.to_string_lossy().to_string();

        // Save the file
        let filepath = todo.save_to_markdown(&output_dir).unwrap();

        // Read it back
        let content = fs::read_to_string(&filepath).unwrap();

        // Verify unicode characters are preserved
        assert!(content.contains("Test mit Ümläuten"));
        assert!(content.contains("Schöne Grüße aus München"));
        assert!(content.contains("Äpfel kaufen"));
        assert!(content.contains("Über Brücke gehen"));
        assert!(content.contains("Tschüss sagen"));
        assert!(content.contains("Café, naïve, 日本語, emoji: 🎉"));

        // Verify unicode is also in the filename
        assert!(filepath.contains("test_mit_ümläuten"));

        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();
    }
}
