use crate::filestorage;
use unicode_width::UnicodeWidthStr;

use filestorage::FileStorage;
use std::fs;
use std::path::PathBuf;
// ============================================================================
// Utility Helper Functions
// ============================================================================

fn find_file_by_id_in_directory(dir: &PathBuf, id_num: u32) -> Option<PathBuf> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();
            if let Some(extracted_id) = extract_id_from_filename(&filename) {
                if extracted_id == id_num {
                    return Some(entry.path());
                }
            }
        }
    }
    None
}
pub fn find_file_by_id(id_num: u32) -> Option<PathBuf> {
    let file_storage = FileStorage::new();
    let todos_dir = file_storage.get_todos_dir();
    let done_dir = file_storage.get_done_dir();

    // Try to find file in todos directory
    if let Some(filepath) = find_file_by_id_in_directory(&todos_dir, id_num) {
        return Some(filepath);
    }

    // Also check done directory
    if let Some(filepath) = find_file_by_id_in_directory(&done_dir, id_num) {
        return Some(filepath);
    }

    None
}

fn extract_id_from_filename(filename: &str) -> Option<u32> {
    use regex::Regex;
    let re = Regex::new(r"^[A-Z]+(\d+)_").ok()?;
    re.captures(filename)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse::<u32>().ok())
}

pub fn count_display_lines_with_wrapping(text: &str, available_width: usize) -> usize {
    if available_width == 0 {
        return text.lines().count();
    }

    let mut total = 0;
    for line in text.lines() {
        let line_width = line.width();

        total += (line_width / available_width) + 1;
    }
    total.max(1)
}
