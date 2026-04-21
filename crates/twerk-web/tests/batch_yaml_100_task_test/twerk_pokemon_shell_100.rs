use twerk_core::job::Job;
use twerk_web::api::yaml::from_slice;

const TWERK_POKEMON_SHELL_100_YAML: &str =
    include_str!("../../../../examples/twerk-pokemon-shell-100.yaml");

fn parse_pokemon_job() -> Job {
    from_slice(TWERK_POKEMON_SHELL_100_YAML.as_bytes()).unwrap()
}

fn pokemon_tasks() -> Vec<twerk_core::task::Task> {
    parse_pokemon_job().tasks.expect("tasks")
}

#[tokio::test]
async fn parse_twerk_pokemon_shell_100_yaml_success() {
    let job = parse_pokemon_job();

    assert_eq!(job.name.as_deref(), Some("twerk-pokemon-shell-stress"));
    assert_eq!(
        job.description.as_deref(),
        Some("Stress test twerk calling Pokemon API via shell")
    );
}

#[tokio::test]
async fn parse_twerk_pokemon_shell_100_has_exactly_100_tasks() {
    assert_eq!(pokemon_tasks().len(), 100);
}

#[tokio::test]
async fn parse_twerk_pokemon_shell_100_all_tasks_have_names() {
    pokemon_tasks()
        .iter()
        .enumerate()
        .for_each(|(index, task)| {
            let name = task.name.as_ref().expect("task name");
            assert!(
                name.starts_with("fetch-"),
                "Task {index} should start with fetch-"
            );
        });
}

#[tokio::test]
async fn parse_twerk_pokemon_shell_100_all_tasks_have_images() {
    pokemon_tasks()
        .iter()
        .enumerate()
        .for_each(|(index, task)| {
            let image = task.image.as_ref().expect("task image");
            assert_eq!(
                image, "ubuntu:mantic",
                "Task {index} should use ubuntu:mantic"
            );
        });
}

#[tokio::test]
async fn parse_twerk_pokemon_shell_100_all_tasks_have_run_commands() {
    pokemon_tasks()
        .iter()
        .enumerate()
        .for_each(|(index, task)| {
            let run = task.run.as_ref().expect("task run");
            assert!(run.contains("curl"), "Task {index} should contain curl");
            assert!(
                run.contains("/api/pokemon/"),
                "Task {index} should contain pokemon path"
            );
        });
}

#[tokio::test]
async fn parse_twerk_pokemon_shell_100_task_names_are_sequential() {
    pokemon_tasks()
        .iter()
        .enumerate()
        .for_each(|(index, task)| {
            let expected_suffix = format!("{:03}", index + 1);
            let name = task.name.as_ref().expect("task name");
            assert!(
                name.ends_with(&expected_suffix),
                "Task {index} should end with {expected_suffix}"
            );
        });
}

#[tokio::test]
async fn parse_twerk_pokemon_shell_100_pokemon_ids_range_from_1_to_100() {
    pokemon_tasks()
        .iter()
        .enumerate()
        .for_each(|(index, task)| {
            let pokemon_id = index + 1;
            let run = task.run.as_ref().expect("task run");
            assert!(
                run.contains(&format!("/api/pokemon/{pokemon_id}")),
                "Task {index} should target pokemon {pokemon_id}"
            );
        });
}
