//! `PostgreSQL` schema for the datastore.

/// The SQL schema for initializing the database.
///
/// **Note:** This schema is tuned for compatibility with `sqlx` 0.7 + Rust types:
/// - `timestamptz` instead of `timestamp` (matches `time::OffsetDateTime`)
/// - `bytea` instead of `jsonb` for JSON payloads (sqlx binds `Vec<u8>` as `bytea`)
/// - `bigint` instead of `integer` for counters (matches Rust `i64`)
/// - `double precision` for progress (matches Rust `f64`)
pub const SCHEMA: &str = r"
CREATE TABLE nodes (
    id                 varchar(32)  not null primary key,
    name               varchar(64)  not null,
    queue              varchar(64)  not null,
    started_at         timestamptz  not null,
    last_heartbeat_at  timestamptz  not null,
    cpu_percent        float        not null,
    status             varchar(10)  not null,
    hostname           varchar(128) not null,
    port               bigint       not null,
    task_count         bigint       not null,
    version_           varchar(32)  not null
);

CREATE INDEX idx_nodes_heartbeat ON nodes (last_heartbeat_at);

CREATE TABLE users (
    id          varchar(32)  not null primary key,
    name        varchar(64)  not null,
    username_   varchar(64)  not null unique,
    password_   varchar(256) not null,
    created_at  timestamptz  not null,
    is_disabled boolean      not null default false
);

insert into users (id,name,username_,password_,created_at,is_disabled) (SELECT REPLACE(gen_random_uuid()::text, '-', ''),'Guest','guest','',current_timestamp,true);

CREATE TABLE roles (
    id          varchar(32)  not null primary key,
    name        varchar(64)  not null,
    slug        varchar(64)  not null unique,
    created_at  timestamptz  not null
);

CREATE UNIQUE INDEX idx_roles_slug ON roles (slug);

insert into roles (id,name,slug,created_at) (SELECT REPLACE(gen_random_uuid()::text, '-', ''),'Public','public',current_timestamp);

CREATE TABLE users_roles (
    id         varchar(32) not null primary key,
    user_id    varchar(32) not null references users(id),
    role_id    varchar(32) not null references roles(id),
    created_at timestamptz not null
);

CREATE UNIQUE INDEX idx_users_roles_uniq ON users_roles (user_id,role_id);

CREATE TABLE scheduled_jobs (
  id             varchar(32) not null primary key,
  name           varchar(64) not null,
  description    text        not null,
  tags           text[]      not null default '{}',
  cron_expr      varchar(64) not null,
  inputs         bytea       not null,
  output_        text        not null,
  tasks          bytea       not null,
  defaults       bytea,
  webhooks       bytea,
  auto_delete    bytea,
  secrets        bytea,
  created_at     timestamptz not null,
  created_by     varchar(32) not null references users(id),
  state          varchar(10) not null
);

CREATE TABLE scheduled_jobs_perms (
    id               varchar(32) not null primary key,
    scheduled_job_id varchar(32) not null references scheduled_jobs(id),
    user_id          varchar(32)          references users(id),
    role_id          varchar(32)          references roles(id)
);

CREATE TABLE jobs (
    id               varchar(32) not null primary key,
    name             varchar(256),
    tags             text[]      not null default '{}',
    state            varchar(10) not null,
    created_at       timestamptz not null,
    created_by       varchar(32) not null references users(id),
    started_at       timestamptz,
    completed_at     timestamptz,
    delete_at        timestamptz,
    failed_at        timestamptz,
    tasks            bytea       not null,
    position         bigint      not null,
    inputs           bytea       not null,
    context          bytea       not null,
    description      text,
    parent_id        varchar(32),
    task_count       bigint      not null,
    output_          text,
    result           text,
    error_           text,
    defaults         bytea,
    webhooks         bytea,
    auto_delete      bytea,
    secrets          bytea,
    progress         double precision default 0,
    scheduled_job_id varchar(32) references scheduled_jobs(id)
);

CREATE INDEX idx_jobs_state ON jobs (state);
CREATE INDEX idx_jobs_delete_at ON jobs (delete_at);
CREATE INDEX idx_jobs_created_at ON jobs (created_at);

create index jobs_tags_idx on jobs using gin (tags);

