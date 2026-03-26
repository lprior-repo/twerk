use serde::{Deserialize, Serialize};

/// Metrics holds aggregated metrics across all twerk components.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    #[serde(rename = "jobs")]
    pub jobs: JobMetrics,
    #[serde(rename = "tasks")]
    pub tasks: TaskMetrics,
    #[serde(rename = "nodes")]
    pub nodes: NodeMetrics,
}

/// JobMetrics holds metrics about twerk jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobMetrics {
    #[serde(rename = "running")]
    pub running: i32,
}

/// TaskMetrics holds metrics about twerk tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskMetrics {
    #[serde(rename = "running")]
    pub running: i32,
}

/// NodeMetrics holds metrics about twerk nodes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NodeMetrics {
    #[serde(rename = "online")]
    pub running: i32,
    #[serde(rename = "cpuPercent")]
    pub cpu_percent: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_roundtrip() {
        let metrics = Metrics {
            jobs: JobMetrics { running: 2 },
            tasks: TaskMetrics { running: 5 },
            nodes: NodeMetrics {
                running: 3,
                cpu_percent: 45.5,
            },
        };

        let json = serde_json::to_string(&metrics).expect("serialization must succeed");
        let parsed: Metrics = serde_json::from_str(&json).expect("deserialization must succeed");

        assert_eq!(parsed.jobs.running, 2);
        assert_eq!(parsed.tasks.running, 5);
        assert_eq!(parsed.nodes.running, 3);
        assert!((parsed.nodes.cpu_percent - 45.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_job_metrics_json() {
        let job = JobMetrics { running: 1 };
        let json = serde_json::to_string(&job).expect("serialization must succeed");
        assert_eq!(json, r#"{"running":1}"#);
    }

    #[test]
    fn test_task_metrics_json() {
        let task = TaskMetrics { running: 10 };
        let json = serde_json::to_string(&task).expect("serialization must succeed");
        assert_eq!(json, r#"{"running":10}"#);
    }

    #[test]
    fn test_node_metrics_json() {
        let node = NodeMetrics {
            running: 4,
            cpu_percent: 82.7,
        };
        let json = serde_json::to_string(&node).expect("serialization must succeed");
        assert_eq!(json, r#"{"online":4,"cpuPercent":82.7}"#);
    }

    #[test]
    fn test_deserialize_from_json() {
        let json = r#"{"running":7}"#;
        let task: TaskMetrics = serde_json::from_str(json).expect("deserialization must succeed");
        assert_eq!(task.running, 7);
    }

    #[test]
    fn test_node_metrics_deserialize() {
        let json = r#"{"online":2,"cpuPercent":50.0}"#;
        let node: NodeMetrics = serde_json::from_str(json).expect("deserialization must succeed");
        assert_eq!(node.running, 2);
        assert!((node.cpu_percent - 50.0).abs() < f64::EPSILON);
    }
}
