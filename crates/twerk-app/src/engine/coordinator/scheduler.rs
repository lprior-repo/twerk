//! Task scheduler for the coordinator

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

use anyhow::Result;
use std::sync::Arc;
use twerk_infrastructure::broker::queue::QUEUE_PENDING;
use twerk_core::eval::{evaluate_task, evaluate_expr};
use twerk_core::id::{JobId, TaskId};

pub struct Scheduler {
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
}

impl Scheduler {
    pub fn new(
        ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
        broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    ) -> Self {
        Self { ds, broker }
    }

    pub async fn schedule_task(&self, task: twerk_core::task::Task) -> Result<()> {
        if task.parallel.is_some() {
            self.schedule_parallel_task(task).await
        } else if task.each.is_some() {
            self.schedule_each_task(task).await
        } else if task.subjob.is_some() {
            self.schedule_subjob_task(task).await
        } else {
            self.schedule_regular_task(task).await
        }
    }

    pub async fn schedule_regular_task(&self, mut task: twerk_core::task::Task) -> Result<()> {
        let task_id = task.id.clone().unwrap_or_default();
        let now = time::OffsetDateTime::now_utc();
        
        task.state = twerk_core::task::TASK_STATE_SCHEDULED.to_string();
        task.scheduled_at = Some(now);
        
        if task.queue.is_none() {
            task.queue = Some("default".to_string());
        }
        
        let q = task.queue.clone().unwrap_or_default();
        let t_queue = task.queue.clone();

        self.ds.update_task(&task_id, Box::new(move |mut u| {
            u.state = twerk_core::task::TASK_STATE_SCHEDULED.to_string();
            u.scheduled_at = Some(now);
            u.queue = t_queue;
            Ok(u)
        })).await?;
        
        self.broker.publish_task(q, &task).await?;
        
        Ok(())
    }

    pub async fn schedule_parallel_task(&self, task: twerk_core::task::Task) -> Result<()> {
        let task_id = task.id.clone().unwrap_or_default();
        let job_id = task.job_id.clone().unwrap_or_default();
        let now = time::OffsetDateTime::now_utc();
        
        let job = self.ds.get_job_by_id(&job_id).await?;
        let job_ctx = job.context.as_ref().map(|c| c.as_map()).unwrap_or_default();

        self.ds.update_task(&task_id, Box::new(move |mut u| {
            u.state = twerk_core::task::TASK_STATE_RUNNING.to_string();
            u.started_at = Some(now);
            Ok(u)
        })).await?;
        
        if let Some(parallel) = &task.parallel {
            if let Some(tasks) = &parallel.tasks {
                for t in tasks {
                    let mut pt = t.clone();
                    pt = evaluate_task(&pt, &job_ctx)
                        .map_err(|e| anyhow::anyhow!("failed to evaluate parallel task: {}", e))?;

                    pt.id = Some(uuid::Uuid::new_v4().to_string().into());
                    pt.job_id = Some(job_id.clone().into());
                    pt.parent_id = Some(task_id.to_string().into());
                    pt.state = twerk_core::task::TASK_STATE_PENDING.to_string();
                    pt.created_at = Some(now);
                    
                    self.ds.create_task(&pt).await?;
                    self.broker.publish_task(QUEUE_PENDING.to_string(), &pt).await?;
                }
            }
        }
        
        Ok(())
    }

