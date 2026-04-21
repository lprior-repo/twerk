use twerk_core::job::Job;
use twerk_web::api::yaml::from_slice;

const TWERK_NOOP_100_YAML: &str = include_str!("../../../../examples/twerk-noop-100.yaml");

fn parse_noop_job() -> Job {
    from_slice(TWERK_NOOP_100_YAML.as_bytes()).unwrap()
}

fn noop_tasks() -> Vec<twerk_core::task::Task> {
    parse_noop_job().tasks.expect("tasks")
}

#[tokio::test]
async fn parse_twerk_noop_100_yaml_success() {
    let job = parse_noop_job();

    assert_eq!(job.name.as_deref(), Some("twerk-noop-stress"));
    assert_eq!(
        job.description.as_deref(),
        Some("Stress test twerk with no-op tasks (no containers, no API calls)")
    );
}

#[tokio::test]
async fn parse_twerk_noop_100_has_exactly_100_tasks() {
    assert_eq!(noop_tasks().len(), 100);
}

#[tokio::test]
async fn parse_twerk_noop_100_all_tasks_have_names() {
    noop_tasks().iter().enumerate().for_each(|(index, task)| {
        let name = task.name.as_ref().expect("task name");
        assert!(
            name.starts_with("noop-"),
            "Task {index} should start with noop-"
        );
    });
}

#[tokio::test]
async fn parse_twerk_noop_100_all_tasks_have_run_commands() {
    noop_tasks().iter().enumerate().for_each(|(index, task)| {
        let run = task.run.as_ref().expect("task run");
        assert!(run.contains("echo"), "Task {index} should contain echo");
    });
}

#[tokio::test]
async fn parse_twerk_noop_100_task_names_are_sequential() {
    noop_tasks().iter().enumerate().for_each(|(index, task)| {
        let expected_suffix = format!("{:03}", index + 1);
        let name = task.name.as_ref().expect("task name");
        assert!(
            name.ends_with(&expected_suffix),
            "Task {index} should end with {expected_suffix}"
        );
    });
}

#[tokio::test]
async fn parse_twerk_noop_100_no_tasks_have_images() {
    noop_tasks().iter().enumerate().for_each(|(index, task)| {
        assert!(task.image.is_none(), "Task {index} should not have image")
    });
}

#[tokio::test]
async fn parse_twerk_noop_100_job_has_no_id_before_submission() {
    let job = parse_noop_job();

    assert!(job.id.is_none());
    assert!(job.created_at.is_none());
}
