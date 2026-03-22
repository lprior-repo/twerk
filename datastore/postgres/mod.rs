//! PostgreSQL datastore implementation.

mod encrypt;
pub mod records;
pub mod schema;

use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Row;
use std::collections::HashMap;
use time::Duration;

use super::{Error as DatastoreError, Page, Result as DatastoreResult};
use records::{
    JobPermRecord, JobRecord, NodeRecord, RoleRecord, ScheduledJobRecord, ScheduledPermRecord,
    TaskLogPartRecord, TaskRecord, UserRecord,
};
use tork::{
    job::{Job, JobSummary,
        ScheduledJob, ScheduledJobSummary},
    task::{Permission, Task, TaskLogPart},
    Node,
    user::User,
    role::Role,
    stats::Metrics,
};

pub use schema::SCHEMA;

/// Default logs retention duration (1 week)
pub const DEFAULT_LOGS_RETENTION_DURATION: Duration = Duration::hours(24 * 7);
/// Default jobs retention duration (1 year)
pub const DEFAULT_JOBS_RETENTION_DURATION: Duration = Duration::hours(24 * 365);
/// Minimum cleanup interval (1 minute)
pub const MIN_CLEANUP_INTERVAL: Duration = Duration::minutes(1);
/// Maximum cleanup interval (1 hour)
pub const MAX_CLEANUP_INTERVAL: Duration = Duration::hours(1);

/// PostgresDatastore is a PostgreSQL implementation of the Datastore trait.
#[derive(Clone)]
pub struct PostgresDatastore {
    pool: PgPool,
    logs_retention_duration: Duration,
    jobs_retention_duration: Duration,
    cleanup_interval: Duration,
    disable_cleanup: bool,
    encryption_key: Option<String>,
}

/// Configuration options for PostgresDatastore
#[derive(Clone)]
pub struct Options {
    pub logs_retention_duration: Duration,
    pub jobs_retention_duration: Duration,
    pub cleanup_interval: Duration,
    pub disable_cleanup: bool,
    pub encryption_key: Option<String>,
    pub max_open_conns: Option<i32>,
    pub max_idle_conns: Option<i32>,
    pub conn_max_lifetime: Option<Duration>,
    pub conn_max_idle_time: Option<Duration>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            logs_retention_duration: DEFAULT_LOGS_RETENTION_DURATION,
            jobs_retention_duration: DEFAULT_JOBS_RETENTION_DURATION,
            cleanup_interval: MIN_CLEANUP_INTERVAL,
            disable_cleanup: false,
            encryption_key: None,
            max_open_conns: None,
            max_idle_conns: None,
            conn_max_lifetime: None,
            conn_max_idle_time: None,
        }
    }
}

impl PostgresDatastore {
    /// Creates a new PostgresDatastore from a connection string.
    pub async fn new(dsn: &str, options: Options) -> DatastoreResult<Self> {
        let mut pool_options = PgPoolOptions::new();
        
        if let Some(max_conns) = options.max_open_conns {
            pool_options = pool_options.max_connections(max_conns as u32);
        }
        if let Some(max_idle) = options.max_idle_conns {
            pool_options = pool_options.min_connections(max_idle as u32);
        }
        
        let pool = pool_options
            .connect(dsn)
            .await
            .map_err(|e| DatastoreError::Database(format!("connection failed: {}", e)))?;

        let cleanup_interval = if options.cleanup_interval < MIN_CLEANUP_INTERVAL {
            return Err(DatastoreError::InvalidInput(
                "cleanup interval cannot be under 1 minute".to_string(),
            ));
        } else {
            options.cleanup_interval
        };

        let logs_retention_duration = if options.logs_retention_duration < MIN_CLEANUP_INTERVAL {
            return Err(DatastoreError::InvalidInput(
                "logs retention period cannot be under 1 minute".to_string(),
            ));
        } else {
            options.logs_retention_duration
        };

        let jobs_retention_duration = if options.jobs_retention_duration < MIN_CLEANUP_INTERVAL {
            return Err(DatastoreError::InvalidInput(
                "jobs retention period cannot be under 1 minute".to_string(),
            ));
        } else {
            options.jobs_retention_duration
        };

        Ok(Self {
            pool,
            logs_retention_duration,
            jobs_retention_duration,
            cleanup_interval,
            disable_cleanup: options.disable_cleanup,
            encryption_key: options.encryption_key,
        })
    }

    /// Executes a SQL script on the database.
    pub async fn exec_script(&self, script: &str) -> DatastoreResult<()> {
        sqlx::query(script)
            .execute(&self.pool)
            .await
            .map_err(|e| DatastoreError::Database(format!("exec script failed: {}", e)))?;
        Ok(())
    }

