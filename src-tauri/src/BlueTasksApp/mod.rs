use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn tasks_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config/Blue-Environment/blue-tasks")
}
fn lists_path() -> PathBuf { tasks_dir().join("lists.json") }
fn tasks_path() -> PathBuf { tasks_dir().join("tasks.json") }

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TaskList {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub list_id: String,
    pub title: String,
    pub notes: String,
    pub done: bool,
    /// ISO 8601 `YYYY-MM-DD`, or `None` for no due date.
    pub due_date: Option<String>,
    /// `HH:MM` 24h, or `None` — only meaningful alongside `due_date`.
    pub due_time: Option<String>,
    /// 0 = none, 1 = low, 2 = medium, 3 = high — a fixed small scale
    /// (matching the frontend's fixed dropdown), not free-form.
    pub priority: u8,
    /// Set when this task has a matching `BlueCalendarApp` event — see
    /// module doc's "Cross-app integration" section.
    pub linked_event_id: Option<String>,
    /// Set when this task was created via Blue Web's "Save to Blue
    /// Tasks" action — lets the UI show a link-back affordance ("open
    /// the page this came from") without needing a separate table.
    pub source_url: Option<String>,
    pub created_at: String,
}

fn read_lists() -> Vec<TaskList> {
    fs::read_to_string(lists_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}
fn write_lists(lists: &[TaskList]) -> Result<(), String> {
    fs::create_dir_all(tasks_dir()).map_err(|e| e.to_string())?;
    fs::write(lists_path(), serde_json::to_string_pretty(lists).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
fn read_tasks() -> Vec<Task> {
    fs::read_to_string(tasks_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}
fn write_tasks(tasks: &[Task]) -> Result<(), String> {
    fs::create_dir_all(tasks_dir()).map_err(|e| e.to_string())?;
    fs::write(tasks_path(), serde_json::to_string_pretty(tasks).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// Ensures a first-run install has at least one list to put tasks in —
/// same "seed a sensible default" pattern as other first-run app state
/// elsewhere in this codebase, not anything task-specific.
fn ensure_default_list(lists: &mut Vec<TaskList>) -> Vec<TaskList> {
    if lists.is_empty() {
        lists.push(TaskList { id: "inbox".to_string(), name: "Inbox".to_string(), color: "#3b82f6".to_string() });
        let _ = write_lists(lists);
    }
    lists.clone()
}

#[tauri::command]
pub fn tasks_load_lists() -> Vec<TaskList> {
    let mut lists = read_lists();
    ensure_default_list(&mut lists)
}

#[tauri::command]
pub fn tasks_save_list(list: TaskList) -> Result<(), String> {
    let mut lists = read_lists();
    if let Some(existing) = lists.iter_mut().find(|l| l.id == list.id) {
        *existing = list;
    } else {
        lists.push(list);
    }
    write_lists(&lists)
}

#[tauri::command]
pub fn tasks_delete_list(id: String) -> Result<(), String> {
    let mut lists = read_lists();
    lists.retain(|l| l.id != id);
    write_lists(&lists)?;
    // Deleting a list deletes its tasks too — orphaned tasks with no
    // list to belong to would just disappear from every view anyway
    // (nothing renders a task whose `list_id` doesn't resolve), so
    // cleaning them up explicitly here avoids silently-invisible dead
    // data accumulating in tasks.json forever.
    let mut tasks = read_tasks();
    tasks.retain(|t| t.list_id != id);
    write_tasks(&tasks)
}

#[tauri::command]
pub fn tasks_load_tasks() -> Vec<Task> {
    read_tasks()
}

#[tauri::command]
pub fn tasks_upsert(task: Task) -> Result<(), String> {
    let mut tasks = read_tasks();
    if let Some(existing) = tasks.iter_mut().find(|t| t.id == task.id) {
        *existing = task;
    } else {
        tasks.push(task);
    }
    write_tasks(&tasks)
}

#[tauri::command]
pub fn tasks_delete(id: String) -> Result<(), String> {
    let mut tasks = read_tasks();
    tasks.retain(|t| t.id != id);
    write_tasks(&tasks)
}

#[tauri::command]
pub fn tasks_set_done(id: String, done: bool) -> Result<(), String> {
    let mut tasks = read_tasks();
    if let Some(t) = tasks.iter_mut().find(|t| t.id == id) {
        t.done = done;
    }
    write_tasks(&tasks)
}
