use std::{collections::HashMap, sync::Arc};
use tokio::sync::mpsc;

use freedesktop_desktop_entry::{DesktopEntry, Iter, default_paths, get_languages_from_env};
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

#[derive(Debug, Default, Clone)]
struct DaemonSharedState {
    pub(crate) desktop_entries: Arc<RwLock<HashMap<String, Arc<DesktopEntry>>>>,
}

#[derive(Debug, Default)]
pub struct DaemonState {
    state: DaemonSharedState,
    pub locales: Vec<String>,
    already_finding: bool,
}

/// The Intern Daemon's state. The state will watch and keep all entries finded.
impl DaemonState {
    /// Creates a new instance. Call `find_entries()` to start the background workers.
    pub fn new() -> Self {
        Self::default()
    }

    /// Init the workers threads and scan default desktop entries to fill the
    /// `entry_map`.
    ///
    /// This method produces 1 provider thread and will watch it with the
    /// inotify api. Furthermore, this method will creates 1 task to fill the
    /// `entry_map`, avoiding the concurrence.
    ///
    /// # Depends
    /// The tokio async runtime must be initiated and still running to use this
    /// method.
    ///
    /// # Async
    /// This method is non-blocking to avoid blocks on `event loop`.
    ///
    /// # Panics
    /// This methods will panics if run more than 1 time.
    #[instrument]
    pub fn find_entries(&mut self) {
        if !self.already_finding {
            info!("Starting the DesktopEntries search...");

            // lock to avoid multiples inits
            self.already_finding = true;

            let locales = get_languages_from_env();
            self.locales = locales.clone();

            let (sender, mut receiver) = mpsc::channel::<Arc<DesktopEntry>>(128);

            // provider
            //
            // thread to read and watch the desktop entries dirs
            tokio::task::spawn_blocking(move || {
                info!("Starting provider thread for XDG paths");

                let paths: Vec<_> = default_paths().collect();

                for path in paths {
                    debug!("Find the entries in: {:?}", path);

                    let single_path = std::iter::once(path);
                    let sender_clone = sender.clone();

                    for raw_entry in Iter::new(single_path).entries(Some(&locales)) {
                        let entry_arc = Arc::new(raw_entry);
                        if let Err(e) = sender_clone.blocking_send(entry_arc) {
                            warn!("Failed to send desktop entry through channel: {:?}", e);
                            break;
                        }
                    }
                    // active the inotify api
                }
            });

            let state_clone = self.state.clone();

            // receiver
            //
            // this unique thread will updates the map to avoid concurrence
            tokio::task::spawn(async move {
                info!("Starting the consumer thread to updates the map");
                while let Some(entry) = receiver.recv().await {
                    let id = entry.id().to_string();

                    let mut map = state_clone.desktop_entries.write().await;

                    // perform a insert, because the order is guaranteed by the
                    // provider
                    map.insert(id, entry);
                }
            });
        } else {
            panic!("The Daemon can not start find more than 1 time.");
        }
    }

    /// Get a `DesktopEntry` pointer.
    ///
    /// It uses the `app_id` (app class) to get a `DesktopEntry` if the entry
    /// is available.
    #[instrument]
    pub async fn get_entry(&self, app_id: &str) -> Option<Arc<DesktopEntry>> {
        self.state
            .desktop_entries
            .read()
            .await
            .get(app_id)
            .map(|entry| entry.clone())
    }

    /// Get a `Vec` with all `DesktopEntry` pointers stored in state.
    #[instrument]
    pub async fn get_all_entries(&self) -> Vec<Arc<DesktopEntry>> {
        self.state
            .desktop_entries
            .read()
            .await
            .values()
            .map(|entry| entry.clone())
            .collect()
    }
}
