//! Output formatting for structured CLI data
//!
//! Provides `format_output()` for rendering structured data in various formats.

use serde::{Deserialize, Serialize};
use serde_json::Value;

const MAX_TABLE_ROWS: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Table,
    Quiet,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskItem {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub state: Option<String>,
}

pub fn format_output(data: &Value, format: OutputFormat) -> String {
    match format {
        OutputFormat::Json => format_json(data),
        OutputFormat::Table => format_table(data),
        OutputFormat::Quiet => format_quiet(data),
    }
}

fn format_json(data: &Value) -> String {
    serde_json::to_string_pretty(data).unwrap_or_else(|_| data.to_string())
}

fn format_table(data: &Value) -> String {
    let tasks = match extract_tasks(data) {
        Some(t) => t,
        None => return "Invalid data format".to_string(),
    };

    if tasks.is_empty() {
        return "No tasks found".to_string();
    }

    let mut output = String::new();
    output.push_str(&format!(
        "{:<20} {:<30} {:<15}",
        "ID", "NAME", "STATE"
    ));
    output.push('\n');
    output.push_str(&"-".repeat(65));
    output.push('\n');

    let display_tasks: Vec<&TaskItem> = tasks.iter().take(MAX_TABLE_ROWS).collect();
    for task in display_tasks {
        output.push_str(&format!(
            "{:<20} {:<30} {:<15}",
            task.id,
            task.name,
            task.state.as_deref().unwrap_or("-")
        ));
        output.push('\n');
    }

    if tasks.len() > MAX_TABLE_ROWS {
        let remaining = tasks.len() - MAX_TABLE_ROWS;
        output.push_str(&format!("... and {} more", remaining));
    }

    output
}

fn format_quiet(data: &Value) -> String {
    let tasks = match extract_tasks(data) {
        Some(t) => t,
        None => return String::new(),
    };

    tasks
        .iter()
        .map(|t| t.id.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_tasks(data: &Value) -> Option<Vec<TaskItem>> {
    let tasks_array = data.get("tasks")?.as_array()?;
    let tasks: Vec<TaskItem> = tasks_array
        .iter()
        .filter_map(|item| {
            serde_json::from_value(item.clone()).ok()
        })
        .collect();
    Some(tasks)
}

pub fn is_valid_json(s: &str) -> bool {
    serde_json::from_str::<Value>(s).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_format_json_with_nested_array_of_tasks() {
        let data = json!({
            "tasks": [
                {"id": "task-1", "name": "Build dashboard", "state": "pending"},
                {"id": "task-2", "name": "Deploy app", "state": "running"},
                {"id": "task-3", "name": "Run tests", "state": "completed"}
            ]
        });

        let output = format_output(&data, OutputFormat::Json);

        assert!(is_valid_json(&output));
        let parsed: Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.get("tasks").is_some());
        let tasks = parsed["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 3);
    }

    #[test]
    fn test_format_table_shows_all_rows() {
        let data = json!({
            "tasks": [
                {"id": "task-1", "name": "First task", "state": "pending"},
                {"id": "task-2", "name": "Second task", "state": "running"},
                {"id": "task-3", "name": "Third task", "state": "completed"}
            ]
        });

        let output = format_output(&data, OutputFormat::Table);

        assert!(output.contains("task-1"));
        assert!(output.contains("task-2"));
        assert!(output.contains("task-3"));
        assert!(output.contains("First task"));
        assert!(output.contains("Second task"));
        assert!(output.contains("Third task"));
        assert!(output.contains("ID"));
        assert!(output.contains("NAME"));
        assert!(output.contains("STATE"));
    }

    #[test]
    fn test_format_json_flag_outputs_valid_json() {
        let data = json!({
            "tasks": [
                {"id": "task-1", "name": "Test task"}
            ]
        });

        let output = format_output(&data, OutputFormat::Json);

        assert!(is_valid_json(&output));
        let parsed: Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.is_object());
    }

    #[test]
    fn test_format_empty_array_outputs_no_tasks_found() {
        let data = json!({
            "tasks": []
        });

        let output = format_output(&data, OutputFormat::Table);

        assert!(output.contains("No tasks found"));
    }

    #[test]
    fn test_format_truncation_for_large_dataset() {
        let mut tasks = Vec::new();
        for i in 0..150 {
            tasks.push(json!({
                "id": format!("task-{}", i),
                "name": format!("Task {}", i),
                "state": "pending"
            }));
        }
        let data = json!({ "tasks": tasks });

        let output = format_output(&data, OutputFormat::Table);

        assert!(output.contains("task-0"));
        assert!(output.contains("... and 50 more"));
        let lines: Vec<&str> = output.lines().collect();
        let data_lines = lines.len();
        assert!(data_lines <= MAX_TABLE_ROWS + 3);
    }

    #[test]
    fn test_format_quiet_outputs_only_ids() {
        let data = json!({
            "tasks": [
                {"id": "task-1", "name": "First"},
                {"id": "task-2", "name": "Second"},
                {"id": "task-3", "name": "Third"}
            ]
        });

        let output = format_output(&data, OutputFormat::Quiet);

        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(output.contains("task-1"));
        assert!(output.contains("task-2"));
        assert!(output.contains("task-3"));
        assert!(!output.contains("First"));
        assert!(!output.contains("Second"));
    }

    #[test]
    fn test_format_table_with_missing_state() {
        let data = json!({
            "tasks": [
                {"id": "task-1", "name": "Test task"}
            ]
        });

        let output = format_output(&data, OutputFormat::Table);

        assert!(output.contains("task-1"));
        assert!(output.contains("Test task"));
        assert!(output.contains("-"));
    }

    #[test]
    fn test_format_invalid_data_returns_error_message() {
        let data = json!({"invalid": "format"});

        let output = format_output(&data, OutputFormat::Table);

        assert!(output.contains("Invalid data format"));
    }

    #[test]
    fn test_is_valid_json_with_valid_json() {
        let json_str = r#"{"tasks": [{"id": "1", "name": "test"}]}"#;
        assert!(is_valid_json(json_str));
    }

    #[test]
    fn test_is_valid_json_with_invalid_json() {
        let invalid_str = "not json";
        assert!(!is_valid_json(invalid_str));
    }

    #[test]
    fn test_format_json_does_not_truncate() {
        let mut tasks = Vec::new();
        for i in 0..150 {
            tasks.push(json!({
                "id": format!("task-{}", i),
                "name": format!("Task {}", i)
            }));
        }
        let data = json!({ "tasks": tasks });

        let output = format_output(&data, OutputFormat::Json);

        let parsed: Value = serde_json::from_str(&output).unwrap();
        let result_tasks = parsed["tasks"].as_array().unwrap();
        assert_eq!(result_tasks.len(), 150);
        assert!(!output.contains("..."));
    }
}