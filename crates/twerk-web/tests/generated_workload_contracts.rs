//! Behavioral contracts for generated YAML workload scenarios.

use std::collections::HashMap;

use twerk_core::job::Job;
use twerk_core::mount::{Mount, MOUNT_TYPE_VOLUME};
use twerk_core::task::{EachTask, ParallelTask, Task};
use twerk_web::api::yaml::from_slice;

const RUNS_PER_SCENARIO: usize = 30;
const CONFIDENCE_Z_SCORE_95: f64 = 1.96;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkloadScenario {
    SimpleEcho,
    SimpleFileWrite,
    SimpleLoop,
    MultiTask,
    EnvVars,
    Volumes,
    Parallel4Tasks,
    Parallel8Tasks,
    Parallel16Tasks,
    Each10Items,
    Each25Items,
    Each50Items,
    MixedJobTypes,
    HighVariability,
    SustainedBurst,
}

fn generate_workload(scenario: WorkloadScenario, seed: u64, iteration: u64) -> String {
    let variant = seed.wrapping_add(iteration);
    let job_id = iteration;

    match scenario {
        WorkloadScenario::SimpleEcho => {
            let messages = ["hello", "world", "test", "data", "job", "task"];
            let message = messages[(variant as usize) % messages.len()];
            format!(
                "name: echo-job-{job_id}\ntasks:\n  - name: echo\n    image: bash:latest\n    cmd: [\"echo\", \"{message}\"]\n"
            )
        }
        WorkloadScenario::SimpleFileWrite => {
            let path = format!("/tmp/file_{}", variant % 100);
            format!(
                "name: io-job-{job_id}\ntasks:\n  - name: write-read\n    image: bash:latest\n    run: echo test > {path} && cat {path}\n"
            )
        }
        WorkloadScenario::SimpleLoop => {
            let count = 10 + (variant % 90);
            format!(
                "name: loop-job-{job_id}\ntasks:\n  - name: loop\n    image: bash:latest\n    run: i=0; while [ $i -lt {count} ]; do i=$((i+1)); done\n"
            )
        }
        WorkloadScenario::MultiTask => format!(
            "name: multitask-job-{job_id}\ntasks:\n  - name: task1\n    image: bash:latest\n    cmd: [\"echo\", \"task1\"]\n  - name: task2\n    image: bash:latest\n    cmd: [\"echo\", \"task2\"]\n  - name: task3\n    image: bash:latest\n    run: echo task3\n  - name: task4\n    image: bash:latest\n    cmd: [\"pwd\"]\n"
        ),
        WorkloadScenario::EnvVars => {
            let env_value = variant % 100;
            format!(
                "name: env-job-{job_id}\ntasks:\n  - name: env-task\n    image: bash:latest\n    env:\n      ENV_0: value_{env_value}\n      ENV_1: value_{}\n      ENV_2: value_{}\n    cmd: [\"env\"]\n",
                env_value + 1,
                env_value + 2
            )
        }
        WorkloadScenario::Volumes => format!(
            "name: vol-job-{job_id}\ntasks:\n  - name: vol-task\n    image: bash:latest\n    mounts:\n      - type: volume\n        source: data-{}\n        target: /data\n    cmd: [\"ls\", \"-la\", \"/data\"]\n",
            variant % 10
        ),
        WorkloadScenario::Parallel4Tasks => generate_parallel_yaml(job_id, 4, variant),
        WorkloadScenario::Parallel8Tasks => generate_parallel_yaml(job_id, 8, variant),
        WorkloadScenario::Parallel16Tasks => generate_parallel_yaml(job_id, 16, variant),
        WorkloadScenario::Each10Items => generate_each_yaml(job_id, 10),
        WorkloadScenario::Each25Items => generate_each_yaml(job_id, 25),
        WorkloadScenario::Each50Items => generate_each_yaml(job_id, 50),
        WorkloadScenario::MixedJobTypes => format!(
            "name: mixed-job-{job_id}\ntasks:\n  - name: bootstrap\n    image: bash:latest\n    cmd: [\"echo\", \"seed-{variant}\"]\n  - name: fanout\n    parallel:\n      tasks:\n        - name: p0\n          image: bash:latest\n          cmd: [\"echo\", \"p0\"]\n        - name: p1\n          image: bash:latest\n          cmd: [\"echo\", \"p1\"]\n  - name: iterate\n    each:\n      list: \"{{{{ sequence(1,3) }}}}\"\n      task:\n        name: iter-task\n        image: bash:latest\n        env:\n          ITEM: \"{{{{item_value}}}}\"\n        run: echo -n $ITEM > $TWERK_OUTPUT\n"
        ),
        WorkloadScenario::HighVariability => format!(
            "name: variable-job-{job_id}\ntasks:\n  - name: env-task\n    image: bash:latest\n    env:\n      SEED: '{seed}'\n      ITERATION: '{iteration}'\n    cmd: [\"env\"]\n  - name: mounted\n    image: bash:latest\n    mounts:\n      - type: volume\n        source: cache-{}\n        target: /cache\n    cmd: [\"ls\", \"/cache\"]\n",
            variant % 5
        ),
        WorkloadScenario::SustainedBurst => {
            let count = 8 + (variant % 16) as usize;
            generate_parallel_yaml(job_id, count, variant)
        }
    }
}

