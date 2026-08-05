use tokio::process::Command;
use tracing::{debug, warn};
use zbus::{fdo, interface};

use crate::DaemonState;
use rain_client::dto::AppEntryDTO;

#[derive(Debug, Default)]
pub struct Daemon {
    state: DaemonState,
}

/// The Appd's heart. This Struct keep the main methods to call and get
/// `DesktopEntry`.
impl Daemon {
    /// Creates a new instance of the daemon to inject in zbus.
    ///
    /// The `state` must be initiated with `find_entries()` and injected in
    /// contructor.
    pub fn new(state: DaemonState) -> Self {
        Self { state }
    }
}

#[interface(name = "org.rain.Appd")]
impl Daemon {
    /// Get all App IDs stored in the daemon.
    async fn get_all_ids(&self) -> Vec<String> {
        self.state
            .get_all_entries()
            .await
            .iter()
            .map(|entry| entry.id().to_string())
            .collect()
    }

    async fn get_entry(&self, app_id: &str) -> fdo::Result<AppEntryDTO> {
        let Some(entry) = self.state.get_entry(app_id).await else {
            warn!(
                "Failed to get the DesktopEntry from entry {}. Entry is None.",
                app_id
            );
            return Err(fdo::Error::UnknownObject(format!(
                "Entry {} not found",
                app_id
            )));
        };

        Ok(AppEntryDTO::from_entry(&entry, &self.state.locales))
    }

    async fn get_all_entries(&self) -> Vec<AppEntryDTO> {
        let entries = self.state.get_all_entries().await;

        entries
            .iter()
            .map(|entry| AppEntryDTO::from_entry(entry, &self.state.locales))
            .collect()
    }

    /// Run a application using the `DesktopEntry` `exec` field.
    async fn run(&self, app_id: &str) -> fdo::Result<()> {
        debug!("Trying run the entry: {}", app_id);

        let Some(entry) = self.state.get_entry(app_id).await else {
            warn!("Can not run the entry {}. The entry is None.", app_id);
            return Err(fdo::Error::UnknownObject(format!(
                "Entry {} not found",
                app_id
            )));
        };

        let Some(cmd) = entry.exec() else {
            warn!(
                "Can not run the entry {}. The exec entry field is None.",
                app_id
            );
            return Err(fdo::Error::Failed("No exec field".to_string()));
        };

        let app_id_string = app_id.to_string();
        match Command::new(cmd).spawn() {
            Ok(mut child) => {
                tokio::spawn(async move {
                    if let Ok(status) = child.wait().await {
                        debug!("The entry {} exited with code: {}", app_id_string, status);
                    }
                });
                Ok(())
            }
            Err(e) => {
                warn!("Error with run the cmd {}: {}", cmd, e);
                Err(fdo::Error::Failed(format!(
                    "Failed to spawn process: {}",
                    e
                )))
            }
        }
    }
}
