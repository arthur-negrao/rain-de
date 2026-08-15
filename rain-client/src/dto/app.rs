use std::{
    os::unix::process::ExitStatusExt,
    process::{Command as STDCommand, Stdio},
};

#[cfg(feature = "gtk")]
use gtk::glib;

use freedesktop_desktop_entry::DesktopEntry;
use serde::{Deserialize, Serialize};
use tokio::process::Command as TokioCommand;
use tracing::{error, info, warn};
use zbus::zvariant::Type;

#[derive(Debug, Default, Clone, Serialize, Deserialize, Type)]
pub struct AppEntryDTO {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub version: String,
    pub exec_cmd: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
}

impl AppEntryDTO {
    pub fn new(
        id: String,
        name: String,
        icon: String,
        version: String,
        exec_cmd: String,
        description: String,
        keywords: Vec<String>,
        categories: Vec<String>,
    ) -> Self {
        Self {
            id,
            name,
            icon,
            version,
            exec_cmd,
            description,
            keywords,
            categories,
        }
    }

    pub fn from_entry(entry: &DesktopEntry, locales: &[String]) -> AppEntryDTO {
        let id = entry.id().to_string();

        let name = entry
            .name(locales)
            .map(|name_str| name_str.to_string())
            .unwrap_or(String::new());

        let icon = entry
            .icon()
            .map(|icon| icon.to_string())
            .unwrap_or(String::new());

        let keywords: Vec<String> = entry
            .keywords(locales)
            .unwrap_or(Vec::new())
            .iter()
            .map(|keyword| keyword.to_string())
            .collect();

        let exec_cmd = entry
            .exec()
            .map(|cmd| cmd.to_string())
            .unwrap_or(String::new());

        let version = entry
            .version()
            .map(|v| v.to_string())
            .unwrap_or(String::new());

        let categories: Vec<String> = entry
            .categories()
            .unwrap_or(Vec::new())
            .iter()
            .map(|category| category.to_string())
            .collect();

        let description = entry
            .comment(locales)
            .map(|comment| comment.to_string())
            .unwrap_or(String::new());

        Self::new(
            id,
            name,
            icon,
            version,
            exec_cmd,
            description,
            keywords,
            categories,
        )
    }

    fn get_sanitize_exec_cmd(&self) -> String {
        let ignore_placeholders = ["%u", "%U", "%f", "%F"];
        let mut cmd = self.exec_cmd.to_string();

        for placeholder in ignore_placeholders {
            cmd = cmd.replace(placeholder, "");
        }

        cmd = cmd.replace("%c", &self.name);

        if !self.icon.is_empty() {
            let icon_arg = format!("--icon {}", self.icon);
            cmd = cmd.replace("%i", &icon_arg);
        } else {
            cmd = cmd.replace("%i", "");
        }

        cmd
    }

    pub fn launch_with_tokio(&self) {
        let cmd_string = self.get_sanitize_exec_cmd();

        info!("Trying launch the app {}.", self.name);
        let name = self.name.clone();

        tokio::spawn(async move {
            let parts: Vec<&str> = cmd_string.split_whitespace().collect();

            if parts.is_empty() {
                error!("Failed to launch the app {}. The Command is None.", name);
                return;
            }

            let exec = parts[0];
            let mut cmd = TokioCommand::new(exec);

            if parts.len() > 1 {
                cmd.args(&parts[1..]);
            }

            let exit_status = cmd
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;

            match exit_status {
                Ok(status) => {
                    match status.code() {
                        None => {
                            warn!(
                                "The app {} was terminated by a signal: {:?}",
                                name,
                                status.signal()
                            );
                        }
                        Some(code) => {
                            info!("The app {} exits with code: {}", name, code);
                        }
                    };
                }
                Err(e) => error!("Failed to run app {}: {}", name, e),
            }
        });
    }

    #[cfg(feature = "gtk")]
    pub fn launch_with_gtk(&self) {
        let cmd_string = self.get_sanitize_exec_cmd();
        let name = self.name.clone();

        glib::MainContext::default().spawn_local(async move {
            gtk::gio::spawn_blocking(move || {
                let parts: Vec<String> = cmd_string
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();

                if parts.is_empty() {
                    error!("Failed to launch the app {}. The Command is None.", name);
                    return;
                }

                let exec = &parts[0];
                let mut cmd = STDCommand::new(exec);

                if parts.len() > 1 {
                    cmd.args(&parts[1..]);
                }

                let exit_status = cmd
                    .stdin(Stdio::null())
                    .stderr(Stdio::null())
                    .stdout(Stdio::null())
                    .status();

                match exit_status {
                    Ok(status) => {
                        match status.code() {
                            None => {
                                warn!(
                                    "The app {} was terminated by a signal: {:?}",
                                    name,
                                    status.signal()
                                );
                            }
                            Some(code) => {
                                info!("The app {} exits with code: {}", name, code);
                            }
                        };
                    }
                    Err(e) => error!("Failed to run app {}: {}", name, e),
                };
            });
        });
    }
}
