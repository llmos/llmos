//! Cron-style scheduled tasks that enqueue work on [`crate::background::BackgroundHub`].

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use cron::Schedule;

use crate::background::BackgroundHub;
use crate::errors::AgentError;

#[async_trait]
pub trait ScheduledTask: Send + Sync {
    async fn run(&self);
}

struct Entry {
    schedule: Schedule,
    name: String,
    task: Arc<dyn ScheduledTask + Send + Sync>,
}

/// Owns cron schedules and starts one async runner per entry.
#[derive(Clone)]
pub struct Scheduler {
    hub: Arc<BackgroundHub>,
    entries: Arc<Vec<Entry>>,
}

impl Scheduler {
    pub fn new(hub: Arc<BackgroundHub>) -> Self {
        Self {
            hub,
            entries: Arc::new(Vec::new()),
        }
    }

    fn from_entries(hub: Arc<BackgroundHub>, entries: Vec<Entry>) -> Self {
        Self {
            hub,
            entries: Arc::new(entries),
        }
    }

    /// Parse `expr` with standard 6-field cron (sec min hour dom mon dow), e.g. `0 */10 * * * *`.
    pub fn builder(hub: Arc<BackgroundHub>) -> SchedulerBuilder {
        SchedulerBuilder { hub, entries: Vec::new() }
    }

    /// Spawn a Tokio task per schedule; each loop sleeps until the next fire, then enqueues `task` on the hub.
    pub fn start(self) {
        for e in self.entries.iter() {
            let hub = self.hub.clone();
            let schedule = e.schedule.clone();
            let name = e.name.clone();
            let task = e.task.clone();
            tokio::spawn(run_entry(hub, schedule, name, task));
        }
    }
}

async fn run_entry(
    hub: Arc<BackgroundHub>,
    schedule: Schedule,
    name: String,
    task: Arc<dyn ScheduledTask + Send + Sync>,
) {
    loop {
        let next = schedule.upcoming(Utc).next();
        let Some(next) = next else {
            tokio::time::sleep(Duration::from_secs(60)).await;
            continue;
        };
        let now = Utc::now();
        let wait = (next - now)
            .to_std()
            .unwrap_or_else(|_| Duration::from_millis(200));
        tokio::time::sleep(wait).await;

        let hub_c = hub.clone();
        let name_c = name.clone();
        let task_c = task.clone();
        hub_c.spawn(format!("cron:{name_c}"), async move {
            task_c.run().await;
        });
    }
}

pub struct SchedulerBuilder {
    hub: Arc<BackgroundHub>,
    entries: Vec<Entry>,
}

impl SchedulerBuilder {
    pub fn try_add(
        mut self,
        name: impl Into<String>,
        cron_expr: &str,
        task: Arc<dyn ScheduledTask + Send + Sync>,
    ) -> Result<Self, AgentError> {
        let schedule = Schedule::from_str(cron_expr)
            .map_err(|e| AgentError::msg(format!("invalid cron {cron_expr:?}: {e}")))?;
        self.entries.push(Entry {
            schedule,
            name: name.into(),
            task,
        });
        Ok(self)
    }

    pub fn build(self) -> Scheduler {
        Scheduler::from_entries(self.hub, self.entries)
    }
}

/// Built-in heartbeat: logs UTC time (wire to metrics or channels later).
#[derive(Debug, Default, Clone)]
pub struct HeartbeatTask;

#[async_trait]
impl ScheduledTask for HeartbeatTask {
    async fn run(&self) {
        tracing::info!(ts = %Utc::now().to_rfc3339(), "scheduler heartbeat");
    }
}
