mod filestorage;
use filestorage::FileStorage;

fn main() {
    let fs = FileStorage::new();
    match fs.get_projects() {
        Ok(projects) => {
            println!("Found {} projects:", projects.len());
            for project in projects.iter().take(10) {
                println!("  ID: {}, Shorthand: {:?}, Name: {}", 
                    project.id, 
                    project.shorthand, 
                    project.name
                );
            }
        }
        Err(e) => {
            println!("Error loading projects: {}", e);
        }
    }
}