    pub async fn schedule_each_task(&self, task: twerk_core::task::Task) -> Result<()> {
        let task_id = task.id.clone().unwrap_or_default();
        let job_id = task.job_id.clone().unwrap_or_default();
        let now = time::OffsetDateTime::now_utc();
        
        let job = self.ds.get_job_by_id(&job_id).await?;
        let job_ctx_map = job.context.as_ref().map(|c| c.as_map()).unwrap_or_default();
        
        let each = task.each.as_ref().ok_or_else(|| anyhow::anyhow!("missing each config"))?;
        let list_expr = each.list.as_deref().unwrap_or_default();
        
        let mut list_val = if list_expr.trim().starts_with('[') {
            serde_json::from_str(list_expr).unwrap_or(serde_json::Value::String(list_expr.to_string()))
        } else {
            evaluate_expr(list_expr, &job_ctx_map)
                .map_err(|e| anyhow::anyhow!("failed to evaluate each list: {}", e))?
        };
        
        if let Some(s) = list_val.as_str() {
            if let Ok(json_list) = serde_json::from_str(s) {
                list_val = json_list;
            }
        }

        let list = list_val.as_array().ok_or_else(|| anyhow::anyhow!("each list must be an array"))?;
        let size = list.len() as i64;

        self.ds.update_task(&task_id, Box::new(move |mut u| {
            u.state = twerk_core::task::TASK_STATE_RUNNING.to_string();
            u.started_at = Some(now);
            if let Some(ref mut e) = u.each {
                e.size = size;
            }
            Ok(u)
        })).await?;
        
        if let Some(each) = &task.each {
            if let Some(template) = &each.task {
                for (ix, item) in list.iter().enumerate() {
                    let mut cx = job_ctx_map.clone();
                    let var_name = each.var.as_deref().unwrap_or("item");
                    cx.insert(var_name.to_string(), serde_json::json!({
                        "index": ix.to_string(),
                        "value": item
                    }));

                    let mut et = (**template).clone();
                    et = evaluate_task(&et, &cx)
                        .map_err(|e| anyhow::anyhow!("failed to evaluate each item task: {}", e))?;

                    et.id = Some(uuid::Uuid::new_v4().to_string().into());
                    et.job_id = Some(job_id.clone().into());
                    et.parent_id = Some(task_id.to_string().into());
                    et.state = twerk_core::task::TASK_STATE_PENDING.to_string();
                    et.created_at = Some(now);
                    
                    self.ds.create_task(&et).await?;
                    self.broker.publish_task(QUEUE_PENDING.to_string(), &et).await?;
                }
            }
        }
        
        Ok(())
    }

    pub async fn schedule_subjob_task(&self, task: twerk_core::task::Task) -> Result<()> {
        let task_id = task.id.clone().unwrap_or_default();
        let job_id = task.job_id.clone().unwrap_or_default();
        let now = time::OffsetDateTime::now_utc();
        
        let job = self.ds.get_job_by_id(&job_id).await?;
        
        if let Some(subjob_task) = &task.subjob {
            let subjob = twerk_core::job::Job {
                id: Some(uuid::Uuid::new_v4().to_string().into()),
                parent_id: Some(task_id.to_string().into()),
                name: subjob_task.name.clone(),
                description: subjob_task.description.clone(),
                state: twerk_core::job::JOB_STATE_PENDING.to_string(),
                tasks: subjob_task.tasks.clone(),
                inputs: subjob_task.inputs.clone(),
                secrets: subjob_task.secrets.clone(),
                task_count: subjob_task.tasks.as_ref().map(|t| t.len() as i64).unwrap_or(0),
                output: subjob_task.output.clone(),
                webhooks: subjob_task.webhooks.clone(),
                auto_delete: subjob_task.auto_delete.clone(),
                created_at: Some(now),
                created_by: job.created_by.clone(),
                ..Default::default()
            };
            
            let subjob_id = subjob.id.clone().unwrap_or_default();
            
            self.ds.update_task(&task_id, Box::new(move |mut u| {
                u.state = twerk_core::task::TASK_STATE_RUNNING.to_string();
                u.started_at = Some(now);
                if let Some(ref mut sj) = u.subjob {
                    sj.id = Some(subjob_id.clone());
                }
                Ok(u)
            })).await?;
            
            self.ds.create_job(&subjob).await?;
            self.broker.publish_job(&subjob).await?;
        }
        
        Ok(())
    }
}
