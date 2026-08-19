use std::sync::Arc;

use kanal::AsyncSender;

#[cfg(not(feature = "tokio"))]
pub type JoinHandle = std::thread::JoinHandle<()>;
#[cfg(feature = "tokio")]
pub type JoinHandle = tokio::task::JoinHandle<()>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match {
    pub id: usize,
    pub score: i32,
}

impl Match {
    pub fn new(id: usize, score: i32) -> Self {
        Self { id, score }
    }
}

impl PartialOrd for Match {
    fn ge(&self, other: &Self) -> bool {
        self.score >= other.score
    }

    fn le(&self, other: &Self) -> bool {
        self.score <= other.score
    }

    fn lt(&self, other: &Self) -> bool {
        self.score < other.score
    }

    fn gt(&self, other: &Self) -> bool {
        self.score > other.score
    }

    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.score > other.score {
            Some(std::cmp::Ordering::Greater)
        } else if self.score == other.score {
            Some(std::cmp::Ordering::Equal)
        } else {
            Some(std::cmp::Ordering::Less)
        }
    }
}

impl Ord for Match {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.score > other.score {
            std::cmp::Ordering::Greater
        } else if self.score == other.score {
            std::cmp::Ordering::Equal
        } else {
            std::cmp::Ordering::Less
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SearchTask {
    /// The pattern to be sent to each worker. It is a Arc to avoid heap
    /// allocations.
    pub pattern: Arc<str>,

    /// Task's epoch to receive in the future. It works like a ID to receive
    /// the mail later.
    pub epoch: u64,

    /// A channel to send the calculated response.
    pub response_sender: AsyncSender<Vec<Match>>,
}

impl SearchTask {
    pub(crate) fn new(
        pattern: Arc<str>,
        epoch: u64,
        response_sender: AsyncSender<Vec<Match>>,
    ) -> Self {
        SearchTask {
            pattern,
            epoch,
            response_sender,
        }
    }
}