fn generate_parallel_yaml(job_id: u64, count: usize, variant: u64) -> String {
    let mut tasks = String::new();
    for index in 0..count {
        let command = match (variant as usize + index) % 4 {
            0 => format!("[\"echo\", \"parallel-{index}\"]"),
            1 => "[\"date\"]".to_string(),
            2 => "[\"pwd\"]".to_string(),
            _ => format!("[\"echo\", \"seed-{variant}\"]"),
        };
        tasks.push_str(&format!(
            "        - name: parallel-task-{index}\n          image: bash:latest\n          cmd: {command}\n"
        ));
    }

    format!(
        "name: parallel-job-{job_id}\ntasks:\n  - name: parallel-root\n    parallel:\n      tasks:\n{tasks}"
    )
}

fn generate_each_yaml(job_id: u64, item_count: usize) -> String {
    format!(
        "name: each-job-{job_id}\ntasks:\n  - name: each-root\n    each:\n      list: \"{{{{ sequence(1,{item_count}) }}}}\"\n      var: item\n      task:\n        name: each-task\n        image: bash:latest\n        env:\n          ITEM: \"{{{{item_value}}}}\"\n        run: echo -n $ITEM > $TWERK_OUTPUT\n"
    )
}

#[derive(Debug, Clone, PartialEq)]
struct GeneratedWorkloadStats {
    scenario: WorkloadScenario,
    samples: Vec<f64>,
    mean: f64,
    std_dev: f64,
    variance: f64,
    min: f64,
    max: f64,
    ci_lower: f64,
    ci_upper: f64,
    p50: f64,
    p95: f64,
    p99: f64,
}

impl GeneratedWorkloadStats {
    fn new(scenario: WorkloadScenario, samples: Vec<f64>) -> Self {
        if samples.is_empty() {
            return Self {
                scenario,
                samples,
                mean: 0.0,
                std_dev: 0.0,
                variance: 0.0,
                min: 0.0,
                max: 0.0,
                ci_lower: 0.0,
                ci_upper: 0.0,
                p50: 0.0,
                p95: 0.0,
                p99: 0.0,
            };
        }

        let count = samples.len() as f64;
        let mean = samples.iter().sum::<f64>() / count;
        let variance = samples.iter().map(|sample| (sample - mean).powi(2)).sum::<f64>() / count;
        let std_dev = variance.sqrt();
        let ci_margin = CONFIDENCE_Z_SCORE_95 * std_dev / count.sqrt();

        let mut sorted = samples.clone();
        sorted.sort_by(f64::total_cmp);

        let p50 = percentile(&sorted, 0.50);
        let p95 = percentile(&sorted, 0.95);
        let p99 = percentile(&sorted, 0.99);
        let min = sorted[0];
        let max = sorted[sorted.len() - 1];

        Self {
            scenario,
            samples,
            mean,
            std_dev,
            variance,
            min,
            max,
            ci_lower: mean - ci_margin,
            ci_upper: mean + ci_margin,
            p50,
            p95,
            p99,
        }
    }

