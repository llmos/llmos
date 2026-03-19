//! Fire-and-forget background work on the server (nanobot-style subtasks, without blocking turns).

use std::future::Future;
use std::sync::Arc;

use tokio::sync::Semaphore;

/// Limits concurrent background jobs; spawned tasks run on the shared Tokio runtime.
#[derive(Clone)]
pub struct BackgroundHub {
    semaphore: Arc<Semaphore>,
}

impl Default for BackgroundHub {
    fn default() -> Self {
        Self::new(32)
    }
}

impl BackgroundHub {
    pub fn new(max_concurrent: usize) -> Self {
        let n = max_concurrent.max(1);
        Self {
            semaphore: Arc::new(Semaphore::new(n)),
        }
    }

    /// Spawn a labeled background future. Returns immediately; work continues asynchronously.
    pub fn spawn<F>(&self, _label: impl Into<String>, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let sem = self.semaphore.clone();
        tokio::spawn(async move {
            let Ok(permit) = sem.acquire_owned().await else {
                return;
            };
            fut.await;
            drop(permit);
        });
    }
}
