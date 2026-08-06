use gtk::glib::subclass::types::ObjectSubclassIsExt;
use gtk::glib::{self};
use gtk::prelude::*;
use tracing::{debug, info, warn};

use rain_client::wayland::protocols::toplevel::ToplevelCommand;

use crate::state::{bucket::DockBucket, manager::DockState};

pub(super) fn build_opened_windows_view(menu_box: gtk::Box, bucket: DockBucket, state: DockState) {
    let windows = bucket.bucket();

    if !windows.is_empty() {
        let win_label = gtk::Label::builder()
            .label("Opened Windows")
            .xalign(0.0)
            .margin_start(6)
            .css_classes(["bucket-window-menu-label"])
            .build();
        menu_box.append(&win_label);

        for (window_id, window) in windows.iter() {
            let win_btn = gtk::Button::builder().tooltip_text(&window.title()).build();

            let btn_label = gtk::Label::builder()
                .label(&window.title())
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .max_width_chars(30)
                .css_classes(["bucket-window-menu-btn-label"])
                .build();

            let win_box = gtk::Box::new(gtk::Orientation::Horizontal, 5);
            let blank_space = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(0)
                .hexpand(true)
                .css_classes(["bucket-window-menu-btn-blank-space"])
                .build();

            let btn_close_win = gtk::Button::builder()
                .icon_name("window-close")
                .css_classes(["bucket-window-menu-btn-close-window"])
                .build();
            btn_close_win.set_tooltip_text(Some("Close Window"));

            let btn_close_win_controller = gtk::GestureClick::new();
            let window_id_unwrap = *window_id;
            btn_close_win_controller.connect_pressed(glib::clone!(
                #[strong]
                state,
                #[strong]
                window,
                move |gesture, _n_press, _x, _y| {
                    debug!("Send Close request to window: {}", window.title());
                    state.send_command(ToplevelCommand::Close(window_id_unwrap));

                    // stop propagation
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                }
            ));
            btn_close_win.add_controller(btn_close_win_controller);

            win_box.append(&btn_label);
            win_box.append(&blank_space);
            win_box.append(&btn_close_win);

            win_btn.set_child(Some(&win_box));

            let window_id_unwrap = *window_id;
            win_btn.connect_clicked(glib::clone!(
                #[strong]
                state,
                #[strong]
                window,
                move |_btn| {
                    debug!("Send Focus request to window: {}", window.title());
                    state.send_command(ToplevelCommand::Focus(window_id_unwrap));
                }
            ));

            menu_box.append(&win_btn);
        }

        let separator = gtk::Separator::builder()
            .orientation(gtk::Orientation::Horizontal)
            .css_classes(["bucket-menu-separator"])
            .build();
        menu_box.append(&separator);
    }
}

pub(super) fn build_actions_menu(
    menu_box: gtk::Box,
    popover: gtk::Popover,
    bucket: DockBucket,
    state: DockState,
) {
    let action_label = gtk::Label::builder()
        .label("Actions")
        .xalign(0.0)
        .margin_start(6)
        .css_classes(["bucket-action-menu-label"])
        .build();

    menu_box.append(&action_label);

    let pin_btn = build_popover_pin_btn(&bucket.app_class(), popover.clone(), state.clone());
    menu_box.append(&pin_btn);

    if !bucket.is_empty() {
        let close_all_windows_btn =
            build_popover_btn_close_all_windows(popover.clone(), state.clone(), bucket.clone());
        menu_box.append(&close_all_windows_btn);

        let fullscreen_btn =
            build_popover_btn_fullscreen_window(popover.clone(), state.clone(), bucket.clone());
        menu_box.append(&fullscreen_btn);

        let minimize_btn =
            build_popover_btn_minimize_window(popover.clone(), state.clone(), bucket.clone());
        menu_box.append(&minimize_btn);

        let close_window_btn =
            build_popover_btn_close_current_window(popover.clone(), state.clone(), bucket.clone());
        menu_box.append(&close_window_btn);
    }

    let open_window_btn =
        build_popover_btn_open_new_window(&bucket.app_class(), popover.clone(), state.clone());
    menu_box.append(&open_window_btn);
}

