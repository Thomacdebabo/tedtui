// ============================================================================
// Module Imports and Dependencies
// ============================================================================

mod app;
mod filestorage;
mod markdown;
mod parser;
mod theme;
mod ui;
mod utils;

use app::{App, InputField, JsonInput, run_app};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use std::io;
use utils::find_file_by_id;

// ============================================================================
// Main Entry Point
// ============================================================================

fn main() -> Result<(), io::Error> {
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();

    let app = if args.len() > 1 {
        let arg = &args[1];

        // Check for --json flag
        if arg == "--json" {
            if args.len() < 3 {
                eprintln!("Usage: tedtui --json '<json_string>'");
                eprintln!();
                JsonInput::print_schema();
                eprintln!();
                eprintln!("Example:");
                eprintln!(
                    "  tedtui --json '{{\"name\":\"My Task\",\"tasks\":[\"Step 1\",\"Step 2\"]}}'\n"
                );
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Missing JSON string",
                ));
            }
            let json_str = &args[2];
            match App::from_json(json_str) {
                Ok(app) => app,
                Err(e) => {
                    eprintln!("Error parsing JSON: {}", e);
                    return Err(e);
                }
            }
        }
        // Check if it's a file path
        else if arg.ends_with(".md") {
            match App::from_file(arg) {
                Ok(app) => app,
                Err(e) => {
                    eprintln!("Error loading file: {}", e);
                    return Err(e);
                }
            }
        }
        // Check if it's a number (ID)
        else if let Ok(id_num) = arg.parse::<u32>() {
            if let Some(filepath) = find_file_by_id(id_num) {
                match App::from_file(&filepath.to_string_lossy()) {
                    Ok(app) => app,
                    Err(e) => {
                        eprintln!("Error loading file: {}", e);
                        return Err(e);
                    }
                }
            } else {
                eprintln!("No file found with ID: {}", id_num);
                return Err(io::Error::new(io::ErrorKind::NotFound, "File not found"));
            }
        }
        // Invalid argument
        else {
            eprintln!("Usage: tedtui [--json '<json_string>'|file.md|ID]");
            eprintln!();
            eprintln!("Options:");
            eprintln!("  --json '<json_string>'  Create todo from JSON input");
            eprintln!("  file.md                 Load existing markdown file");
            eprintln!("  ID                      Load todo by numeric ID");
            eprintln!();
            JsonInput::print_schema();
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid argument",
            ));
        }
    } else {
        App::new()
    };

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the app
    let res = run_app(&mut terminal, app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {:?}", err);
    }

    Ok(())
}
