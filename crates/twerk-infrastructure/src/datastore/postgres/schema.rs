//! `PostgreSQL` schema for the datastore.

/// The SQL schema for initializing the database.
///
/// **Note:** This schema is tuned for compatibility with `sqlx` 0.7 + Rust types:
/// - `timestamptz` instead of `timestamp` (matches `time::OffsetDateTime`)
/// - `bytea` instead of `jsonb` for JSON payloads (sqlx binds `Vec<u8>` as `bytea`)
/// - `bigint` instead of `integer` for counters (matches Rust `i64`)
/// - `double precision` for progress (matches Rust `f64`)
pub const SCHEMA: &str = r"
CREATE EXTENSION IF NOT EXISTS pgcrypto;

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

insert into users (id,name,username_,password_,created_at,is_disabled) (SELECT REPLACE(gen_random_uuid()::text, '-', ''),'Guest','guest','',current_timestamp,false);

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
    scheduled_job_id varchar(32) not null references scheduled_jobs(id) ON DELETE CASCADE,
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
    scheduled_job_id varchar(32) references scheduled_jobs(id),
    ts               tsvector GENERATED ALWAYS AS (
        setweight(to_tsvector('english', coalesce(description, '')), 'C') ||
        setweight(to_tsvector('english', coalesce(name, '')), 'B') ||
        setweight(to_tsvector('english', state), 'A')
    ) STORED
);

CREATE INDEX idx_jobs_state ON jobs (state);
CREATE INDEX idx_jobs_delete_at ON jobs (delete_at);
CREATE INDEX idx_jobs_created_at ON jobs (created_at);

create index jobs_tags_idx on jobs using gin (tags);

create index jobs_ts_idx on jobs using gin (ts);

CREATE TABLE jobs_perms (
    id      varchar(32) not null primary key,
    job_id  varchar(32) not null references jobs(id) ON DELETE CASCADE,
    user_id varchar(32)          references users(id),
    role_id varchar(32)          references roles(id)
);

CREATE INDEX jobs_perms_job_id_idx ON jobs_perms (job_id);
CREATE INDEX jobs_perms_user_role_idx ON jobs_perms (user_id,role_id);

CREATE TABLE tasks (
    id            varchar(32) not null primary key,
    job_id        varchar(32) not null references jobs(id) ON DELETE CASCADE,
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
    task_id    varchar(32) not null references tasks(id) ON DELETE CASCADE,
    created_at timestamptz not null,
    contents   text        not null,
    ts        tsvector GENERATED ALWAYS AS (
        to_tsvector('english', contents)
    ) STORED
);

CREATE INDEX idx_tasks_log_parts_task_id ON tasks_log_parts (task_id);
CREATE INDEX idx_tasks_log_parts_created_at ON tasks_log_parts (created_at);

create index tasks_log_parts_ts_idx on tasks_log_parts using gin (ts);
";
