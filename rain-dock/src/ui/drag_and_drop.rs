use serde::{Deserialize, Serialize};

use gtk::{gdk, glib, prelude::*};

use crate::{
    state::{bucket::DockBucket, manager::DockState},
    ui::bucket_button::BucketButton,
};

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct DragPayload {
    source_position: u32,
    app_class: String,
    is_pinned: bool,
}

pub(super) fn bind_drag_and_drop(
    state: DockState,
    bucket: DockBucket,
    list_item: gtk::ListItem,
    btn: BucketButton,
) {
    let drag_source = gtk::DragSource::builder()
        .name("drag-source")
        .actions(gdk::DragAction::MOVE)
        .build();

    drag_source.connect_prepare(glib::clone!(
        #[weak]
        list_item,
        #[weak]
        btn,
        #[strong]
        state,
        #[strong]
        bucket,
        #[upgrade_or]
        None,
        move |source, _x, _y| {
            // add icon to source drag
            let gicon = bucket.app_icon();
            let display = btn.display();
            let icon_theme = gtk::IconTheme::for_display(&display);

            // convert to a pintable icon
            let icon = icon_theme.lookup_by_gicon(
                &gicon,
                48,
                1,
                gtk::TextDirection::Ltr,
                gtk::IconLookupFlags::empty(),
            );

            source.set_icon(Some(&icon), 0, 0);

            let app_class = bucket.app_class();
            let is_pinned = state.is_pinned(&app_class).unwrap_or(false);
            let payload = DragPayload {
                source_position: list_item.position(),
                is_pinned: is_pinned,
                app_class: app_class,
            };

            if let Ok(payload_json) = serde_json::to_string(&payload) {
                Some(gdk::ContentProvider::for_value(&payload_json.to_value()))
            } else {
                None
            }
        }
    ));

    btn.add_controller(drag_source);

    let drop_target = gtk::DropTarget::new(glib::Type::STRING, gdk::DragAction::MOVE);
    drop_target.set_name(Some("drop-target"));

    drop_target.connect_drop(glib::clone!(
        #[weak]
        list_item,
        #[strong]
        bucket,
        #[strong]
        state,
        #[upgrade_or]
        false,
        move |_target, value, _x, _y| {
            if let Ok(json_string) = value.get::<String>() {
                if let Ok(payload) = serde_json::from_str::<DragPayload>(&json_string) {
                    let _source_idx = payload.source_position;
                    let _source_is_pinned = payload.is_pinned;
                    let source_app_class = payload.app_class;

                    let target_idx = list_item.position();
                    let target_app_class = bucket.app_class();

                    if source_app_class == target_app_class {
                        return false;
                    }

                    let target_is_pinned = state.is_pinned(&target_app_class).unwrap_or(false);

                    state.move_bucket(&source_app_class, target_idx, target_is_pinned);

                    return true;
                }
            }
            false
        }
    ));

    btn.add_controller(drop_target);
}
