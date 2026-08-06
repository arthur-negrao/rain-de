use gtk::gio::ListStore;
use gtk::gio::prelude::ListModelExt;
use gtk::glib;
use gtk::glib::object::{Cast, CastNone};
use gtk::prelude::*;
use gtk::{ListItem, SignalListItemFactory};

use tracing::debug;

use rain_client::wayland::protocols::toplevel::ToplevelCommand;

use crate::state::bucket::DockBucket;
use crate::state::manager::DockState;
use crate::ui::bucket_button::BucketButton;
use crate::ui::{drag_and_drop::*, popover_menu::*};

pub fn build_buckets_view(
    model: ListStore,
    orientation: gtk::Orientation,
    state: DockState,
) -> gtk::ListView {
    let selection_model = gtk::NoSelection::new(Some(model));

    let factory = gtk::SignalListItemFactory::new();
    let cloned_state = state.clone();

    factory.connect_setup(glib::clone!(
        #[weak]
        state,
        move |_factory, item| build_bucket_btn(state, item)
    ));
    factory.connect_bind(move |_factory, list_item| {
        bind_dock_btn(list_item, cloned_state.clone());
    });
    factory.connect_unbind(unbind_dock_btn);

    gtk::ListView::builder()
        .model(&selection_model)
        .factory(&factory)
        .orientation(orientation)
        .css_classes(["buckets-view"])
        .build()
}

fn build_bucket_btn(state: DockState, list_item: &glib::Object) {
    let item = list_item
        .downcast_ref::<ListItem>()
        .expect("The item mut be a ListItem");

    let btn_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(5)
        .css_classes(["bucket-box"])
        .build();

    let btn_icon = gtk::Image::builder()
        .pixel_size(48)
        .css_classes(["bucket-icon"])
        .build();
    btn_box.append(&btn_icon);

    let btn_indicator = gtk::Box::builder()
        .halign(gtk::Align::Center)
        .valign(gtk::Align::End)
        .css_classes(["bucket-indicator"])
        .build();
    btn_box.append(&btn_indicator);

    let btn_popover = gtk::Popover::builder()
        .css_classes(["bucket-popover"])
        .position(gtk::PositionType::Top)
        .build();

    btn_popover.connect_show(glib::clone!(
        #[weak]
        state,
        move |_popover| {
            state.set_popup_is_open(true);
        }
    ));

    btn_popover.connect_hide(glib::clone!(
        #[weak]
        state,
        move |_popover| {
            state.set_popup_is_open(false);
        }
    ));

    let btn = BucketButton::new(&btn_box, &btn_icon, &btn_indicator, &btn_popover);

    btn_popover.set_parent(&btn);

    // add right click
    let right_click_ctrl = gtk::GestureClick::new();
    right_click_ctrl.set_button(gtk::gdk::BUTTON_SECONDARY);
    right_click_ctrl.set_name(Some("right-click"));
    btn.add_controller(right_click_ctrl);

    btn.set_child(Some(&btn_box));
    item.set_child(Some(&btn));
}

fn bind_dock_btn(list_item: &glib::Object, state: DockState) {
    let item = list_item
        .downcast_ref::<ListItem>()
        .expect("The item mut be a ListItem");

    let bucket = item
        .item()
        .and_downcast::<DockBucket>()
        .expect("The item must be a DockBucket");

    let btn = item
        .child()
        .and_downcast::<BucketButton>()
        .expect("The item must be a BucketButton");

    let icon = btn.icon().clone();
    let indicator = btn.indicator().clone();
    let popover = btn.popover().clone();

    btn.set_tooltip_text(Some(&bucket.app_class()));

    bucket
        .bind_property("app-icon", &icon, "gicon")
        .sync_create()
        .build();

    let controllers = btn.observe_controllers();

    let mut right_click_ctrl = None;

    for i in 0..controllers.n_items() {
        if let Some(ctrl) = controllers.item(i).and_downcast::<gtk::GestureClick>() {
            match ctrl.name().as_deref() {
                Some("right-click") => right_click_ctrl = Some(ctrl),
                _ => {}
            }
        }
    }

    // connect the left click
    btn.connect_clicked(glib::clone!(
        #[strong]
        state,
        #[strong]
        bucket,
        move |_btn| {
            if let Some(app) = bucket.bucket().get(&bucket.last_focus()) {
                let cmd = if !app.is_focused() {
                    debug!("Focus on window: {}", app.title());
                    ToplevelCommand::Focus(app.id())
                } else {
                    debug!("Minimize window: {}", app.title());
                    ToplevelCommand::Minimize((app.id(), true))
                };

                state.send_command(cmd);
            }
        }
    ));

    // connect right click
    let right_click_ctrl =
        right_click_ctrl.expect("The dock bucket button must has a controller to right click");

    right_click_ctrl.connect_pressed(glib::clone!(
        #[strong]
        state,
        #[strong]
        bucket,
        move |_gesture, _n_press, _x, _y| {
            let menu_box = gtk::Box::new(gtk::Orientation::Vertical, 4);

            build_opened_windows_view(menu_box.clone(), bucket.clone(), state.clone());

            build_actions_menu(
                menu_box.clone(),
                popover.clone(),
                bucket.clone(),
                state.clone(),
            );

            popover.set_child(Some(&menu_box));
            popover.popup();
        }
    ));

    bind_drag_and_drop(state.clone(), bucket.clone(), item.clone(), btn.clone());

    let update_css = glib::clone!(
        #[weak]
        indicator,
        #[weak]
        bucket,
        move || {
            indicator.remove_css_class("running");
            indicator.remove_css_class("focused");

            if bucket.is_running() {
                indicator.add_css_class("running");
            }

            if bucket.is_focused() {
                indicator.add_css_class("focused");
            }
        }
    );

    update_css();

    let signal_running = bucket.connect_is_running_notify(glib::clone!(
        #[strong]
        update_css,
        move |_bucket| update_css()
    ));

    let signal_focus = bucket.connect_is_focused_notify(glib::clone!(
        #[strong]
        update_css,
        move |_bucket| update_css()
    ));

    btn.store_signal(bucket.upcast_ref::<glib::Object>(), signal_running);
    btn.store_signal(bucket.upcast_ref::<glib::Object>(), signal_focus);
}

fn unbind_dock_btn(_factory: &SignalListItemFactory, list_item: &glib::Object) {
    let item = list_item
        .downcast_ref::<gtk::ListItem>()
        .expect("The item must be a ListItem");

    if let Some(btn) = item.child().and_downcast::<BucketButton>() {
        let popover = btn.popover();
        popover.unparent();

        let controllers = btn.observe_controllers();
        let mut to_remove = Vec::new();

        for i in 0..controllers.n_items() {
            if let Some(controller) = controllers.item(i).and_downcast::<gtk::GestureClick>() {
                to_remove.push(controller);
            }
        }

        for ctrl in to_remove {
            btn.remove_controller(&ctrl);
        }

        btn.clear_signals();
    }
}
