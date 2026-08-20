use gtk::{
    Application,
    gio::prelude::{ApplicationExt, ApplicationExtManual},
};

use tracing::error;

use rain_client::wayland::BridgeBuilder;
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

    let bridge_result = BridgeBuilder::new().enable_toplevel().build();

    // keeps the runner alive
    let _runner_guard = match bridge_result {
        Ok((bridge, _runner)) => {
            app.connect_activate(move |app| build_dock(app, bridge.clone()));

            Some(_runner)
        }
        Err(e) => {
            error!(
                "Fail to run Dock. The Wayland Bridge failed to init: {}",
                e
            );
            None
        }
    };

    app.run()
}