pub(super) fn build_popover_pin_btn(
    app_class: &str,
    popover: gtk::Popover,
    state: DockState,
) -> gtk::Button {
    let is_pinned = state.is_pinned(app_class).unwrap_or(false);

    let is_pinned_label = if is_pinned { "Unpin" } else { "Pin" };

    let btn = create_action_btn(is_pinned_label, Some("view-pin"));

    let cloned_app_class = app_class.to_string();

    btn.connect_clicked(move |_| {
        if is_pinned {
            debug!("Unpin the bucket: {}", &cloned_app_class);
            state.unpin_bucket(&cloned_app_class, None);
        } else {
            debug!("Pin the bucket: {}", &cloned_app_class);
            state.pin_bucket(&cloned_app_class, None);
        }

        popover.popdown();
    });

    btn
}

pub(super) fn build_popover_btn_minimize_window(
    popover: gtk::Popover,
    state: DockState,
    bucket: DockBucket,
) -> gtk::Button {
    let btn = create_action_btn("Minimize Current Window", Some("window-minimize"));

    btn.connect_clicked(move |_| {
        if let Some(app) = bucket.bucket().get(&bucket.last_focus()) {
            let set = !app.is_minimized();

            state.send_command(ToplevelCommand::Minimize((app.id(), set)));

            info!(
                "{} Minimize request to the window: {}",
                if set { "Set" } else { "Unset" },
                app.title()
            );
            popover.popdown();
        }
        popover.popdown();
    });

    btn
}

pub(super) fn build_popover_btn_fullscreen_window(
    popover: gtk::Popover,
    state: DockState,
    bucket: DockBucket,
) -> gtk::Button {
    let btn = create_action_btn("Fullscreen Current Window", Some("view-fullscreen"));

    btn.connect_clicked(move |_| {
        if let Some(app) = bucket.bucket().get(&bucket.last_focus()) {
            let set = !app.is_fullscreen();

            state.send_command(ToplevelCommand::Fullscreen((app.id(), set)));

            info!(
                "{} Fullscreen request to the window: {}",
                if set { "Set" } else { "Unset" },
                app.title()
            );
            popover.popdown();
        }
    });

    btn
}

pub(super) fn build_popover_btn_close_all_windows(
    popover: gtk::Popover,
    state: DockState,
    bucket: DockBucket,
) -> gtk::Button {
    let btn = create_action_btn("Close All Windows", Some("window-close"));

    btn.connect_clicked(move |_| {
        info!("Closing all windows from bucket: {}", bucket.app_class());

        for (window_id, app) in bucket.bucket().iter() {
            debug!("Send close command to wayland to window: {}", app.title());
            state.send_command(ToplevelCommand::Close(*window_id));
        }
        popover.popdown();
    });

    btn
}

pub(super) fn build_popover_btn_open_new_window(
    app_class: &str,
    popover: gtk::Popover,
    state: DockState,
) -> gtk::Button {
    let btn = create_action_btn("Open New Window", Some("window-new"));

    let cloned_app_class = app_class.to_string();
    let cache = state.imp().app_entries_cache.clone();

    btn.connect_clicked(move |_| {
        info!(
            "Request to open new application window to {}.",
            cloned_app_class
        );
        if let Some(entry) = cache.borrow().get(&cloned_app_class) {
            entry.launch_with_gtk();
        } else {
            warn!(
                "Fail to open new instance of: {}. No entry in cache.",
                cloned_app_class
            );
        }

        popover.popdown();
    });

    btn
}

pub(super) fn build_popover_btn_close_current_window(
    popover: gtk::Popover,
    state: DockState,
    bucket: DockBucket,
) -> gtk::Button {
    let btn = create_action_btn("Close Current Window", Some("window-close"));

    btn.connect_clicked(move |_| {
        if let Some(app) = bucket.bucket().get(&bucket.last_focus()) {
            info!("Close request to the window: {}", app.title());
            state.send_command(ToplevelCommand::Close(app.id()));

            popover.popdown();
        }
    });

    btn
}

fn create_action_btn(text: &str, icon_name: Option<&str>) -> gtk::Button {
    let label = gtk::Label::builder()
        .label(text)
        .css_classes(["bucket-action-menu-btn-label"])
        .build();

    let box_btn = gtk::Box::builder()
        .halign(gtk::Align::Start)
        .spacing(5)
        .css_classes(["bucket-action-menu-btn-box"])
        .build();

    if let Some(icon_name) = icon_name {
        let icon = gtk::Image::builder()
            .icon_name(icon_name)
            .css_classes(["bucket-action-menu-btn-icon"])
            .build();
        box_btn.append(&icon);
    }

    box_btn.append(&label);

    let btn = gtk::Button::new();
    btn.set_child(Some(&box_btn));

    btn
}
