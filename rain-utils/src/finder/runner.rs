use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use kanal::AsyncSender;
use tracing::{error, trace};

use super::dispatcher::Dispatcher;
use super::engine::Engine;
use super::types::{JoinHandle, SearchTask};

/// Controls the lifetime of the background worker threads.
///
/// The [`Runner`] acts as a guard that keeps the worker threads active. The
/// entire fuzzy finding execution depends on this struct; when the [`Runner`]
/// is dropped, all associated worker threads are explicitly stopped and joined.
///
/// Keep this instance alive in a top-level scope (such as your main loop) and
/// use the [`Runner::dispatcher`] method to obtain a [`Dispatcher`] reference,
/// which allows you to submit search tasks.
///
/// Each task sends a pattern to the workers to calculate the match score
/// against the dataset candidates.
///
/// # Important
///
/// To prevent the search engine from terminating prematurely, ensure the
/// [`Runner`] remains in scope for the entire duration of the application's
/// runtime.
pub struct Runner {
    dispatcher: Dispatcher,
    workers_handles: Vec<Option<JoinHandle>>,
    task_senders: Arc<Vec<AsyncSender<SearchTask>>>,
}

impl Runner {
    /// Create a new [`Runner`] instance and init the worker threads.
    pub fn new(candidates: Arc<Vec<String>>, workers: usize) -> io::Result<Self> {
        if candidates.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "The candidates must have items",
            ));
        }

        if workers == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "The workers can not be 0",
            ));
        }

        let current_epoch: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));

        let mut task_senders: Vec<AsyncSender<SearchTask>> = Vec::new();

        let workers = if candidates.len() >= workers {
            workers
        } else {
            candidates.len()
        };

        let chunk_size = (candidates.len() + workers - 1) / workers;

        let mut chunks: Vec<Vec<usize>> = Vec::with_capacity(workers);
        let mut current_chunk: Vec<usize> = Vec::with_capacity(chunk_size);

        for id in 0..candidates.len() {
            current_chunk.push(id);

            if current_chunk.len() == chunk_size {
                chunks.push(current_chunk);
                current_chunk = Vec::with_capacity(chunk_size);
            }
        }

        if !current_chunk.is_empty() {
            chunks.push(current_chunk);
        }

        let mut workers_handles = Vec::new();
        for (worker_id, chunk) in chunks.into_iter().enumerate() {
            let (task_sender, task_receiver) = kanal::bounded_async::<SearchTask>(2);
            task_senders.push(task_sender);

            let current_epoch_clone = current_epoch.clone();
            let candidates_clone = candidates.clone();

            let handle = spawn_worker(worker_id, move || {
                let refs: Vec<(&str, usize)> = chunk
                    .iter()
                    .map(|&id| (candidates_clone[id].as_ref(), id))
                    .collect();

                let mut engine = Engine::new(&refs);

                while let Ok(task) = task_receiver.as_sync().recv() {
                    if task.epoch < current_epoch_clone.load(Ordering::Relaxed) {
                        continue;
                    }

                    let matches = engine.search(&task.pattern, false);

                    if task.epoch < current_epoch_clone.load(Ordering::Relaxed) {
                        continue;
                    }

                    if task.response_sender.as_sync().send(matches).is_err() {
                        break;
                    }
                }
            })?;

            workers_handles.push(Some(handle));
        }

        let senders_arc = Arc::new(task_senders);
        let dispatcher = Dispatcher::new(senders_arc.clone(), workers, current_epoch);

        Ok(Self {
            dispatcher,
            workers_handles,
            task_senders: senders_arc,
        })
    }

    /// Get a [`Dispatcher`] reference to submit tasks.
    pub fn dispatcher(&self) -> &Dispatcher {
        &self.dispatcher
    }

    /// Check if the all worker threads are finished.
    pub fn is_finished(&self) -> bool {
        let mut is_finished = true;

        for handle_opt in &self.workers_handles {
            if let Some(handle) = handle_opt {
                is_finished = is_finished && handle.is_finished();
            }
        }

        is_finished
    }

    /// Get the number of worker threads.
    #[inline]
    pub fn workers(&self) -> usize {
        self.dispatcher.workers()
    }

    /// Get the number of worker threads alive.
    #[inline]
    pub fn active_workers(&self) -> usize {
        let mut workers = 0;
        for handle_opt in &self.workers_handles {
            if handle_opt.is_some() {
                workers += 1;
            }
        }

        workers
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        trace!("Join Fuzzy workers.");

        // Clear the channel to avoid deadlocks. this clear() drop all channels
        // and the workers main loop will broke because the recv() command will
        // returns a Err(ReceiveError).
        for sender in self.task_senders.iter() {
            if let Err(e) = sender.close() {
                error!("Error to close a channel: {}", e);
            }
        }

        for handle_opt in self.workers_handles.iter_mut() {
            if let Some(handle) = handle_opt.take() {
                #[cfg(not(feature = "tokio"))]
                {
                    if let Err(e) = handle.join() {
                        error!("Failed to join worker: {:?}", e);
                    };
                }

                #[cfg(feature = "tokio")]
                {
                    // force the worker destruction in tokio
                    handle.abort();
                }
            }
        }
    }
}

