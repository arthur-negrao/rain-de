use std::path::PathBuf;
use tracing::{debug, error, info, warn};

const DEFAULT_CSS_CALLBACK: &str = r#"
.dock-window{
    background: none;
    background-color: transparent;
    box-shadow: none;
    border: none;
}

.dock-box {
    background-color: rgba(18, 18, 18, 0.75);
    border: 2px solid rgba(255, 255, 255, 0.1);
    border-radius: 16px;
    padding: 6px;
    margin-bottom: 5px;

    transition: transform 250ms cubic-bezier(0.25, 1, 0.5, 1);
}

.trigger-zone {
    /* has 0.01 of opacity to grab the mouse focus to reveal the dock */
    background-color: rgba(0, 0, 0, 0.01);
}

.buckets-view,
.buckets-view > row,
.buckets-view > row:hover,
.buckets-view > row:selected{
    background-color: transparent;
    background: none;
    box-shadow: none;
    border: none;
    padding: 0;
}

.bucket-box {
    background: transparent;
    transition: all 200ms ease-in-out;
}

.bucket-btn {
    background: transparent;
    border: none;
    box-shadow: none;
    padding: 4px;
}

.bucket-icon {
    transition: all 200ms ease-in-out;
}

.bucket-btn:hover .bucket-icon {
    transform: scale(1.2);
    filter: drop-shadow(0px 4px 6px rgba(0,0,0,0.3));
}

.bucket-indicator {
    min-height: 4px;
    min-width: 0px;
    background-color: transparent;
    border-radius: 50%;
    margin-top: 4px;
    transition: all 200ms ease-in-out;
}

.bucket-indicator.running {
    min-width: 4px;
    background-color: #01b077;
}

.bucket-indicator.running.focused {
    min-width: 24px;
    border-radius: 2px;
    background-color: #01b077;
}

.buckets-view-separator {
    margin: 4px 0;
    min-height: 1px;
    min-width: 2px;
    border-radius: 2px;
    background-color: rgba(255, 255, 255, 0.1);
}


.bucket-popover > contents {
    background-color: rgba(18, 18, 18, 0.95);
    border: 2px solid rgba(255, 255, 255, 0.1);
    border-radius: 12px;
    padding: 8px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
}

.bucket-popover > arrow {
    background: none;
    border: none;
}

.bucket-window-menu-label,
.bucket-action-menu-label {
    font-size: 11px;
    font-weight: bold;
    color: rgba(255, 255, 255, 0.5);
    margin-top: 4px;
    margin-bottom: 4px;
    text-transform: uppercase;
}

.bucket-popover button {
    background: transparent;
    border: none;
    box-shadow: none;
    border-radius: 8px;
    padding: 6px;
    color: #ffffff;
    transition: background-color 150ms ease-in-out;
}

.bucket-popover button:hover {
    background-color: rgba(255, 255, 255, 0.1);
}

.bucket-menu-separator {
    margin: 4px 0;
    min-height: 1px;
    background-color: rgba(255, 255, 255, 0.1);
}

.bucket-window-menu-btn-close-window {
    border-radius: 16px;
}

.bucket-window-menu-btn-close-window:hover {
    background-color: rgba(220, 50, 50, 0.8);
}
"#;

fn find_css_path() -> Option<PathBuf> {
    let home_path = std::env::var("HOME").unwrap_or_else(|_| String::from("~"));
    let search_paths = [
        format!("{}/.config/rain/rain-dock/style.css", home_path),
        String::from("/etc/rain/rain-dock/style.css"),
        String::from("/usr/share/rain/rain-dock/style.css"),
    ];

    for current_path in search_paths {
        debug!("Try load CSS from: {}", current_path);
        let path = PathBuf::from(&current_path);

        if path.exists() && path.is_file() {
            info!("CSS loaded from: {}", current_path);
            return Some(path);
        }
    }

    warn!("No CSS files found.");
    None
}

pub fn load_css() {
    let Some(display) = gtk::gdk::Display::default() else {
        error!("The GTK display fails to apply the CSS style.");
        return;
    };

    let default_provider = gtk::CssProvider::new();
    default_provider.load_from_string(DEFAULT_CSS_CALLBACK);

    gtk::style_context_add_provider_for_display(
        &display,
        &default_provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    if let Some(path) = find_css_path() {
        let user_provider = gtk::CssProvider::new();
        user_provider.load_from_path(path);

        gtk::style_context_add_provider_for_display(
            &display,
            &user_provider,
            gtk::STYLE_PROVIDER_PRIORITY_USER,
        );

        info!("Custom CSS applied on top of the default css.");
    }
}
