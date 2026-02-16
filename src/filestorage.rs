use std::fs;
use std::path::{Path, PathBuf};
use regex::Regex;

#[derive(Debug, Clone)]
pub struct Project {
    pub id: String,
    pub shorthand: Option<String>,
    pub name: String,
    pub filepath: PathBuf,
}

pub struct FileStorage {
    ted_root: PathBuf,
}

impl FileStorage {
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let ted_root = Path::new(&home).join(".ted");
        FileStorage { ted_root }
    }

    pub fn with_root(root: PathBuf) -> Self {
        FileStorage { ted_root: root }
    }

    /// Get the next available ID by scanning todos and done folders
    pub fn get_next_id(&self) -> Result<u32, std::io::Error> {
        let mut max_id = 0;

        // Scan todos folder
        if let Ok(entries) = fs::read_dir(self.ted_root.join("todos")) {
            for entry in entries.flatten() {
                if let Some(id) = self.extract_id_from_filename(&entry.file_name().to_string_lossy()) {
                    max_id = max_id.max(id);
                }
            }
        }

        // Scan done folder
        if let Ok(entries) = fs::read_dir(self.ted_root.join("done")) {
            for entry in entries.flatten() {
                if let Some(id) = self.extract_id_from_filename(&entry.file_name().to_string_lossy()) {
                    max_id = max_id.max(id);
                }
            }
        }

        Ok(max_id + 1)
    }

    /// Extract numeric ID from filename
    /// Examples: "ADM106_something.md" -> Some(106), "T00061_test.md" -> Some(61)
    fn extract_id_from_filename(&self, filename: &str) -> Option<u32> {
        // Match patterns like: PREFIX123_description.md or PREFIX00123_description.md
        let re = Regex::new(r"^[A-Z]+(\d+)_").ok()?;
        if let Some(caps) = re.captures(filename) {
            if let Some(id_str) = caps.get(1) {
                return id_str.as_str().parse::<u32>().ok();
            }
        }
        None
    }

    /// Get all projects from the projects folder
    pub fn get_projects(&self) -> Result<Vec<Project>, std::io::Error> {
        let mut projects = Vec::new();
        let projects_dir = self.ted_root.join("projects");

        if !projects_dir.exists() {
            return Ok(projects);
        }

        for entry in fs::read_dir(projects_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("md") {
                if let Some(project) = self.parse_project_file(&path)? {
                    projects.push(project);
                }
            }
        }

        // Sort by ID
        projects.sort_by(|a, b| a.id.cmp(&b.id));

        Ok(projects)
    }

    /// Parse a project file to extract metadata
    fn parse_project_file(&self, path: &Path) -> Result<Option<Project>, std::io::Error> {
        let filename = path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        // Parse filename: P00017_MOVE_Move.md
        let re = Regex::new(r"^(P\d+)(?:_([A-Z]+))?(?:_(.+))?\.md$").ok();
        if let Some(re) = re {
            if let Some(caps) = re.captures(filename) {
                let id = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
                let shorthand_from_filename = caps.get(2).map(|m| m.as_str().to_string());
                
                // Try to get name and shorthand from file content (the heading)
                let (name, shorthand) = if let Ok(content) = fs::read_to_string(path) {
                    let (extracted_shorthand, extracted_name) = self.extract_project_info_from_content(&content);
                    let final_name = extracted_name
                        .or_else(|| caps.get(3).map(|m| m.as_str().to_string()))
                        .unwrap_or_else(|| id.clone());
                    let final_shorthand = shorthand_from_filename.or(extracted_shorthand);
                    (final_name, final_shorthand)
                } else {
                    let final_name = caps.get(3).map(|m| m.as_str().to_string())
                        .unwrap_or_else(|| id.clone());
                    (final_name, shorthand_from_filename)
                };

                return Ok(Some(Project {
                    id,
                    shorthand,
                    name,
                    filepath: path.to_path_buf(),
                }));
            }
        }

        Ok(None)
    }

    /// Extract project name and shorthand from markdown content (from heading)
    fn extract_project_info_from_content(&self, content: &str) -> (Option<String>, Option<String>) {
        // Look for heading like "# MOVE: Move" or "# ADM: Admin things"
        for line in content.lines() {
            if line.starts_with("# ") {
                let heading = line.trim_start_matches("# ").trim();
                // Check if there's a shorthand prefix before colon
                if let Some(colon_pos) = heading.find(':') {
                    let shorthand = heading[..colon_pos].trim().to_string();
                    let name = heading[colon_pos + 1..].trim().to_string();
                    return (Some(shorthand), Some(name));
                } else {
                    return (None, Some(heading.to_string()));
                }
            }
        }
        (None, None)
    }

    /// Generate filename for a new todo
    pub fn generate_filename(&self, name: &str, project_shorthand: Option<&str>) -> String {
        let next_id = self.get_next_id().unwrap_or(1);
        let sanitized_name = self.sanitize_name(name);
        
        if let Some(shorthand) = project_shorthand {
            format!("{}{:03}_{}.md", shorthand, next_id, sanitized_name)
        } else {
            format!("T{:05}_{}.md", next_id, sanitized_name)
        }
    }

    /// Sanitize name for use in filename
    /// Preserves unicode characters but blocks filesystem-problematic characters
    fn sanitize_name(&self, name: &str) -> String {
        name.chars()
            .map(|c| match c {
                // Replace spaces with underscores
                ' ' => '_',
                // Block filesystem-problematic characters
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                // Block control characters and newlines
                c if c.is_control() => '_',
                // Allow everything else (including unicode)
                _ => c,
            })
            .collect::<String>()
            .to_lowercase()
    }

    /// Get the todos directory path
    pub fn get_todos_dir(&self) -> PathBuf {
        self.ted_root.join("todos")
    }

    pub fn get_done_dir(&self) -> PathBuf {
        self.ted_root.join("done")
    }

    /// Format project ID for markdown frontmatter
    pub fn format_project_id(project_id: &str) -> String {
        if project_id.is_empty() {
            "''".to_string()
        } else if project_id.starts_with("[[") && project_id.ends_with("]]") {
            format!("'{}'", project_id)
        } else {
            format!("'[[{}]]'", project_id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_id_from_filename() {
        let fs = FileStorage::new();
        assert_eq!(fs.extract_id_from_filename("ADM106_something.md"), Some(106));
        assert_eq!(fs.extract_id_from_filename("T00061_test.md"), Some(61));
        assert_eq!(fs.extract_id_from_filename("BUY112_new_phone.md"), Some(112));
        assert_eq!(fs.extract_id_from_filename("WGR097_change.md"), Some(97));
        assert_eq!(fs.extract_id_from_filename("ORG110_cleaning.md"), Some(110));
    }

    #[test]
    fn test_sanitize_name() {
        let fs = FileStorage::new();
        assert_eq!(fs.sanitize_name("Hello World"), "hello_world");
        // Unicode characters are preserved
        assert_eq!(fs.sanitize_name("Schöne Grüße"), "schöne_grüße");
        assert_eq!(fs.sanitize_name("Über Äpfel"), "über_äpfel");
        assert_eq!(fs.sanitize_name("Tschüss"), "tschüss");
        // Problematic filesystem characters are blocked
        assert_eq!(fs.sanitize_name("test/path"), "test_path");
        assert_eq!(fs.sanitize_name("file:name"), "file_name");
        assert_eq!(fs.sanitize_name("file*name?"), "file_name_");
    }

    #[test]
    fn test_unicode_handling() {
        let fs = FileStorage::new();
        // Unicode characters are fully preserved in filenames
        assert_eq!(fs.sanitize_name("日本語"), "日本語");
        assert_eq!(fs.sanitize_name("Café ☕"), "café_☕");
        assert_eq!(fs.sanitize_name("Привет мир"), "привет_мир");
    }

    #[test]
    fn test_load_projects_with_shorthands() {
        let fs = FileStorage::new();
        if let Ok(projects) = fs.get_projects() {
            println!("\n=== Projects loaded ===");
            for project in projects.iter().take(5) {
                println!("ID: {}, Shorthand: {:?}, Name: {}", 
                    project.id, project.shorthand, project.name);
            }
            
            // Check if P00002 has ADM shorthand
            if let Some(p2) = projects.iter().find(|p| p.id == "P00002") {
                assert_eq!(p2.shorthand, Some("ADM".to_string()), 
                    "P00002 should have ADM shorthand");
            }
        }
    }
}