    /// Creates a new test datastore with a fresh schema.
    #[cfg(test)]
    pub async fn new_test() -> DatastoreResult<Self> {
        let schema_name = format!("tork{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
        let dsn = format!(
            "host=localhost user=tork password=tork dbname=tork search_path={} sslmode=disable",
            schema_name
        );
        
        let ds = Self::new(&dsn, Options::default()).await?;
        
        sqlx::query(&format!("create schema {}", schema_name))
            .execute(&ds.pool)
            .await
            .map_err(|e| DatastoreError::Database(format!("create schema failed: {}", e)))?;
        
        ds.exec_script(SCHEMA).await?;
        
        Ok(ds)
    }

    // Task operations

    /// Creates a new task in the database.
    pub async fn create_task(&self, task: &Task) -> DatastoreResult<()> {
        let env = task.env.as_ref()
            .and_then(|e| serde_json::to_vec(e).ok());
        
        let files = task.files.as_ref()
            .and_then(|f| serde_json::to_vec(f).ok());
        
        let pre = serde_json::to_vec(&task.pre)
            .map_err(|e| DatastoreError::Serialization(format!("task.pre: {}", e)))?;
        
        let post = serde_json::to_vec(&task.post)
            .map_err(|e| DatastoreError::Serialization(format!("task.post: {}", e)))?;
        
        let sidecars = serde_json::to_vec(&task.sidecars)
            .map_err(|e| DatastoreError::Serialization(format!("task.sidecars: {}", e)))?;
        
        let retry = task.retry.as_ref()
            .and_then(|r| serde_json::to_vec(r).ok());
        
        let limits = task.limits.as_ref()
            .and_then(|l| serde_json::to_vec(l).ok());
        
        let parallel = task.parallel.as_ref()
            .and_then(|p| serde_json::to_vec(p).ok());
        
        let each = task.each.as_ref()
            .and_then(|e| serde_json::to_vec(e).ok());
        
        let subjob = task.subjob.as_ref()
            .and_then(|s| serde_json::to_vec(s).ok());
        
        let registry = task.registry.as_ref()
            .and_then(|r| serde_json::to_vec(r).ok());
        
        let mounts = task.mounts.as_ref()
            .and_then(|m| serde_json::to_vec(m).ok());

        let id = task.id.as_ref().ok_or_else(|| DatastoreError::InvalidInput("task id is required".to_string()))?;
        let job_id = task.job_id.as_ref().ok_or_else(|| DatastoreError::InvalidInput("task job_id is required".to_string()))?;

        let cmd = task.cmd.as_ref().map(|c| c.as_slice());
        let entrypoint = task.entrypoint.as_ref().map(|e| e.as_slice());
        let networks = task.networks.as_ref().map(|n| n.as_slice());
        let tags = task.tags.as_ref().map(|t| t.as_slice());

        sqlx::query(
            r#"
            insert into tasks (
                id, job_id, position, name, state, created_at, scheduled_at, started_at,
                completed_at, failed_at, cmd, entrypoint, run_script, image, env, queue,
                error_, pre_tasks, post_tasks, mounts, node_id, retry, limits, timeout,
                var, result, parallel, parent_id, each_, description, subjob, networks,
                files_, registry, gpus, if_, tags, priority, workdir, sidecars
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
                $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29, $30,
                $31, $32, $33, $34, $35, $36, $37, $38, $39, $40)
            "#,
        )
        .bind(id)
        .bind(job_id)
        .bind(task.position)
        .bind(&task.name)
        .bind(task.state.as_ref())
        .bind(task.created_at)
        .bind(task.scheduled_at)
        .bind(task.started_at)
        .bind(task.completed_at)
        .bind(task.failed_at)
        .bind(cmd)
        .bind(entrypoint)
        .bind(&task.run)
        .bind(&task.image)
        .bind(&env)
        .bind(&task.queue)
        .bind(sanitize_string(&task.error))
        .bind(&pre)
        .bind(&post)
        .bind(&mounts)
        .bind(&task.node_id)
        .bind(&retry)
        .bind(&limits)
        .bind(&task.timeout)
        .bind(&task.var)
        .bind(sanitize_string(&task.result))
        .bind(&parallel)
        .bind(&task.parent_id)
        .bind(&each)
        .bind(&task.description)
        .bind(&subjob)
        .bind(networks)
        .bind(&files)
        .bind(&registry)
        .bind(&task.gpus)
        .bind(&task.r#if)
        .bind(tags)
        .bind(task.priority)
        .bind(&task.workdir)
        .bind(&sidecars)
        .execute(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("create task failed: {}", e)))?;

        Ok(())
    }

    /// Gets a task by ID from the database.
    pub async fn get_task_by_id(&self, id: &str) -> DatastoreResult<Task> {
        let record: TaskRecord = sqlx::query_as(
            "SELECT * FROM tasks WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("get task failed: {}", e)))?
        .ok_or(DatastoreError::TaskNotFound)?;

        record.to_task()
    }

    /// Gets active tasks for a job.
    pub async fn get_active_tasks(&self, job_id: &str) -> DatastoreResult<Vec<Task>> {
        let active_states = ["CREATED", "PENDING", "SCHEDULED", "RUNNING"];
        
        let records: Vec<TaskRecord> = sqlx::query_as(
            r#"
            SELECT * FROM tasks 
            WHERE job_id = $1 AND state = ANY($2)
            ORDER BY position, created_at ASC
            "#,
        )
        .bind(job_id)
        .bind(&active_states)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("get active tasks failed: {}", e)))?;

        records
            .iter()
            .map(TaskRecord::to_task)
            .collect()
    }

