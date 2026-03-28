# Contract Specification: Cron Scheduler Handler

## Context

- **Feature**: Distributed cron scheduler handler for scheduled jobs
- **Domain Terms**:
  - `ScheduledJob` - A job definition with cron schedule that produces `Job` executions
  - `Job` - An execution instance created from a `ScheduledJob` on each cron trigger
  - `Scheduler` - gocron-based scheduler managing cron job registrations
  - `Coordinator` - Multiple coordinator instances requiring distributed locking
  - `Lock` - Distributed lock (via `glocker`/`glock`) preventing duplicate execution across coordinators
  - `minScheduledJobLockTTL` - Minimum 10-second lock hold time to prevent coordinator drift
- **Assumptions**:
  - Datastore, Broker, and Locker dependencies are available and functional at initialization
  - gocron scheduler uses distributed locking via custom `glocker` implementation
  - Scheduled jobs can be in `Active` or `Paused` states only
- **Open Questions**:
  - What happens if the cron expression is malformed? (gocron likely validates)
  - Is there a maximum number of scheduled jobs supported?

## Preconditions

### For `NewJobSchedulerHandler`
- [ ] `ds` (Datastore) must be non-nil and connected
- [ ] `b` (Broker) must be non-nil and connected
- [ ] `l` (Locker) must be non-nil and functional
- [ ] gocron scheduler must initialize successfully with distributed locker

### For `handle` (Active state)
- [ ] `sj.ID` must be non-empty string (scheduled job identifier)
- [ ] `sj.Cron` must be a valid cron expression parseable by gocron
- [ ] `sj.State` must equal `ScheduledJobStateActive`
- [ ] Context must be non-nil and with deadline

### For `handle` (Paused state)
- [ ] `sj.ID` must reference an existing job in the internal map `h.m`
- [ ] `sj.State` must equal `ScheduledJobStatePaused`

## Postconditions

### For `handleActive`
- [ ] A new `Job` instance is created with:
  - Generated UUID via `uuid.NewUUID()`
  - `CreatedBy` copied from scheduled job
  - `CreatedAt` set to current UTC time
  - `State` set to `JobStatePending`
  - `Tasks`, `Inputs`, `Secrets`, `Tags`, `Name`, `Description`, `Output`, `Webhooks`, `AutoDelete` copied from scheduled job
  - `Schedule` populated with scheduled job ID and cron expression
- [ ] `Job` is persisted to datastore via `h.ds.CreateJob`
- [ ] `Job` is published to broker via `h.broker.PublishJob`
- [ ] `gocron.Job` is added to internal map `h.m` keyed by `sj.ID`
- [ ] Lock for the scheduled job is held for at least `minScheduledJobLockTTL` (10 seconds) before release

### For `handlePaused`
- [ ] The `gocron.Job` is removed from the gocron scheduler
- [ ] The entry is deleted from internal map `h.m`
- [ ] Lock is released after minimum TTL enforcement

### For `NewJobSchedulerHandler`
- [ ] gocron scheduler is started (`sc.Start()`)
- [ ] All existing active scheduled jobs from datastore are registered with the scheduler
- [ ] Handler function is returned for processing subsequent schedule changes

## Invariants

- [ ] Internal map `h.m` contains ONLY jobs in `Active` state (jobs removed on pause)
- [ ] gocron scheduler state reflects the union of all `Active` scheduled jobs
- [ ] Lock hold time is always at least `minScheduledJobLockTTL` (10 seconds) to prevent coordinator drift
- [ ] At most one coordinator instance can execute a particular scheduled job at any given time (distributed lock guarantee)
- [ ] Thread safety: `h.mu` (sync.Mutex) protects concurrent access to `h.m`

## Error Taxonomy

- **Error::UnknownScheduledJobState**
  - When: `s.State` is neither `ScheduledJobStateActive` nor `ScheduledJobStatePaused`
  - Contains: The invalid state value

- **Error::SchedulerInitializationFailed**
  - When: `gocron.NewScheduler()` fails
  - Contains: Underlying gocron error

- **Error::SchedulerStartFailed**
  - When: `sc.Start()` fails after initialization
  - Contains: Underlying gocron error

- **Error::JobNotFound**
  - When: `handlePaused` is called but `sj.ID` does not exist in internal map `h.m`
  - Contains: The unknown scheduled job ID

- **Error::JobSchedulingFailed**
  - When: `h.scheduler.NewJob()` fails (invalid cron, scheduler error)
  - Contains: Underlying gocron error, wrapped with job ID

- **Error::JobRemovalFailed**
  - When: `h.scheduler.RemoveJob()` fails
  - Contains: Underlying gocron error

- **Error::DatastoreFetchFailed**
  - When: `h.ds.GetScheduledJobByID()` fails during job execution
  - Contains: Underlying datastore error

- **Error::DatastoreCreateFailed**
  - When: `h.ds.CreateJob()` fails
  - Contains: Underlying datastore error

- **Error::BrokerPublishFailed**
  - When: `h.broker.PublishJob()` fails
  - Contains: Underlying broker error

- **Error::LockAcquisitionFailed**
  - When: `glocker.Lock()` fails to acquire distributed lock
  - Contains: Underlying locker error

- **Error::LockReleaseFailed**
  - When: `glock.Unlock()` fails to release lock
  - Contains: Underlying locker error

## Contract Signatures

### Constructor
```go
func NewJobSchedulerHandler(
    ds datastore.Datastore,
    b broker.Broker,
    l locker.Locker,
) (func(ctx context.Context, s *tork.ScheduledJob) error, error)
```
- Returns: Handler function and potential initialization error
- Errors: `Error::SchedulerInitializationFailed`, `Error::DatastoreFetchFailed`

### Handler Function
```go
func (h *jobSchedulerHandler) handle(ctx context.Context, s *tork.ScheduledJob) error
```
- Returns: `nil` on success, error on failure
- Errors: `Error::UnknownScheduledJobState`, `Error::JobSchedulingFailed`, `Error::JobNotFound`, `Error::JobRemovalFailed`, `Error::DatastoreFetchFailed`, `Error::DatastoreCreateFailed`, `Error::BrokerPublishFailed`

### Active Handler
```go
func (h *jobSchedulerHandler) handleActive(ctx context.Context, sj *tork.ScheduledJob) error
```
- Returns: `nil` on success, error on failure
- Errors: `Error::JobSchedulingFailed`, `Error::DatastoreFetchFailed`, `Error::DatastoreCreateFailed`, `Error::BrokerPublishFailed`

### Paused Handler
```go
func (h *jobSchedulerHandler) handlePaused(ctx context.Context, s *tork.ScheduledJob) error
```
- Returns: `nil` on success, error on failure
- Errors: `Error::JobNotFound`, `Error::JobRemovalFailed`

### Lock Wrapper (glock)
```go
func (l glock) Unlock(ctx context.Context) error
```
- Returns: `nil` on success, error on failure
- Errors: `Error::LockReleaseFailed`
- Note: Enforces minimum lock TTL before actual release

### Lock Factory (glocker)
```go
func (d glocker) Lock(ctx context.Context, key string) (gocron.Lock, error)
```
- Returns: Lock interface and potential error
- Errors: `Error::LockAcquisitionFailed`

## Non-goals

- [ ] Job execution retry logic (handled elsewhere)
- [ ] Job result aggregation from multiple executions
- [ ] Dynamic cron expression updates without restart
- [ ] Scheduled job prioritization
- [ ] Explicit concurrency limits per scheduled job
