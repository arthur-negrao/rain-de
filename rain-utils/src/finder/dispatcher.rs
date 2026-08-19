use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use kanal::{AsyncSender, bounded_async};

use super::types::{Match, SearchTask};

/// A routing interface to submit search tasks and collect results.
///
/// The [`Dispatcher`] provides methods to dispatch search patterns to the
/// background worker threads. Each search call broadcasts the pattern to all
/// available workers, which concurrently calculate the candidates' scores and
/// return the matches.
///
/// It provides both blocking and non-blocking methods to seamlessly integrate
/// with asynchronous runtimes or synchronous system threads.
///
/// # Concurrency
///
/// The [`Dispatcher`] is strictly thread-safe ([`Send`] + [`Sync`]). It is
/// cheaply clonable and designed to be shared across multiple threads to allow
/// concurrent search submissions.
///
/// # Important
///
/// This component relies on the background worker threads managed by the
/// [`super::Runner`]. If the `Runner` goes out of scope and is dropped, any
/// pending or future requests made through the [`Dispatcher`] will fail with an
/// I/O error ([`std::io::ErrorKind::BrokenPipe`]).
#[derive(Debug, Clone)]
pub struct Dispatcher {
    task_senders: Arc<Vec<AsyncSender<SearchTask>>>,
    workers: usize,
    current_epoch: Arc<AtomicU64>,
}

impl Dispatcher {
    pub(super) fn new(
        task_senders: Arc<Vec<AsyncSender<SearchTask>>>,
        workers: usize,
        current_epoch: Arc<AtomicU64>,
    ) -> Self {
        Self {
            task_senders,
            workers,
            current_epoch,
        }
    }

    /// Dispatches an asynchronous search task to the workers and returns an
    /// [`Match`] vector.
    pub async fn submit(&self, pattern: &str, sort: bool) -> io::Result<Vec<Match>> {
        // get the current epoch and update it
        let epoch = self.current_epoch.fetch_add(1, Ordering::SeqCst) + 1;

        let pattern_arc = Arc::<str>::from(pattern);

        let (response_sender, response_receiver) = bounded_async::<Vec<Match>>(self.workers);

        for sender in self.task_senders.iter() {
            sender
                .send(SearchTask::new(
                    pattern_arc.clone(),
                    epoch,
                    response_sender.clone(),
                ))
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e.to_string()))?;
        }

        drop(response_sender);

        let mut all_matches = Vec::with_capacity(self.workers);

        while let Ok(result) = response_receiver.recv().await {
            all_matches.extend(result);
        }

        if sort {
            all_matches.sort_unstable_by(|a, b| b.score.cmp(&a.score));
        }

        Ok(all_matches)
    }

    /// Blocks the thread until dispatches an asynchronous search task to the
    /// workers and returns an [`Match`] vector.
    pub fn submit_blocking(&self, pattern: &str, sort: bool) -> io::Result<Vec<Match>> {
        // get the current epoch and update it
        let epoch = self.current_epoch.fetch_add(1, Ordering::SeqCst) + 1;

        let pattern_arc = Arc::<str>::from(pattern);

        let (response_sender, response_receiver) = bounded_async::<Vec<Match>>(self.workers);

        for sender in self.task_senders.iter() {
            sender
                .as_sync()
                .send(SearchTask::new(
                    pattern_arc.clone(),
                    epoch,
                    response_sender.clone(),
                ))
                .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e.to_string()))?;
        }

        // free the channel to the last worker destroy it
        drop(response_sender);

        let mut all_matches = Vec::with_capacity(self.workers);

        while let Ok(result) = response_receiver.as_sync().recv() {
            all_matches.extend(result);
        }

        if sort {
            all_matches.sort_unstable_by(|a, b| b.score.cmp(&a.score));
        }

        Ok(all_matches)
    }

    /// Get the number of workers.
    #[inline]
    pub fn workers(&self) -> usize {
        self.workers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dispatcher_broken_pipe_propagation() {
        // mock the channel
        let (tx, rx) = kanal::bounded_async(1);

        let task_senders = Arc::new(vec![tx]);
        let epoch = Arc::new(AtomicU64::new(0));

        let dispatcher = Dispatcher::new(task_senders, 1, epoch);

        // similar to a worker crash
        drop(rx);

        // dispatcher must returns a error (no workers)
        let result = dispatcher.submit("pattern", false).await;

        assert!(
            result.is_err(),
            "The dispatcher must abort the operation and must returns a error when the worker channel is closed/broken."
        );

        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::BrokenPipe,
            "The channel closes must be mapped to a BrokenPipe error."
        );
    }
}