    /// Gets the next task for execution.
    pub async fn get_next_task(&self, parent_task_id: &str) -> DatastoreResult<Task> {
        let record: TaskRecord = sqlx::query_as(
            "SELECT * FROM tasks WHERE parent_id = $1 AND state = 'CREATED' LIMIT 1",
        )
        .bind(parent_task_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("get next task failed: {}", e)))?
        .ok_or(DatastoreError::TaskNotFound)?;

        record.to_task()
    }

    /// Creates a task log part.
    pub async fn create_task_log_part(&self, part: &TaskLogPart) -> DatastoreResult<()> {
        let task_id = part.task_id.as_ref()
            .ok_or_else(|| DatastoreError::InvalidInput("task_id is required".to_string()))?;
        
        if part.number < 1 {
            return Err(DatastoreError::InvalidInput("part number must be > 0".to_string()));
        }

        let id = uuid::Uuid::new_v4().to_string().replace('-', "");

        sqlx::query(
            r#"
            insert into tasks_log_parts (id, number_, task_id, created_at, contents)
            values ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(&id)
        .bind(part.number)
        .bind(task_id)
        .bind(time::OffsetDateTime::now_utc())
        .bind(&part.contents)
        .execute(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("create task log part failed: {}", e)))?;

        Ok(())
    }

    /// Gets task log parts with pagination.
    pub async fn get_task_log_parts(
        &self,
        task_id: &str,
        q: &str,
        page: i64,
        size: i64,
    ) -> DatastoreResult<Page<TaskLogPart>> {
        let offset = (page - 1) * size;
        
        let (search_term, _tags) = parse_query(q);
        
        let records: Vec<TaskLogPartRecord> = sqlx::query_as(&format!(
            r#"
            SELECT * FROM tasks_log_parts 
            WHERE task_id = $1 AND ($2 = '' OR ts @@ plainto_tsquery('english', $2))
            ORDER BY number_ DESC
            OFFSET {} LIMIT {}
            "#,
            offset, size
        ))
        .bind(task_id)
        .bind(&search_term)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("get task log parts failed: {}", e)))?;

        let items: Vec<TaskLogPart> = records
            .iter()
            .map(TaskLogPartRecord::to_task_log_part)
            .collect();

        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM tasks_log_parts WHERE task_id = $1",
        )
        .bind(task_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("count task log parts failed: {}", e)))?;

        let total_pages = count / size + if count % size != 0 { 1 } else { 0 };

        Ok(Page {
            items,
            number: page,
            size,
            total_pages,
            total_items: count,
        })
    }

    // Node operations

    /// Creates a new node in the database.
    pub async fn create_node(&self, node: &Node) -> DatastoreResult<()> {
        let id = node.id.as_ref().ok_or_else(|| DatastoreError::InvalidInput("node id is required".to_string()))?;

        sqlx::query(
            r#"
            insert into nodes (id, name, started_at, last_heartbeat_at, cpu_percent, queue,
                status, hostname, task_count, version_, port)
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(id)
        .bind(&node.name)
        .bind(node.started_at)
        .bind(node.last_heartbeat_at)
        .bind(node.cpu_percent)
        .bind(&node.queue)
        .bind(&node.status)
        .bind(&node.hostname)
        .bind(node.task_count)
        .bind(&node.version)
        .bind(node.port)
        .execute(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("create node failed: {}", e)))?;

        Ok(())
    }

    /// Gets a node by ID.
    pub async fn get_node_by_id(&self, id: &str) -> DatastoreResult<Node> {
        let record: NodeRecord = sqlx::query_as(
            "SELECT * FROM nodes WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("get node failed: {}", e)))?
        .ok_or(DatastoreError::NodeNotFound)?;

        Ok(record.to_node())
    }

    /// Gets active nodes (with recent heartbeats).
    pub async fn get_active_nodes(&self) -> DatastoreResult<Vec<Node>> {
        let timeout = time::OffsetDateTime::now_utc() - time::Duration::minutes(5);
        
        let records: Vec<NodeRecord> = sqlx::query_as(
            r#"
            SELECT * FROM nodes 
            WHERE last_heartbeat_at > $1
            ORDER BY name ASC
            "#,
        )
        .bind(timeout)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("get active nodes failed: {}", e)))?;

        Ok(records
            .iter()
            .map(NodeRecord::to_node)
            .collect())
    }

    // Job operations

    /// Creates a new job in the database.
    pub async fn create_job(&self, job: &Job) -> DatastoreResult<()> {
        let id = job.id.as_ref().ok_or_else(|| DatastoreError::InvalidInput("job id is required".to_string()))?;

        if job.created_by.is_none() {
            return Err(DatastoreError::InvalidInput("created_by is required".to_string()));
        }
        let created_by_id = job.created_by.as_ref().and_then(|u| u.id.as_ref())
            .ok_or_else(|| DatastoreError::InvalidInput("created_by.id is required".to_string()))?;

        let tasks = serde_json::to_vec(&job.tasks)
            .map_err(|e| DatastoreError::Serialization(format!("job.tasks: {}", e)))?;
        
        let inputs = serde_json::to_vec(&job.inputs)
            .map_err(|e| DatastoreError::Serialization(format!("job.inputs: {}", e)))?;
        
        let context = serde_json::to_vec(&job.context)
            .map_err(|e| DatastoreError::Serialization(format!("job.context: {}", e)))?;
        
        let defaults = job.defaults.as_ref()
            .and_then(|d| serde_json::to_vec(d).ok());
        
        let auto_delete = job.auto_delete.as_ref()
            .and_then(|a| serde_json::to_vec(a).ok());
        
        let webhooks = serde_json::to_vec(&job.webhooks)
            .map_err(|e| DatastoreError::Serialization(format!("job.webhooks: {}", e)))?;

        let tags = job.tags.as_ref().map(|t| t.as_slice()).unwrap_or(&[]);
        let scheduled_job_id = job.schedule.as_ref().and_then(|s| s.id.as_ref());

        let mut secrets: HashMap<String, String> = job.secrets.as_ref()
            .map(|s| s.clone())
            .unwrap_or_default();
        
        if !secrets.is_empty() {
            secrets = encrypt::encrypt_secrets(&secrets, self.encryption_key.as_deref())?;
        }
        let secrets_bytes = if secrets.is_empty() {
            None
        } else {
            serde_json::to_vec(&secrets).ok()
        };

        sqlx::query(
            r#"
            insert into jobs (id, name, description, state, created_at, started_at, tasks,
                position, inputs, context, parent_id, task_count, output_, result, error_,
                defaults, webhooks, created_by, tags, auto_delete, secrets, scheduled_job_id)
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
                $16, $17, $18, $19, $20, $21, $22)
            "#,
        )
        .bind(id)
        .bind(&job.name)
        .bind(&job.description)
        .bind(&job.state)
        .bind(job.created_at)
        .bind(job.started_at)
        .bind(&tasks)
        .bind(job.position)
        .bind(&inputs)
        .bind(&context)
        .bind(&job.parent_id)
        .bind(job.task_count)
        .bind(&job.output)
        .bind(&job.result)
        .bind(&job.error)
        .bind(&defaults)
        .bind(&webhooks)
        .bind(created_by_id)
        .bind(tags)
        .bind(&auto_delete)
        .bind(&secrets_bytes)
        .bind(scheduled_job_id)
        .execute(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("create job failed: {}", e)))?;

        // Insert job permissions
        if let Some(perms) = &job.permissions {
            for perm in perms {
                let (user_id, role_id) = match (&perm.user, &perm.role) {
                    (Some(u), None) => (u.id.clone(), None),
                    (None, Some(r)) => (None, r.id.clone()),
                    _ => continue,
                };
                
                let perm_id = uuid::Uuid::new_v4().to_string().replace('-', "");
                
                sqlx::query(
                    r#"
                    insert into jobs_perms (id, job_id, user_id, role_id)
                    values ($1, $2, $3, $4)
                    "#,
                )
                .bind(&perm_id)
                .bind(id)
                .bind(&user_id)
                .bind(&role_id)
                .execute(&self.pool)
                .await
                .map_err(|e| DatastoreError::Database(format!("create job perm failed: {}", e)))?;
            }
        }

        Ok(())
    }

    /// Gets a job by ID.
    pub async fn get_job_by_id(&self, id: &str) -> DatastoreResult<Job> {
        let record: JobRecord = sqlx::query_as(
            "SELECT * FROM jobs WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("get job failed: {}", e)))?
        .ok_or(DatastoreError::JobNotFound)?;

        // Get tasks for this job
        let tasks: Vec<Task> = vec![]; // TODO: Load tasks if needed
        
        // Get execution (completed tasks)
        let execution_records: Vec<TaskRecord> = sqlx::query_as(
            r#"
            SELECT * FROM tasks 
            WHERE job_id = $1 
            ORDER BY position, started_at ASC
            "#,
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("get job tasks failed: {}", e)))?;

        let execution: Vec<Task> = execution_records
            .iter()
            .filter_map(|r| r.to_task().ok())
            .collect();

        // Get created_by user
        let user = self.get_user(&record.created_by).await?;
        
        // Get permissions
        let perm_records: Vec<JobPermRecord> = sqlx::query_as(
            "SELECT * FROM jobs_perms WHERE job_id = $1",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("get job perms failed: {}", e)))?;

        let mut perms = Vec::new();
        for pr in perm_records {
            if let Some(role_id) = &pr.role_id {
                if let Ok(role) = self.get_role(role_id).await {
                    perms.push(Permission {
                        role: Some(role),
                        user: None,
                    });
                }
            } else if let Some(user_id) = &pr.user_id {
                if let Ok(user) = self.get_user(user_id).await {
                    perms.push(Permission {
                        role: None,
                        user: Some(user),
                    });
                }
            }
        }

        record.to_job(tasks, execution, user, perms, self.encryption_key.as_deref())
    }

    /// Gets job log parts with pagination.
    pub async fn get_job_log_parts(
        &self,
        job_id: &str,
        q: &str,
        page: i64,
        size: i64,
    ) -> DatastoreResult<Page<TaskLogPart>> {
        let offset = (page - 1) * size;
        
        let (search_term, _tags) = parse_query(q);
        
        let records: Vec<TaskLogPartRecord> = sqlx::query_as(&format!(
            r#"
            SELECT tlp.* FROM tasks_log_parts tlp
            JOIN tasks t ON t.id = tlp.task_id
            WHERE t.job_id = $1 AND ($2 = '' OR tlp.ts @@ plainto_tsquery('english', $2))
            ORDER BY t.position desc, t.created_at desc, tlp.number_ desc, tlp.created_at DESC
            OFFSET {} LIMIT {}
            "#,
            offset, size
        ))
        .bind(job_id)
        .bind(&search_term)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("get job log parts failed: {}", e)))?;

        let items: Vec<TaskLogPart> = records
            .iter()
            .map(TaskLogPartRecord::to_task_log_part)
            .collect();

        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT count(*) FROM tasks_log_parts tlp
            JOIN tasks t ON t.id = tlp.task_id
            WHERE t.job_id = $1
            "#,
        )
        .bind(job_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("count job log parts failed: {}", e)))?;

        let total_pages = count / size + if count % size != 0 { 1 } else { 0 };

        Ok(Page {
            items,
            number: page,
            size,
            total_pages,
            total_items: count,
        })
    }

    /// Gets jobs with pagination and filtering.
    pub async fn get_jobs(
        &self,
        current_user: &str,
        q: &str,
        page: i64,
        size: i64,
    ) -> DatastoreResult<Page<JobSummary>> {
        let offset = (page - 1) * size;
        let (search_term, tags) = parse_query(q);

        // Complex query with CTE for permissions
        let query = format!(
            r#"
            WITH user_info AS (
                SELECT id AS user_id FROM users WHERE username_ = $3
            ),
            role_info AS (
                SELECT role_id FROM users_roles ur
                JOIN user_info ui ON ur.user_id = ui.user_id
            ),
            job_perms_info AS (
                SELECT job_id FROM jobs_perms jp
                WHERE jp.user_id = (SELECT user_id FROM user_info)
                OR jp.role_id IN (SELECT role_id FROM role_info)
            ),
            no_job_perms AS (
                SELECT j.id as job_id FROM jobs j
                WHERE NOT EXISTS (SELECT 1 FROM jobs_perms jp WHERE j.id = jp.job_id)
            )
            SELECT j.* FROM jobs j
            WHERE 
                ($1 = '' OR ts @@ plainto_tsquery('english', $1))
            AND 
                (array_length($2::text[], 1) IS NULL OR j.tags && $2)
            AND
                ($3 = '' OR EXISTS (SELECT 1 FROM no_job_perms njp WHERE njp.job_id=j.id) 
                    OR EXISTS (SELECT 1 FROM job_perms_info jpi WHERE jpi.job_id = j.id))
            ORDER BY created_at DESC 
            OFFSET {} LIMIT {}
            "#,
            offset, size
        );

        let records: Vec<JobRecord> = sqlx::query_as(&query)
            .bind(&search_term)
            .bind(&tags)
            .bind(&current_user)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DatastoreError::Database(format!("get jobs failed: {}", e)))?;

        let mut result = Vec::new();
        for record in records {
            if let Ok(user) = self.get_user(&record.created_by).await {
                if let Ok(job) = record.to_job(
                    vec![],
                    vec![],
                    user,
                    vec![],
                    self.encryption_key.as_deref(),
                ) {
                    result.push(tork::job::new_job_summary(&job));
                }
            }
        }

        // Count query
        let count_query = format!(
            r#"
            WITH user_info AS (
                SELECT id AS user_id FROM users WHERE username_ = $3
            ),
            role_info AS (
                SELECT role_id FROM users_roles ur
                JOIN user_info ui ON ur.user_id = ui.user_id
            ),
            job_perms_info AS (
                SELECT job_id FROM jobs_perms jp
                WHERE jp.user_id = (SELECT user_id FROM user_info)
                OR jp.role_id IN (SELECT role_id FROM role_info)
            ),
            no_job_perms AS (
                SELECT j.id as job_id FROM jobs j
                WHERE NOT EXISTS (SELECT 1 FROM jobs_perms jp WHERE j.id = jp.job_id)
            )
            SELECT count(*) FROM jobs j
            WHERE 
                ($1 = '' OR ts @@ plainto_tsquery('english', $1))
            AND 
                (array_length($2::text[], 1) IS NULL OR j.tags && $2)
            AND
                ($3 = '' OR EXISTS (SELECT 1 FROM no_job_perms njp WHERE njp.job_id=j.id) 
                    OR EXISTS (SELECT 1 FROM job_perms_info jpi WHERE jpi.job_id = j.id))
            "#,
        );

        let count: i64 = sqlx::query_scalar(&count_query)
            .bind(&search_term)
            .bind(&tags)
            .bind(&current_user)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DatastoreError::Database(format!("count jobs failed: {}", e)))?;

        let total_pages = count / size + if count % size != 0 { 1 } else { 0 };
        let result_size = result.len() as i64;

        Ok(Page {
            items: result,
            number: page,
            size: result_size,
            total_pages,
            total_items: count,
        })
    }

    // Scheduled job operations

    /// Creates a new scheduled job.
    pub async fn create_scheduled_job(&self, sj: &ScheduledJob) -> DatastoreResult<()> {
        let id = sj.id.as_ref().ok_or_else(|| DatastoreError::InvalidInput("scheduled job id is required".to_string()))?;

        if sj.created_by.is_none() {
            return Err(DatastoreError::InvalidInput("created_by is required".to_string()));
        }
        let created_by_id = sj.created_by.as_ref().and_then(|u| u.id.as_ref())
            .ok_or_else(|| DatastoreError::InvalidInput("created_by.id is required".to_string()))?;

        let tasks = serde_json::to_vec(&sj.tasks)
            .map_err(|e| DatastoreError::Serialization(format!("scheduled_job.tasks: {}", e)))?;
        
        let inputs = serde_json::to_vec(&sj.inputs)
            .map_err(|e| DatastoreError::Serialization(format!("scheduled_job.inputs: {}", e)))?;
        
        let defaults = sj.defaults.as_ref()
            .and_then(|d| serde_json::to_vec(d).ok());
        
        let auto_delete = sj.auto_delete.as_ref()
            .and_then(|a| serde_json::to_vec(a).ok());
        
        let webhooks = serde_json::to_vec(&sj.webhooks)
            .map_err(|e| DatastoreError::Serialization(format!("scheduled_job.webhooks: {}", e)))?;

        let tags = sj.tags.as_ref().map(|t| t.as_slice()).unwrap_or(&[]);

        let mut secrets: HashMap<String, String> = sj.secrets.as_ref()
            .map(|s| s.clone())
            .unwrap_or_default();
        
        if !secrets.is_empty() {
            secrets = encrypt::encrypt_secrets(&secrets, self.encryption_key.as_deref())?;
        }
        let secrets_bytes = if secrets.is_empty() {
            None
        } else {
            serde_json::to_vec(&secrets).ok()
        };

        sqlx::query(
            r#"
            insert into scheduled_jobs (id, name, description, cron_expr, state, tasks, inputs,
                defaults, webhooks, auto_delete, secrets, created_by, tags, created_at, output_)
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            "#,
        )
        .bind(id)
        .bind(&sj.name)
        .bind(&sj.description)
        .bind(&sj.cron)
        .bind(&sj.state)
        .bind(&tasks)
        .bind(&inputs)
        .bind(&defaults)
        .bind(&webhooks)
        .bind(&auto_delete)
        .bind(&secrets_bytes)
        .bind(created_by_id)
        .bind(tags)
        .bind(sj.created_at)
        .bind(&sj.output)
        .execute(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("create scheduled job failed: {}", e)))?;

        Ok(())
    }

    /// Gets active scheduled jobs.
    pub async fn get_active_scheduled_jobs(&self) -> DatastoreResult<Vec<ScheduledJob>> {
        let records: Vec<ScheduledJobRecord> = sqlx::query_as(
            "SELECT * FROM scheduled_jobs WHERE state = 'ACTIVE'",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("get active scheduled jobs failed: {}", e)))?;

        let mut result = Vec::new();
        for record in records {
            let tasks: Vec<Task> = serde_json::from_slice(&record.tasks)
                .map_err(|e| DatastoreError::Serialization(format!("scheduled_job.tasks: {}", e)))?;
            
            let user = self.get_user(&record.created_by).await?;
            
            let sj = record.to_scheduled_job(tasks, user, vec![], self.encryption_key.as_deref())?;
            result.push(sj);
        }

        Ok(result)
    }

    /// Gets scheduled jobs with pagination.
    pub async fn get_scheduled_jobs(
        &self,
        current_user: &str,
        page: i64,
        size: i64,
    ) -> DatastoreResult<Page<ScheduledJobSummary>> {
        let offset = (page - 1) * size;

        let records: Vec<ScheduledJobRecord> = sqlx::query_as(&format!(
            r#"
            WITH user_info AS (
                SELECT id AS user_id FROM users WHERE username_ = $1
            ),
            role_info AS (
                SELECT role_id FROM users_roles ur
                JOIN user_info ui ON ur.user_id = ui.user_id
            ),
            job_perms_info AS (
                SELECT scheduled_job_id FROM scheduled_jobs_perms jp
                WHERE jp.user_id = (SELECT user_id FROM user_info)
                OR jp.role_id IN (SELECT role_id FROM role_info)
            ),
            no_job_perms AS (
                SELECT j.id as scheduled_job_id FROM scheduled_jobs j
                WHERE NOT EXISTS (SELECT 1 FROM scheduled_jobs_perms jp WHERE j.id = jp.scheduled_job_id)
            )
            SELECT j.* FROM scheduled_jobs j
            WHERE ($1 = '' OR EXISTS (SELECT 1 FROM no_job_perms njp WHERE njp.scheduled_job_id=j.id) 
                OR EXISTS (SELECT 1 FROM job_perms_info jpi WHERE jpi.scheduled_job_id = j.id))
            ORDER BY created_at DESC 
            OFFSET {} LIMIT {}
            "#,
            offset, size
        ))
        .bind(&current_user)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("get scheduled jobs failed: {}", e)))?;

        let mut result = Vec::new();
        for record in records {
            let tasks: Vec<Task> = serde_json::from_slice(&record.tasks)
                .map_err(|e| DatastoreError::Serialization(format!("scheduled_job.tasks: {}", e)))?;
            
            let user = self.get_user(&record.created_by).await?;
            
            let sj = record.to_scheduled_job(tasks, user, vec![], self.encryption_key.as_deref())?;
            result.push(tork::job::new_scheduled_job_summary(&sj));
        }

        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM scheduled_jobs",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("count scheduled jobs failed: {}", e)))?;

        let total_pages = count / size + if count % size != 0 { 1 } else { 0 };
        let result_size = result.len() as i64;

        Ok(Page {
            items: result,
            number: page,
            size: result_size,
            total_pages,
            total_items: count,
        })
    }

    /// Gets a scheduled job by ID.
    pub async fn get_scheduled_job_by_id(&self, id: &str) -> DatastoreResult<ScheduledJob> {
        let record: ScheduledJobRecord = sqlx::query_as(
            "SELECT * FROM scheduled_jobs WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("get scheduled job failed: {}", e)))?
        .ok_or(DatastoreError::ScheduledJobNotFound)?;

        let tasks: Vec<Task> = serde_json::from_slice(&record.tasks)
            .map_err(|e| DatastoreError::Serialization(format!("scheduled_job.tasks: {}", e)))?;
        
        let user = self.get_user(&record.created_by).await?;

        // Get permissions
        let perm_records: Vec<ScheduledPermRecord> = sqlx::query_as(
            "SELECT * FROM scheduled_jobs_perms WHERE scheduled_job_id = $1",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("get scheduled job perms failed: {}", e)))?;

        let mut perms = Vec::new();
        for pr in perm_records {
            if let Some(role_id) = &pr.role_id {
                if let Ok(role) = self.get_role(role_id).await {
                    perms.push(Permission {
                        role: Some(role),
                        user: None,
                    });
                }
            } else if let Some(user_id) = &pr.user_id {
                if let Ok(user) = self.get_user(user_id).await {
                    perms.push(Permission {
                        role: None,
                        user: Some(user),
                    });
                }
            }
        }

        record.to_scheduled_job(tasks, user, perms, self.encryption_key.as_deref())
    }

    // User operations

    /// Creates a new user.
    pub async fn create_user(&self, user: &User) -> DatastoreResult<()> {
        let id = user.id.as_ref().ok_or_else(|| DatastoreError::InvalidInput("user id is required".to_string()))?;
        let created_at = user.created_at.unwrap_or_else(|| time::OffsetDateTime::now_utc());

        sqlx::query(
            r#"
            insert into users (id, name, username_, password_, created_at, is_disabled)
            values ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(id)
        .bind(&user.name)
        .bind(&user.username)
        .bind(&user.password_hash)
        .bind(created_at)
        .bind(user.disabled)
        .execute(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("create user failed: {}", e)))?;

        Ok(())
    }

    /// Gets a user by username or ID.
    pub async fn get_user(&self, uid: &str) -> DatastoreResult<User> {
        let record: UserRecord = sqlx::query_as(
            "SELECT * FROM users WHERE username_ = $1 OR id = $1",
        )
        .bind(uid)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("get user failed: {}", e)))?
        .ok_or(DatastoreError::UserNotFound)?;

        Ok(record.to_user())
    }

    // Role operations

    /// Creates a new role.
    pub async fn create_role(&self, role: &Role) -> DatastoreResult<()> {
        let id = role.id.as_ref().ok_or_else(|| DatastoreError::InvalidInput("role id is required".to_string()))?;
        let created_at = role.created_at.unwrap_or_else(|| time::OffsetDateTime::now_utc());

        sqlx::query(
            r#"
            insert into roles (id, slug, name, created_at)
            values ($1, $2, $3, $4)
            "#,
        )
        .bind(id)
        .bind(&role.slug)
        .bind(&role.name)
        .bind(created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("create role failed: {}", e)))?;

        Ok(())
    }

    /// Gets a role by ID or slug.
    pub async fn get_role(&self, id: &str) -> DatastoreResult<Role> {
        let record: RoleRecord = sqlx::query_as(
            "SELECT * FROM roles WHERE id = $1 OR slug = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("get role failed: {}", e)))?
        .ok_or(DatastoreError::RoleNotFound)?;

        Ok(record.to_role())
    }

    /// Gets all roles.
    pub async fn get_roles(&self) -> DatastoreResult<Vec<Role>> {
        let records: Vec<RoleRecord> = sqlx::query_as(
            "SELECT * FROM roles ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("get roles failed: {}", e)))?;

        Ok(records
            .iter()
            .map(RoleRecord::to_role)
            .collect())
    }

    /// Gets roles assigned to a user.
    pub async fn get_user_roles(&self, user_id: &str) -> DatastoreResult<Vec<Role>> {
        let records: Vec<RoleRecord> = sqlx::query_as(
            r#"
            SELECT r.* FROM roles r
            INNER JOIN users_roles ur ON ur.role_id = r.id
            WHERE ur.user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("get user roles failed: {}", e)))?;

        Ok(records
            .iter()
            .map(RoleRecord::to_role)
            .collect())
    }

    /// Assigns a role to a user.
    pub async fn assign_role(&self, user_id: &str, role_id: &str) -> DatastoreResult<()> {
        let id = uuid::Uuid::new_v4().to_string().replace('-', "");

        sqlx::query(
            r#"
            insert into users_roles (id, user_id, role_id, created_at)
            values ($1, $2, $3, $4)
            "#,
        )
        .bind(&id)
        .bind(user_id)
        .bind(role_id)
        .bind(time::OffsetDateTime::now_utc())
        .execute(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("assign role failed: {}", e)))?;

        Ok(())
    }

    /// Unassigns a role from a user.
    pub async fn unassign_role(&self, user_id: &str, role_id: &str) -> DatastoreResult<()> {
        sqlx::query(
            "delete from users_roles where user_id = $1 and role_id = $2",
        )
        .bind(user_id)
        .bind(role_id)
        .execute(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("unassign role failed: {}", e)))?;

        Ok(())
    }

    // Metrics

    /// Gets system metrics.
    pub async fn get_metrics(&self) -> DatastoreResult<Metrics> {
        let jobs_running: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM jobs WHERE state = 'RUNNING'",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("get jobs running failed: {}", e)))?;

        let tasks_running: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM tasks t JOIN jobs j ON t.job_id = j.id WHERE t.state = 'RUNNING' AND j.state = 'RUNNING'",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("get tasks running failed: {}", e)))?;

        let nodes_running: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM nodes WHERE last_heartbeat_at > current_timestamp - interval '5 minutes'",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("get nodes running failed: {}", e)))?;

        let nodes_cpu: f64 = sqlx::query_scalar(
            "SELECT coalesce(avg(cpu_percent),0) FROM nodes WHERE last_heartbeat_at > current_timestamp - interval '5 minutes'",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DatastoreError::Database(format!("get nodes cpu failed: {}", e)))?;

        Ok(Metrics {
            jobs: tork::stats::JobMetrics {
                running: jobs_running,
            },
            tasks: tork::stats::TaskMetrics {
                running: tasks_running,
            },
            nodes: tork::stats::NodeMetrics {
                running: nodes_running,
                cpu_percent: nodes_cpu,
            },
        })
    }

    /// Health check.
    pub async fn health_check(&self) -> DatastoreResult<()> {
        sqlx::query("select 1")
            .execute(&self.pool)
            .await
            .map_err(|e| DatastoreError::Database(format!("health check failed: {}", e)))?;
        Ok(())
    }
}

/// Sanitizes a string by removing null characters.
fn sanitize_string(s: &Option<String>) -> Option<String> {
    s.as_ref().map(|s| s.replace('\u{0}', ""))
}

/// Parses a query string into search term and tags.
fn parse_query(query: &str) -> (String, Vec<String>) {
    let mut terms = Vec::new();
    let mut tags = Vec::new();
    
    for part in query.split_whitespace() {
        if part.starts_with("tag:") {
            tags.push(part.trim_start_matches("tag:").to_string());
        } else if part.starts_with("tags:") {
            for tag in part.trim_start_matches("tags:").split(',') {
                tags.push(tag.to_string());
            }
        } else {
            terms.push(part.to_string());
        }
    }
    
    (terms.join(" "), tags)
}
