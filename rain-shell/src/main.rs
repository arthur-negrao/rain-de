use gtk::{
    Application,
    gio::prelude::{ApplicationExt, ApplicationExtManual},
};

use tracing::error;

use rain_client::wayland::Bridge;
use rain_dock::ui::bar::build_dock;
use rain_utils::log::telemetry::init_telemetry;

const APPLICATION_NAME: &str = "com.rain.Rain";

fn main() -> gtk::glib::ExitCode {
    let tokio_runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Filed to create tokio runtime.");

    let _tokio_guard = tokio_runtime.enter();
    let _log_guard = init_telemetry();

    let app = Application::builder()
        .application_id(APPLICATION_NAME)
        .build();

    match Bridge::new() {
        Ok(bridge) => {
            app.connect_activate(move |app| build_dock(app, bridge.clone()));
        }
        Err(e) => {
            error!(
                "Fail to run Dock. The Wayland Bridge already is started. Can not start it again: {}",
                e
            );
        }
    };

    app.run()
}