CREATE TABLE jobs_perms (
    id      varchar(32) not null primary key,
    job_id  varchar(32) not null references jobs(id),
    user_id varchar(32)          references users(id),
    role_id varchar(32)          references roles(id)
);

CREATE INDEX jobs_perms_job_id_idx ON jobs_perms (job_id);
CREATE INDEX jobs_perms_user_role_idx ON jobs_perms (user_id,role_id);

CREATE TABLE tasks (
    id            varchar(32) not null primary key,
    job_id        varchar(32) not null references jobs(id),
    position      bigint      not null,
    name          varchar(256),
    state         varchar(10) not null,
    created_at    timestamptz not null,
    scheduled_at  timestamptz,
    started_at    timestamptz,
    completed_at  timestamptz,
    failed_at     timestamptz,
    cmd           text[],
    entrypoint    text[],
    run_script    text,
    image         varchar(256),
    registry      bytea,
    env           bytea,
    files_        bytea,
    queue         varchar(256),
    error_        text,
    pre_tasks     bytea,
    post_tasks    bytea,
    sidecars      bytea,
    mounts        bytea,
    node_id       varchar(32),
    retry         bytea,
    limits        bytea,
    timeout       varchar(8),
    result        text,
    var           varchar(64),
    parallel      bytea,
    parent_id     varchar(32),
    each_         bytea,
    description   text,
    subjob        bytea,
    networks      text[],
    gpus          text,
    if_           text,
    tags          text[],
    priority      bigint,
    workdir       varchar(256),
    progress      double precision default 0
);

CREATE INDEX idx_tasks_state ON tasks (state);
CREATE INDEX idx_tasks_job_id ON tasks (job_id);
CREATE INDEX idx_tasks_parent_and_state ON tasks (parent_id,state);

CREATE TABLE tasks_log_parts (
    id         varchar(32) not null primary key,
    number_    bigint      not null,
    task_id    varchar(32) not null references tasks(id),
    created_at timestamptz not null,
    contents   text        not null
);