    fn coefficient_of_variation(&self) -> f64 {
        if self.mean == 0.0 {
            return 0.0;
        }

        (self.std_dev / self.mean.abs()) * 100.0
    }
}

fn percentile(sorted: &[f64], quantile: f64) -> f64 {
    let last_index = sorted.len() - 1;
    let index = ((sorted.len() as f64) * quantile) as usize;
    sorted[index.min(last_index)]
}

fn run_scenario(scenario: WorkloadScenario, seed: u64) -> GeneratedWorkloadStats {
    let samples = (0..RUNS_PER_SCENARIO)
        .map(|run| {
            let yaml = generate_workload(scenario, seed, run as u64);
            let job = parse_job(&yaml);
            deterministic_sample_value(&yaml, &job)
        })
        .collect();

    GeneratedWorkloadStats::new(scenario, samples)
}

fn parse_job(yaml: &str) -> Job {
    match from_slice::<Job>(yaml.as_bytes()) {
        Ok(job) => job,
        Err(error) => panic!("expected generated YAML to parse, got {error:?}\nYAML:\n{yaml}"),
    }
}

fn deterministic_sample_value(yaml: &str, job: &Job) -> f64 {
    (yaml.len() as u64 + structural_weight(job)) as f64
}

fn structural_weight(job: &Job) -> u64 {
    let top_level_tasks = job.tasks.as_ref().map_or(0_u64, |tasks| tasks.len() as u64);
    let nested_parallel_tasks = job
        .tasks
        .as_ref()
        .map_or(0_u64, |tasks| tasks.iter().map(task_parallel_count).sum::<u64>());
    let each_tasks = job
        .tasks
        .as_ref()
        .map_or(0_u64, |tasks| tasks.iter().map(task_each_count).sum::<u64>());
    let env_vars = job
        .tasks
        .as_ref()
        .map_or(0_u64, |tasks| tasks.iter().map(task_env_count).sum::<u64>());
    let mounts = job
        .tasks
        .as_ref()
        .map_or(0_u64, |tasks| tasks.iter().map(task_mount_count).sum::<u64>());
    let name_len = job.name.as_ref().map_or(0_u64, |name| name.len() as u64);

    top_level_tasks * 100 + nested_parallel_tasks * 10 + each_tasks * 7 + env_vars * 5 + mounts * 3 + name_len
}

fn task_parallel_count(task: &Task) -> u64 {
    task.parallel
        .as_ref()
        .and_then(|parallel| parallel.tasks.as_ref())
        .map_or(0_u64, |tasks| tasks.len() as u64)
}

fn task_each_count(task: &Task) -> u64 {
    if task.each.is_some() { 1 } else { 0 }
}

fn task_env_count(task: &Task) -> u64 {
    task.env.as_ref().map_or(0_u64, |env| env.len() as u64)
}

fn task_mount_count(task: &Task) -> u64 {
    task.mounts.as_ref().map_or(0_u64, |mounts| mounts.len() as u64)
}

fn job_tasks(job: &Job) -> &[Task] {
    match job.tasks.as_deref() {
        Some(tasks) => tasks,
        None => panic!("expected job to include tasks: {job:?}"),
    }
}

fn only_task(job: &Job) -> &Task {
    let tasks = job_tasks(job);
    assert_eq!(tasks.len(), 1);
    &tasks[0]
}

fn nested_parallel_tasks(task: &Task) -> &[Task] {
    match task.parallel.as_ref().and_then(|parallel| parallel.tasks.as_deref()) {
        Some(tasks) => tasks,
        None => panic!("expected task to include nested parallel tasks: {task:?}"),
    }
}

fn task_names(tasks: &[Task]) -> Vec<&str> {
    tasks
        .iter()
        .map(|task| match task.name.as_deref() {
            Some(name) => name,
            None => panic!("expected task to include a name: {task:?}"),
        })
        .collect()
}

