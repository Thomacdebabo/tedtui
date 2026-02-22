# tedtui

# Vibe Coding update
I am finally at the point where I cannot let Claude do more work on this. I am impressed how far it got, but now it is time for me to understand how to actually build tuis myself. 

The code is quite messy, unnecessarily complex and there are way too many lines in the main file. 

At least I know got code that does what I want so I can get more familiar with rust and how it works.

# Overview

A terminal UI for quickly creating and managing Ted todos.

## Features

- **Quick todo creation** - Fill in name, project, goal, tasks, and notes
- **File editing** - Open and edit existing todos by path or ID
- **Task completion** - Toggle task completion with Space key
- **Move to done** - Mark todos as complete and move to done directory with Ctrl+D
- **Project integration** - Browse and select from your Ted projects
- **Smart ID generation** - Automatically generates the next available ID
- **Project shorthands** - Uses project shorthands (WGR, ADM, etc.) in filenames
- **Auto-clear** - Clears form after save for quick consecutive todo creation (new todos only)
- **Full Unicode support** - Type any characters including umlauts (ä, ö, ü), accents, Japanese (日本語), emoji (🎉), and more - preserved in both content and filenames

## Installation
Install rust then:
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
tedtui 1
# if you have fted alias from ted repo
tedtui $(fted) 
```

### Keyboard Shortcuts

- **Tab / Shift+Tab** - Navigate between fields
- **Ctrl+P** - Open project selector (when in Project ID field)
- **Enter** - Add task (when in Tasks input field)
- **Tab** - Move from Tasks input to Task List (to toggle/delete tasks)
- **Space** - Toggle task completion (when in Task List field)
- **↑/↓** - Navigate tasks (when in Task List field) or projects (in project selector)
- **Delete** - Remove selected task (when in Task List field)
- **Ctrl+S** - Save todo (clears form only for new todos)
- **Ctrl+D** - Move todo to done directory (with confirmation for incomplete tasks)
- **Esc / Ctrl+C** - Quit

## Workflow

### Creating a new todo

1. **Name** - Enter the todo name
2. **Project ID** - (Optional) Press Ctrl+P to select a project, or type it manually
3. **Goal** - Enter a short description
4. **Tasks** - Add multiple tasks (press Enter after each, type spaces freely)
5. **Task List** - (Optional) Tab to the task list to toggle/delete tasks with Space/Delete
6. **Note** - Add any additional notes
7. **Save** - Press Ctrl+S to save and start a new todo

### Editing an existing todo

1. **Load** - Run `tedtui <path>` or `tedtui <id>`
2. **Navigate** - Use Tab to move between fields
3. **Edit** - Modify any field as needed
4. **Toggle tasks** - Tab to Task List field, navigate with ↑/↓, press Space to mark complete/incomplete
5. **Save** - Press Ctrl+S to save changes (preserves original ID and creation date)

**Project changes**: If you change the project while editing, the file will be renamed with the new project's shorthand while keeping the same numeric ID. The old file will be deleted automatically.

For example: `WGR115_task.md` → `ADM115_task.md` when changing from project WGR to ADM.

**Task Management**: The Tasks input field is for typing new tasks (you can use spaces freely). Press Tab to move to the Task List field where you can navigate existing tasks with ↑/↓, toggle completion with Space, or delete with Delete.

### Moving a todo to done

1. **Open the todo** - Load it with `tedtui <path>` or `tedtui <id>`
2. **Press Ctrl+D** - Initiate move to done directory
3. **Handle incomplete tasks** - If there are uncompleted tasks:
   - Press **Y** to mark all tasks complete and move
   - Press **N** to cancel the move
4. **Completion** - The file is moved to `~/.ted/done/` with a completion timestamp

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