#[cfg(feature = "tokio")]
fn spawn_worker<F>(id: usize, f: F) -> io::Result<JoinHandle>
where
    F: FnOnce() -> () + Send + 'static,
{
    trace!("Spawning a blocking worker: {}", id);
    Ok(tokio::task::spawn_blocking(f))
}

#[cfg(not(feature = "tokio"))]
fn spawn_worker<F>(id: usize, f: F) -> io::Result<JoinHandle>
where
    F: FnOnce() -> () + Send + 'static,
{
    trace!("Spawning a blocking worker: {}", id);
    std::thread::Builder::new()
        .name(format!("fuzzy-worker({})", id))
        .spawn(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_async_engine() {
        let candidates = Arc::new(vec![
            "src/test/".to_string(),
            "src/test/mod.rs".to_string(),
            "src/main.rs".to_string(),
            "sr/mod.rs".to_string(),
        ]);

        let workers = 2;
        let runner = Runner::new(candidates, workers)
            .expect("Failed to init the engine with more candidates than workers.");

        assert_eq!(
            runner.workers(),
            2,
            "The number of workers must be equal to the engine when has more candidates than workers."
        );
    }

    #[tokio::test]
    async fn test_async_search_flow() {
        let candidates = Arc::new(vec![
            "src/test/".to_string(),
            "src/test/mod.rs".to_string(),
            "src/main.rs".to_string(),
        ]);

        let runner = Runner::new(candidates, 2).unwrap();
        let dispatcher = runner.dispatcher();

        let matches = dispatcher
            .submit("test", true)
            .await
            .expect("The task submit must be work successfully.");

        assert!(
            !matches.is_empty(),
            "Shold find matches with pattern 'test'."
        );

        // test sort
        if matches.len() > 1 {
            assert!(
                matches[0].score >= matches[1].score,
                "The sorter must prioritize the greater score."
            );
        }
    }

    #[tokio::test]
    async fn test_epoch_synchronization_and_skip() {
        // large dataset to force a slow search
        let candidates: Arc<Vec<String>> = Arc::new(
            (0..50_000)
                .map(|i| format!("documento_{}.txt", i))
                .collect(),
        );

        let runner = Runner::new(candidates, 1).unwrap();
        let dispatcher = runner.dispatcher();

        // first search
        let dispatcher_clone = dispatcher.clone();
        let handle_obsolete =
            tokio::spawn(async move { dispatcher_clone.submit("doc", true).await.unwrap() });

        // give the thread control to the tokio runtime for a instant
        tokio::task::yield_now().await;

        // new task to ignore the first
        let matches_new = dispatcher.submit("documento_99", true).await.unwrap();

        // await the obsolet matches
        let matches_obsolete = handle_obsolete.await.unwrap();

        // the new epoch will invalidate the old submit
        assert!(
            matches_obsolete.is_empty(),
            "The older task must be interrupt and return 0."
        );

        // the new task validation
        assert!(
            !matches_new.is_empty(),
            "The new task must be process normally."
        );
    }

    #[test]
    fn test_empty_candidates_handling() {
        let candidates: Arc<Vec<String>> = Arc::new(vec![]);
        let workers = 4;

        let engine_result = Runner::new(candidates, workers);

        assert!(
            engine_result.is_err(),
            "The engine must return an Error if the candidates are empty."
        );
    }

    #[test]
    fn test_zero_workers_handling() {
        let candidates = Arc::new(vec![
            "src/test/".to_string(),
            "src/test/mod.rs".to_string(),
            "src/main.rs".to_string(),
        ]);
        let workers = 0;

        let engine_result = Runner::new(candidates, workers);

        assert!(
            engine_result.is_err(),
            "The engine must return an Error if the workers is equal to 0."
        )
    }
}