fn assert_close(actual: f64, expected: f64) {
    let difference = (actual - expected).abs();
    assert!(
        difference < 1e-9,
        "expected {expected}, got {actual}, difference {difference}"
    );
}

#[cfg(test)]
mod generated_workload_contracts {
    use super::*;

    fn parse_generated_job(scenario: WorkloadScenario, seed: u64, iteration: u64) -> Job {
        let yaml = generate_workload(scenario, seed, iteration);
        match from_slice::<Job>(yaml.as_bytes()) {
            Ok(job) => job,
            Err(error) => panic!("expected generated YAML to parse, got {error:?}\nYAML:\n{yaml}"),
        }
    }

    fn bash_task_with_cmd(name: &str, command: &[&str]) -> Task {
        Task {
            name: Some(name.to_string()),
            image: Some("bash:latest".to_string()),
            cmd: Some(command.iter().map(|part| (*part).to_string()).collect()),
            ..Task::default()
        }
    }

    fn bash_task_with_run(name: &str, script: &str) -> Task {
        Task {
            name: Some(name.to_string()),
            image: Some("bash:latest".to_string()),
            run: Some(script.to_string()),
            ..Task::default()
        }
    }

    fn env_map(entries: &[(&str, &str)]) -> HashMap<String, String> {
        entries
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    fn job_with_tasks(name: &str, tasks: Vec<Task>) -> Job {
        Job {
            name: Some(name.to_string()),
            tasks: Some(tasks),
            ..Job::default()
        }
    }

    fn parallel_root_task(tasks: Vec<Task>) -> Task {
        Task {
            name: Some("parallel-root".to_string()),
            parallel: Some(ParallelTask {
                tasks: Some(tasks),
                ..ParallelTask::default()
            }),
            ..Task::default()
        }
    }

    fn parallel_task_name(index: usize) -> String {
        format!("parallel-task-{index}")
    }

    fn expected_parallel_tasks_4() -> Vec<Task> {
        vec![
            bash_task_with_cmd(&parallel_task_name(0), &["date"]),
            bash_task_with_cmd(&parallel_task_name(1), &["pwd"]),
            bash_task_with_cmd(&parallel_task_name(2), &["echo", "seed-49"]),
            bash_task_with_cmd(&parallel_task_name(3), &["echo", "parallel-3"]),
        ]
    }

    fn expected_parallel_tasks_16() -> Vec<Task> {
        vec![
            bash_task_with_cmd(&parallel_task_name(0), &["date"]),
            bash_task_with_cmd(&parallel_task_name(1), &["pwd"]),
            bash_task_with_cmd(&parallel_task_name(2), &["echo", "seed-49"]),
            bash_task_with_cmd(&parallel_task_name(3), &["echo", "parallel-3"]),
            bash_task_with_cmd(&parallel_task_name(4), &["date"]),
            bash_task_with_cmd(&parallel_task_name(5), &["pwd"]),
            bash_task_with_cmd(&parallel_task_name(6), &["echo", "seed-49"]),
            bash_task_with_cmd(&parallel_task_name(7), &["echo", "parallel-7"]),
            bash_task_with_cmd(&parallel_task_name(8), &["date"]),
            bash_task_with_cmd(&parallel_task_name(9), &["pwd"]),
            bash_task_with_cmd(&parallel_task_name(10), &["echo", "seed-49"]),
            bash_task_with_cmd(&parallel_task_name(11), &["echo", "parallel-11"]),
            bash_task_with_cmd(&parallel_task_name(12), &["date"]),
            bash_task_with_cmd(&parallel_task_name(13), &["pwd"]),
            bash_task_with_cmd(&parallel_task_name(14), &["echo", "seed-49"]),
            bash_task_with_cmd(&parallel_task_name(15), &["echo", "parallel-15"]),
        ]
    }

    fn expected_simple_file_write_job() -> Job {
        job_with_tasks(
            "io-job-7",
            vec![bash_task_with_run(
                "write-read",
                "echo test > /tmp/file_49 && cat /tmp/file_49",
            )],
        )
    }

    fn expected_simple_loop_job() -> Job {
        job_with_tasks(
            "loop-job-7",
            vec![bash_task_with_run(
                "loop",
                "i=0; while [ $i -lt 59 ]; do i=$((i+1)); done",
            )],
        )
    }

    fn expected_multitask_job() -> Job {
        job_with_tasks(
            "multitask-job-7",
            vec![
                bash_task_with_cmd("task1", &["echo", "task1"]),
                bash_task_with_cmd("task2", &["echo", "task2"]),
                bash_task_with_run("task3", "echo task3"),
                bash_task_with_cmd("task4", &["pwd"]),
            ],
        )
    }

    fn expected_parallel_4_job() -> Job {
        job_with_tasks("parallel-job-7", vec![parallel_root_task(expected_parallel_tasks_4())])
    }

    fn expected_parallel_16_job() -> Job {
        job_with_tasks(
            "parallel-job-7",
            vec![parallel_root_task(expected_parallel_tasks_16())],
        )
    }

    fn expected_env_vars_job() -> Job {
        job_with_tasks(
            "env-job-7",
            vec![Task {
                name: Some("env-task".to_string()),
                image: Some("bash:latest".to_string()),
                env: Some(env_map(&[
                    ("ENV_0", "value_49"),
                    ("ENV_1", "value_50"),
                    ("ENV_2", "value_51"),
                ])),
                cmd: Some(vec!["env".to_string()]),
                ..Task::default()
            }],
        )
    }

    fn expected_volumes_job() -> Job {
        job_with_tasks(
            "vol-job-7",
            vec![Task {
                name: Some("vol-task".to_string()),
                image: Some("bash:latest".to_string()),
                mounts: Some(vec![Mount {
                    mount_type: Some(MOUNT_TYPE_VOLUME.to_string()),
                    source: Some("data-9".to_string()),
                    target: Some("/data".to_string()),
                    ..Mount::default()
                }]),
                cmd: Some(vec!["ls".to_string(), "-la".to_string(), "/data".to_string()]),
                ..Task::default()
            }],
        )
    }

    fn expected_each_10_job() -> Job {
        job_with_tasks(
            "each-job-7",
            vec![Task {
                name: Some("each-root".to_string()),
                each: Some(Box::new(EachTask {
                    var: Some("item".to_string()),
                    list: Some("{{ sequence(1,10) }}".to_string()),
                    task: Some(Box::new(Task {
                        name: Some("each-task".to_string()),
                        image: Some("bash:latest".to_string()),
                        env: Some(env_map(&[("ITEM", "{{item_value}}")])),
                        run: Some("echo -n $ITEM > $TWERK_OUTPUT".to_string()),
                        ..Task::default()
                    })),
                    ..EachTask::default()
                })),
                ..Task::default()
            }],
        )
    }

    fn expected_mixed_job_types_job() -> Job {
        job_with_tasks(
            "mixed-job-7",
            vec![
                bash_task_with_cmd("bootstrap", &["echo", "seed-49"]),
                Task {
                    name: Some("fanout".to_string()),
                    parallel: Some(ParallelTask {
                        tasks: Some(vec![
                            bash_task_with_cmd("p0", &["echo", "p0"]),
                            bash_task_with_cmd("p1", &["echo", "p1"]),
                        ]),
                        ..ParallelTask::default()
                    }),
                    ..Task::default()
                },
                Task {
                    name: Some("iterate".to_string()),
                    each: Some(Box::new(EachTask {
                        list: Some("{{ sequence(1,3) }}".to_string()),
                        task: Some(Box::new(Task {
                            name: Some("iter-task".to_string()),
                            image: Some("bash:latest".to_string()),
                            env: Some(env_map(&[("ITEM", "{{item_value}}")])),
                            run: Some("echo -n $ITEM > $TWERK_OUTPUT".to_string()),
                            ..Task::default()
                        })),
                        ..EachTask::default()
                    })),
                    ..Task::default()
                },
            ],
        )
    }

    fn expected_sample_values(scenario: WorkloadScenario, seed: u64) -> Vec<f64> {
        (0..RUNS_PER_SCENARIO)
            .map(|run| {
                let yaml = generate_workload(scenario, seed, run as u64);
                let job = parse_job(&yaml);
                deterministic_sample_value(&yaml, &job)
            })
            .collect()
    }

    #[test]
    fn simple_file_write_parses_with_exact_structural_parity() {
        let job = parse_generated_job(WorkloadScenario::SimpleFileWrite, 42, 7);

        assert_eq!(job, expected_simple_file_write_job());
    }

    #[test]
    fn simple_loop_parses_with_exact_structural_parity() {
        let job = parse_generated_job(WorkloadScenario::SimpleLoop, 42, 7);

        assert_eq!(job, expected_simple_loop_job());
    }

    #[test]
    fn multi_task_parses_with_exact_structural_parity() {
        let job = parse_generated_job(WorkloadScenario::MultiTask, 42, 7);

        assert_eq!(job, expected_multitask_job());
    }

    #[test]
    fn parallel_four_tasks_parses_with_exact_structural_parity() {
        let job = parse_generated_job(WorkloadScenario::Parallel4Tasks, 42, 7);

        assert_eq!(job, expected_parallel_4_job());
    }

    #[test]
    fn parallel_sixteen_tasks_parses_with_exact_structural_parity() {
        let job = parse_generated_job(WorkloadScenario::Parallel16Tasks, 42, 7);

        assert_eq!(job, expected_parallel_16_job());
    }

    #[test]
    fn env_vars_scenario_parses_to_the_expected_job_contract() {
        let job = parse_generated_job(WorkloadScenario::EnvVars, 42, 7);

        assert_eq!(job, expected_env_vars_job());
    }

    #[test]
    fn volumes_scenario_parses_to_the_expected_job_contract() {
        let job = parse_generated_job(WorkloadScenario::Volumes, 42, 7);

        assert_eq!(job, expected_volumes_job());
    }

    #[test]
    fn each_scenario_parses_to_the_expected_job_contract() {
        let job = parse_generated_job(WorkloadScenario::Each10Items, 42, 7);

        assert_eq!(job, expected_each_10_job());
    }

    #[test]
    fn mixed_job_types_scenario_parses_to_the_expected_job_contract() {
        let job = parse_generated_job(WorkloadScenario::MixedJobTypes, 42, 7);

        assert_eq!(job, expected_mixed_job_types_job());
    }

    #[test]
    fn generated_workload_stats_preserve_exact_single_sample_statistics() {
        let result = GeneratedWorkloadStats::new(WorkloadScenario::SimpleEcho, vec![42.0]);

        assert_eq!(
            result,
            GeneratedWorkloadStats {
                scenario: WorkloadScenario::SimpleEcho,
                samples: vec![42.0],
                mean: 42.0,
                std_dev: 0.0,
                variance: 0.0,
                min: 42.0,
                max: 42.0,
                ci_lower: 42.0,
                ci_upper: 42.0,
                p50: 42.0,
                p95: 42.0,
                p99: 42.0,
            }
        );
    }

    #[test]
    fn generated_workload_stats_use_the_explicit_empty_sample_zero_contract() {
        let result = GeneratedWorkloadStats::new(WorkloadScenario::SimpleEcho, Vec::new());

        assert_eq!(
            result,
            GeneratedWorkloadStats {
                scenario: WorkloadScenario::SimpleEcho,
                samples: Vec::new(),
                mean: 0.0,
                std_dev: 0.0,
                variance: 0.0,
                min: 0.0,
                max: 0.0,
                ci_lower: 0.0,
                ci_upper: 0.0,
                p50: 0.0,
                p95: 0.0,
                p99: 0.0,
            }
        );
    }

    #[test]
    fn generated_workload_stats_use_known_percentiles_variance_and_confidence_interval() {
        let result = GeneratedWorkloadStats::new(
            WorkloadScenario::SimpleEcho,
            vec![10.0, 20.0, 30.0, 40.0],
        );

        assert_eq!(result.scenario, WorkloadScenario::SimpleEcho);
        assert_eq!(result.samples, vec![10.0, 20.0, 30.0, 40.0]);
        assert_eq!(result.mean, 25.0);
        assert_eq!(result.min, 10.0);
        assert_eq!(result.max, 40.0);
        assert_eq!(result.p50, 30.0);
        assert_eq!(result.p95, 40.0);
        assert_eq!(result.p99, 40.0);
        assert_close(result.variance, 125.0);
        assert_close(result.std_dev, 11.180_339_887_498_949);
        assert_close(result.ci_lower, 14.043_266_910_251_03);
        assert_close(result.ci_upper, 35.956_733_089_748_97);
    }

    #[test]
    fn coefficient_of_variation_returns_zero_for_zero_variance_samples() {
        let result = GeneratedWorkloadStats::new(WorkloadScenario::SimpleEcho, vec![7.0, 7.0, 7.0]);

        assert_eq!(result.coefficient_of_variation(), 0.0);
    }

    #[test]
    fn coefficient_of_variation_returns_the_known_percentage_for_known_samples() {
        let result = GeneratedWorkloadStats::new(
            WorkloadScenario::SimpleEcho,
            vec![10.0, 20.0, 30.0, 40.0],
        );

        assert_close(result.coefficient_of_variation(), 44.721_359_549_995_796);
    }

    #[test]
    fn run_scenario_returns_the_requested_scenario_and_the_expected_sample_series() {
        let result = run_scenario(WorkloadScenario::MultiTask, 42);

        assert_eq!(result.scenario, WorkloadScenario::MultiTask);
        assert_eq!(result.samples, expected_sample_values(WorkloadScenario::MultiTask, 42));
        assert_eq!(result.samples.len(), RUNS_PER_SCENARIO);
    }

    #[test]
    fn run_scenario_is_reproducible_for_the_same_seed_and_scenario() {
        let first = run_scenario(WorkloadScenario::SimpleEcho, 42);
        let second = run_scenario(WorkloadScenario::SimpleEcho, 42);

        assert_eq!(first, second);
    }

    #[test]
    fn run_scenario_changes_the_sample_series_when_the_seed_changes() {
        let first = run_scenario(WorkloadScenario::SimpleEcho, 42);
        let second = run_scenario(WorkloadScenario::SimpleEcho, 99);

        assert_ne!(first.samples, second.samples);
    }

    #[test]
    fn run_scenario_produces_meaningfully_different_results_for_different_scenarios() {
        let simple_result = run_scenario(WorkloadScenario::SimpleEcho, 42);
        let parallel_result = run_scenario(WorkloadScenario::Parallel16Tasks, 42);

        assert_ne!(simple_result.samples, parallel_result.samples);
        assert_ne!(simple_result.mean, parallel_result.mean);
        assert_eq!(
            parallel_result.samples,
            expected_sample_values(WorkloadScenario::Parallel16Tasks, 42)
        );
    }

    #[test]
    fn additional_generated_scenarios_continue_to_parse_through_the_public_yaml_api() {
        let parallel_job = parse_generated_job(WorkloadScenario::Parallel8Tasks, 42, 7);
        let each_25_job = parse_generated_job(WorkloadScenario::Each25Items, 42, 7);
        let each_50_job = parse_generated_job(WorkloadScenario::Each50Items, 42, 7);
        let variability_job = parse_generated_job(WorkloadScenario::HighVariability, 42, 7);
        let burst_job = parse_generated_job(WorkloadScenario::SustainedBurst, 42, 7);

        assert_eq!(nested_parallel_tasks(only_task(&parallel_job)).len(), 8);
        assert_eq!(
            only_task(&each_25_job)
                .each
                .as_ref()
                .and_then(|each| each.list.as_deref()),
            Some("{{ sequence(1,25) }}")
        );
        assert_eq!(
            only_task(&each_50_job)
                .each
                .as_ref()
                .and_then(|each| each.list.as_deref()),
            Some("{{ sequence(1,50) }}")
        );
        assert_eq!(task_names(job_tasks(&variability_job)), vec!["env-task", "mounted"]);
        assert_eq!(nested_parallel_tasks(only_task(&burst_job)).len(), 9);
    }
}