CREATE INDEX idx_tasks_log_parts_task_id ON tasks_log_parts (task_id);
CREATE INDEX idx_tasks_log_parts_created_at ON tasks_log_parts (created_at);
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_contains_create_table_nodes() {
        assert!(
            SCHEMA.contains("CREATE TABLE nodes ("),
            "SCHEMA must define nodes table"
        );
    }

    #[test]
    fn schema_contains_create_table_users() {
        assert!(
            SCHEMA.contains("CREATE TABLE users ("),
            "SCHEMA must define users table"
        );
    }

    #[test]
    fn schema_contains_create_table_roles() {
        assert!(
            SCHEMA.contains("CREATE TABLE roles ("),
            "SCHEMA must define roles table"
        );
    }

    #[test]
    fn schema_contains_create_table_users_roles() {
        assert!(
            SCHEMA.contains("CREATE TABLE users_roles ("),
            "SCHEMA must define users_roles table"
        );
    }

    #[test]
    fn schema_contains_create_table_scheduled_jobs() {
        assert!(
            SCHEMA.contains("CREATE TABLE scheduled_jobs ("),
            "SCHEMA must define scheduled_jobs table"
        );
    }

    #[test]
    fn schema_contains_create_table_scheduled_jobs_perms() {
        assert!(
            SCHEMA.contains("CREATE TABLE scheduled_jobs_perms ("),
            "SCHEMA must define scheduled_jobs_perms table",
        );
    }

    #[test]
    fn schema_contains_create_table_jobs() {
        assert!(
            SCHEMA.contains("CREATE TABLE jobs ("),
            "SCHEMA must define jobs table"
        );
    }

    #[test]
    fn schema_contains_create_table_jobs_perms() {
        assert!(
            SCHEMA.contains("CREATE TABLE jobs_perms ("),
            "SCHEMA must define jobs_perms table"
        );
    }

    #[test]
    fn schema_contains_create_table_tasks() {
        assert!(
            SCHEMA.contains("CREATE TABLE tasks ("),
            "SCHEMA must define tasks table"
        );
    }

    #[test]
    fn schema_contains_create_table_tasks_log_parts() {
        assert!(
            SCHEMA.contains("CREATE TABLE tasks_log_parts ("),
            "SCHEMA must define tasks_log_parts table",
        );
    }

    // ── Index existence tests ──────────────────────────────────────────

    #[test]
    fn schema_contains_index_nodes_heartbeat() {
        assert!(
            SCHEMA.contains("CREATE INDEX idx_nodes_heartbeat ON nodes"),
            "SCHEMA must index nodes.last_heartbeat_at",
        );
    }

    #[test]
    fn schema_contains_index_roles_slug() {
        assert!(
            SCHEMA.contains("CREATE UNIQUE INDEX idx_roles_slug ON roles"),
            "SCHEMA must have unique index on roles.slug",
        );
    }

    #[test]
    fn schema_contains_index_users_roles_unique() {
        assert!(
            SCHEMA.contains("CREATE UNIQUE INDEX idx_users_roles_uniq ON users_roles"),
            "SCHEMA must have unique index on (user_id, role_id)",
        );
    }

    #[test]
    fn schema_contains_index_jobs_state() {
        assert!(
            SCHEMA.contains("CREATE INDEX idx_jobs_state ON jobs"),
            "SCHEMA must index jobs.state",
        );
    }

    #[test]
    fn schema_contains_index_jobs_delete_at() {
        assert!(
            SCHEMA.contains("CREATE INDEX idx_jobs_delete_at ON jobs"),
            "SCHEMA must index jobs.delete_at",
        );
    }

    #[test]
    fn schema_contains_index_jobs_created_at() {
        assert!(
            SCHEMA.contains("CREATE INDEX idx_jobs_created_at ON jobs"),
            "SCHEMA must index jobs.created_at",
        );
    }

    #[test]
    fn schema_contains_index_jobs_tags() {
        assert!(
            SCHEMA.contains("create index jobs_tags_idx on jobs using gin (tags)"),
            "SCHEMA must have GIN index on jobs.tags",
        );
    }

    #[test]
    fn schema_contains_index_jobs_perms_job_id() {
        assert!(
            SCHEMA.contains("CREATE INDEX jobs_perms_job_id_idx ON jobs_perms"),
            "SCHEMA must index jobs_perms.job_id",
        );
    }

    #[test]
    fn schema_contains_index_jobs_perms_user_role() {
        assert!(
            SCHEMA.contains("CREATE INDEX jobs_perms_user_role_idx ON jobs_perms"),
            "SCHEMA must index jobs_perms (user_id, role_id)",
        );
    }

    #[test]
    fn schema_contains_index_tasks_state() {
        assert!(
            SCHEMA.contains("CREATE INDEX idx_tasks_state ON tasks"),
            "SCHEMA must index tasks.state",
        );
    }

    #[test]
    fn schema_contains_index_tasks_job_id() {
        assert!(
            SCHEMA.contains("CREATE INDEX idx_tasks_job_id ON tasks"),
            "SCHEMA must index tasks.job_id",
        );
    }

    #[test]
    fn schema_contains_index_tasks_parent_and_state() {
        assert!(
            SCHEMA.contains("CREATE INDEX idx_tasks_parent_and_state ON tasks"),
            "SCHEMA must index tasks (parent_id, state)",
        );
    }

    #[test]
    fn schema_contains_index_tasks_log_parts_task_id() {
        assert!(
            SCHEMA.contains("CREATE INDEX idx_tasks_log_parts_task_id ON tasks_log_parts"),
            "SCHEMA must index tasks_log_parts.task_id",
        );
    }

    #[test]
    fn schema_contains_index_tasks_log_parts_created_at() {
        assert!(
            SCHEMA.contains("CREATE INDEX idx_tasks_log_parts_created_at ON tasks_log_parts"),
            "SCHEMA must index tasks_log_parts.created_at",
        );
    }

    // ── Constraint / column existence tests ────────────────────────────

    /// Normalizes whitespace in the SCHEMA string for flexible matching.
    /// Collapses all runs of whitespace into single spaces.
    fn normalized_schema() -> String {
        SCHEMA.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn schema_nodes_has_primary_key() {
        assert!(
            normalized_schema().contains("id varchar(32) not null primary key"),
            "nodes.id must be varchar(32) primary key",
        );
    }

    #[test]
    fn schema_users_has_unique_username() {
        assert!(
            normalized_schema().contains("username_ varchar(64) not null unique"),
            "users.username_ must be unique",
        );
    }

    #[test]
    fn schema_roles_has_unique_slug() {
        assert!(
            normalized_schema().contains("slug varchar(64) not null unique"),
            "roles.slug must be unique",
        );
    }

    #[test]
    fn schema_users_roles_has_foreign_keys() {
        let sql = normalized_schema();
        assert!(
            sql.contains("user_id varchar(32) not null references users(id)"),
            "users_roles.user_id must reference users(id)",
        );
        assert!(
            sql.contains("role_id varchar(32) not null references roles(id)"),
            "users_roles.role_id must reference roles(id)",
        );
    }

    #[test]
    fn schema_scheduled_jobs_has_created_by_fk() {
        assert!(
            normalized_schema().contains("created_by varchar(32) not null references users(id)"),
            "scheduled_jobs.created_by must reference users(id)",
        );
    }

    #[test]
    fn schema_jobs_has_created_by_fk() {
        // The jobs table also has created_by referencing users
        let count = normalized_schema()
            .matches("created_by varchar(32) not null references users(id)")
            .count();
        assert!(
            count >= 2,
            "jobs and scheduled_jobs must both reference users(id)"
        );
    }

    #[test]
    fn schema_tasks_has_job_id_fk() {
        assert!(
            normalized_schema().contains("job_id varchar(32) not null references jobs(id)"),
            "tasks.job_id must reference jobs(id)",
        );
    }

    #[test]
    fn schema_tasks_log_parts_has_task_id_fk() {
        assert!(
            normalized_schema().contains("task_id varchar(32) not null references tasks(id)"),
            "tasks_log_parts.task_id must reference tasks(id)",
        );
    }

    #[test]
    fn schema_jobs_has_scheduled_job_id_fk() {
        assert!(
            normalized_schema()
                .contains("scheduled_job_id varchar(32) references scheduled_jobs(id)"),
            "jobs.scheduled_job_id must reference scheduled_jobs(id)",
        );
    }

    // ── Seed data tests ────────────────────────────────────────────────

    #[test]
    fn schema_seeds_guest_user() {
        assert!(
            SCHEMA.contains("'Guest','guest'"),
            "SCHEMA must seed a Guest user",
        );
    }

    #[test]
    fn schema_seeds_public_role() {
        assert!(
            SCHEMA.contains("'Public','public'"),
            "SCHEMA must seed a Public role",
        );
    }

    // ── Column type checks (sqlx-compatible) ──────────────────────────

    #[test]
    fn schema_jobs_progress_is_double() {
        assert!(
            normalized_schema().contains("progress double precision default 0"),
            "jobs.progress must be double precision",
        );
    }

    #[test]
    fn schema_tasks_progress_is_double() {
        let count = normalized_schema()
            .matches("progress double precision default 0")
            .count();
        assert_eq!(
            count, 2,
            "both jobs and tasks must have progress double precision"
        );
    }

    #[test]
    fn schema_jobs_tags_is_text_array() {
        assert!(
            normalized_schema().contains("tags text[] not null default '{}'"),
            "jobs.tags must be text[] not null default {{}}",
        );
    }

    #[test]
    fn schema_scheduled_jobs_tags_is_text_array() {
        let count = normalized_schema()
            .matches("tags text[] not null default '{}'")
            .count();
        assert!(
            count >= 2,
            "both jobs and scheduled_jobs must have tags text[]"
        );
    }

    #[test]
    fn schema_uses_timestamptz() {
        // Verify all timestamp columns use timestamptz (not plain timestamp)
        let sql = normalized_schema();
        assert!(
            !sql.contains("timestamp not null"),
            "SCHEMA must use timestamptz, not bare timestamp"
        );
        assert!(
            sql.contains("timestamptz"),
            "SCHEMA must use timestamptz columns"
        );
    }

    #[test]
    fn schema_uses_bigint_for_counters() {
        let sql = normalized_schema();
        assert!(
            sql.contains("position bigint not null"),
            "position columns must be bigint to match Rust i64"
        );
        assert!(
            sql.contains("task_count bigint not null"),
            "task_count columns must be bigint to match Rust i64"
        );
    }

    #[test]
    fn schema_nodes_port_is_bigint() {
        let sql = normalized_schema();
        assert!(
            sql.contains("port bigint not null"),
            "nodes.port must be bigint to match Rust i64"
        );
    }
}
