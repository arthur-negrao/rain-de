use gtk::glib;
use gtk::{Application, ApplicationWindow, Orientation, Revealer, prelude::*};
use gtk4_layer_shell::Edge::Bottom;
use tracing::error;

use rain_client::appd::entry::connect_to_appd;
use rain_client::wayland::Bridge;
use rain_utils::ui::layer_shell::LayerShellConfig;

use crate::state::manager::DockState;
use crate::ui::{css_loader::load_css, factory::build_buckets_view};

pub struct Dock {
    state: DockState,
    bar: Revealer,
}

impl Dock {
    pub fn new(state: &DockState, orientation: gtk::Orientation) -> Self {
        let bar_box = build_bar_view(state, orientation);

        let revealer = Revealer::builder()
            .transition_type(gtk::RevealerTransitionType::SlideUp)
            .transition_duration(250)
            .child(&bar_box)
            .css_classes(["dock-revealer"])
            .build();

        revealer.connect_show(glib::clone!(
            #[weak]
            state,
            move |_reveler| {
                state.set_is_visible(true);
            }
        ));

        revealer.connect_hide(glib::clone!(
            #[weak]
            state,
            move |_reveler| {
                state.set_is_visible(false);
            }
        ));

        if !state.auto_hide() && !state.is_empty() {
            revealer.set_reveal_child(true);
        }

        Self {
            state: state.clone(),
            bar: revealer,
        }
    }

    pub fn attach_auto_hide_controller(&self, trigger_container: &gtk::Box) {
        let motion_ctrl = gtk::EventControllerMotion::new();

        motion_ctrl.connect_enter(glib::clone!(
            #[strong(rename_to = bar)]
            self.bar,
            #[strong(rename_to = state)]
            self.state,
            move |_ctrl, _x, _y| {
                if state.auto_hide() && !state.is_empty() {
                    bar.set_reveal_child(true);
                }
            }
        ));

        motion_ctrl.connect_leave(glib::clone!(
            #[strong(rename_to = bar)]
            self.bar,
            #[strong(rename_to = state)]
            self.state,
            move |_ctrl| {
                if state.auto_hide() && !state.popup_is_open() {
                    bar.set_reveal_child(false);
                }
            }
        ));

        trigger_container.add_controller(motion_ctrl);
    }
}

/// Build a dock window
///
/// The `bridge` is a communication between the dock and the Wayland Thread.
pub fn build_dock(app: &Application, bridge: Bridge) {
    let state = DockState::new(bridge);

    glib::MainContext::default().spawn_local(glib::clone!(
        #[strong]
        state,
        async move {
            let proxy_result = connect_to_appd().await;

            match proxy_result {
                Err(e) => {
                    error!("Failed to connect with appd: {}", e);
                }
                Ok(proxy) => {
                    state.set_appd_proxy(proxy);
                }
            };
        }
    ));

    let window = ApplicationWindow::builder()
        .application(app)
        .decorated(false)
        .title("Rain-Dock")
        .resizable(false)
        .css_classes(["dock-window"])
        .build();

    let orientation = gtk::Orientation::Horizontal;

    let main_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .valign(gtk::Align::End)
        .spacing(0)
        .css_classes(["dock-container"])
        .build();

    let trigger_zone = gtk::Box::builder()
        .height_request(3)
        .css_classes(["trigger-zone"])
        .build();

    let dock = Dock::new(&state, orientation);

    main_box.append(&dock.bar);
    main_box.append(&trigger_zone);

    window.set_child(Some(&main_box));

    match orientation {
        gtk::Orientation::Horizontal => window.set_size_request(-1, 120),
        gtk::Orientation::Vertical => window.set_size_request(120, -1),
        _ => {}
    };

    LayerShellConfig::new().anchor(Bottom, true).apply(&window);

    dock.attach_auto_hide_controller(&main_box);

    window.present();
}

fn build_bar_view(state: &DockState, orientation: gtk::Orientation) -> gtk::Box {
    load_css();

    let dock_box = gtk::Box::builder()
        .orientation(orientation)
        .halign(gtk::Align::Center)
        .css_classes(["dock-box"])
        .build();

    let pinned_model = state.pinned_buckets();
    let free_model = state.buckets();

    let pinned_buckets = build_buckets_view(pinned_model.clone(), orientation, state.clone());
    let free_buckets = build_buckets_view(free_model.clone(), orientation, state.clone());

    let separator_orientation = match orientation {
        Orientation::Vertical => Orientation::Horizontal,
        Orientation::Horizontal => Orientation::Vertical,
        _ => Orientation::Vertical,
    };
    let separator = gtk::Separator::builder()
        .orientation(separator_orientation)
        .margin_start(12)
        .margin_end(12)
        .visible(pinned_model.n_items() > 0 && free_model.n_items() > 0)
        .css_classes(["buckets-view-separator"])
        .build();

    // connect the separator
    // the separator will be visible when the both models have elements
    pinned_model.connect_items_changed(glib::clone!(
        #[weak]
        separator,
        #[weak]
        free_model,
        move |model, _, _, _| {
            separator.set_visible(model.n_items() > 0 && free_model.n_items() > 0);
        },
    ));

    free_model.connect_items_changed(glib::clone!(
        #[weak]
        separator,
        #[weak]
        pinned_model,
        move |model, _, _, _| {
            separator.set_visible(model.n_items() > 0 && pinned_model.n_items() > 0);
        }
    ));

    dock_box.append(&pinned_buckets);
    dock_box.append(&separator);
    dock_box.append(&free_buckets);

    dock_box
}
