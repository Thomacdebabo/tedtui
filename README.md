# tedtui

A terminal UI for quickly creating and managing Ted todos.

## Features

- **Quick todo creation** - Fill in name, project, goal, tasks, and notes
- **File editing** - Open and edit existing todos by path or ID
- **Task completion** - Toggle task completion with Space key
- **Project integration** - Browse and select from your Ted projects
- **Smart ID generation** - Automatically generates the next available ID
- **Project shorthands** - Uses project shorthands (WGR, ADM, etc.) in filenames
- **Auto-clear** - Clears form after save for quick consecutive todo creation (new todos only)

## Installation

```bash
./install.sh
```

This will:
1. Build the release version
2. Install `tedtui` to `~/.local/bin` or `/usr/local/bin`
3. Make it executable

## Usage

### Creating a new todo

Run the application:

```bash
tedtui
```

### Editing an existing todo

Load a todo by file path:

```bash
tedtui ~/.ted/todos/T00001_my_todo.md
```

Or by numeric ID (searches todos and done folders):

```bash
tedtui 1
```

### Keyboard Shortcuts

- **Tab / Shift+Tab** - Navigate between fields
- **Ctrl+P** - Open project selector (when in Project ID field)
- **Enter** - Add task (when in Tasks field)
- **Space** - Toggle task completion (when a task is selected)
- **↑/↓** - Navigate tasks or projects
- **Delete** - Remove selected task
- **Ctrl+S** - Save todo (clears form only for new todos)
- **Esc / Ctrl+C** - Quit

## Workflow

### Creating a new todo

1. **Name** - Enter the todo name
2. **Project ID** - (Optional) Press Ctrl+P to select a project, or type it manually
3. **Goal** - Enter a short description
4. **Tasks** - Add multiple tasks (press Enter after each)
5. **Note** - Add any additional notes
6. **Save** - Press Ctrl+S to save and start a new todo

### Editing an existing todo

1. **Load** - Run `tedtui <path>` or `tedtui <id>`
2. **Navigate** - Use Tab to move between fields
3. **Edit** - Modify any field as needed
4. **Toggle tasks** - Navigate to tasks with ↑/↓ and press Space to mark complete/incomplete
5. **Save** - Press Ctrl+S to save changes (preserves original ID and creation date)

## File Output

Todos are saved to `~/.ted/todos/` with the format:
- With project: `<SHORTHAND><ID>_<name>.md` (e.g., `WGR115_my_todo.md`)
- Without project: `T<ID>_<name>.md` (e.g., `T00119_my_todo.md`)

## Uninstallation

```bash
./uninstall.sh
```

## Development

Build and run:

```bash
cargo run
```

Run tests:

```bash
cargo test
```
