use std::cell::{Cell, Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::path::PathBuf;

use rain_client::appd::entry::AppdProxy;
use rain_client::dto::AppEntryDTO;
use rain_client::wayland::Bridge;
use rain_client::wayland::protocols::toplevel::{
    ToplevelCommand, ToplevelEvent, WindowData, WindowState,
};
use tracing::{debug, error, info, warn};

use gtk::gio;
use gtk::glib;
use gtk::glib::subclass::prelude::*;
use gtk::prelude::*;
use gtk::{
    gio::{ListStore, prelude::ListModelExt},
    glib::object::Cast,
};

use crate::state::bucket::DockBucket;
use crate::state::dock_app::DockApp;
use crate::state::save_state::{PinnedApp, PinnedJson};

mod imp {
    use std::{cell::OnceCell, rc::Rc};

    use gtk::glib::{Properties, subclass::types::ObjectSubclass};
    use rain_client::{appd::entry::AppdProxy, dto::AppEntryDTO, wayland::Bridge};

    use super::*;

    #[derive(Debug, Properties)]
    #[properties(wrapper_type = super::DockState)]
    pub struct DockState {
        #[property(get)]
        pub buckets: ListStore,

        #[property(get)]
        pub pinned_buckets: ListStore,

        pub map_is_pinned: RefCell<HashMap<String, bool>>,

        #[property(get, set)]
        pub auto_hide: Cell<bool>,

        #[property(get, set)]
        pub is_visible: Cell<bool>,

        #[property(get, set)]
        pub max_buckets: Cell<u32>,

        #[property(get)]
        pub bucket_focused: RefCell<Option<String>>,

        #[property(get, set)]
        pub popup_is_open: Cell<bool>,

        pub wayland_bridge: OnceCell<Bridge>,

        pub app_entries_cache: Rc<RefCell<HashMap<String, AppEntryDTO>>>,

        pub appd_proxy: Rc<OnceCell<AppdProxy<'static>>>,
    }

    impl Default for DockState {
        fn default() -> Self {
            Self {
                buckets: ListStore::new::<DockBucket>(),
                pinned_buckets: ListStore::new::<DockBucket>(),
                map_is_pinned: RefCell::new(HashMap::default()),
                auto_hide: Cell::new(true),
                is_visible: Cell::new(true),
                max_buckets: Cell::new(15),
                bucket_focused: RefCell::new(None),
                popup_is_open: Cell::new(false),
                app_entries_cache: Rc::new(RefCell::new(HashMap::default())),
                appd_proxy: Rc::new(OnceCell::new()),
                wayland_bridge: OnceCell::new(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DockState {
        const NAME: &'static str = "DockState";
        type Type = super::DockState;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for DockState {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, _id: usize, _value: &glib::Value, _pspec: &glib::ParamSpec) {
            self.derived_set_property(_id, _value, _pspec);
        }

        fn property(&self, _id: usize, _pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(_id, _pspec)
        }
    }
}

glib::wrapper! {
    /// A Dock's state representation.
    pub struct DockState(ObjectSubclass<imp::DockState>);
}

impl DockState {
    const XDG_PINNED_JSON_PATH: &str = ".config/rain/rain-dock/pinned.json";

    /// Create a new instance of the Dock
    pub fn new() -> Self {
        let obj: Self = glib::Object::builder().build();
        obj.load_pinned_buckets();

        return obj;
    }

    /// Add a app instance in a bucket.
    pub fn add_app(&self, app_class: &str, app: DockApp) {
        let app_is_focused = app.is_focused();

        if let Some(bucket) = self.find_bucket(app_class) {
            bucket.insert(app);

            if app_is_focused {
                self.change_focus(app_class);
            }

            return;
        }

        self.set_bucket_on_map(app_class, false);

        let icon = self.resolve_icon_from_class(app_class);
        let bucket = DockBucket::new(app_class, icon);

        bucket.insert(app);
        self.buckets().append(&bucket);

        // try resolve the entry in cache
        self.resolve_entry(app_class, &bucket);

        if app_is_focused {
            self.change_focus(app_class);
        }
    }

    /// Insert a bucket into a bucket.
    ///
    /// Insert at last position if `index` is None and insert at
    /// `pinned_buckets` if `pinned` is `true`.
    ///
    /// It does not insert a empty bucket if it is not pinned.
    pub fn insert_bucket(&self, bucket: DockBucket, index: Option<u32>, pinned: bool) {
        if !pinned && bucket.is_empty() {
            return;
        }

        self.set_bucket_on_map(&bucket.app_class(), pinned);

        let buckets = match pinned {
            true => self.pinned_buckets(),
            false => self.buckets(),
        };

        match index {
            Some(idx) => {
                buckets.insert(idx, &bucket);
            }
            None => buckets.append(&bucket),
        }

        self.resolve_entry(&bucket.app_class(), &bucket);
    }

    /// Remove a app instance from a bucket.
    ///
    /// Remove a instance from a bucket using the `app_class` and the
    /// `app_title`. If the instance exists, then returns the `DockApp`.
    ///
    /// Remove the entire bucket if it is not pinned and is empty.
    pub fn remove_app(&self, app_class: &str, window_id: u32) -> Option<DockApp> {
        let bucket = self.find_bucket(app_class)?;

        let removed_app = bucket.remove(window_id);

        let is_pinned = self.is_pinned(app_class)?;

        if bucket.is_empty() && !is_pinned {
            self.remove_bucket(app_class);
        }

        removed_app
    }

    pub fn update_app(
        &self,
        app_class: &str,
        window_id: u32,
        app_title: &str,
        app_state: WindowState,
    ) {
        if let Some(bucket) = self.find_bucket(app_class) {
            bucket.update_app(window_id, app_title, app_state);
        }
    }

    /// Remove a entire app bucket to remove all instances and the icon.
    ///
    /// Remove the bucket by the `app_class` and returns the `DockBucket` if
    /// exists.
    ///
    /// # Complexity
    /// O(n)
    pub fn remove_bucket(&self, app_class: &str) -> Option<DockBucket> {
        if let Some((buckets, bucket_idx)) = self.find_bucket_location(app_class) {
            let bucket = buckets.item(bucket_idx)?.downcast::<DockBucket>().ok()?;

            buckets.remove(bucket_idx);
            self.remove_bucket_from_map(app_class);

            return Some(bucket);
        }

        None
    }

    /// Move a bucket to required `index` in `pinned_buckets` if `pinned` is
    /// `true`.
    ///
    /// # Complexity
    /// O(n)
    pub fn move_bucket(&self, app_class: &str, index: u32, pinned: bool) {
        if let Some(bucket) = self.remove_bucket(app_class) {
            self.insert_bucket(bucket, Some(index), pinned);
        }
    }

    /// Remove all instances from a bucket.
    ///
    /// If the bucket is not pinned, then the bucket will be removed.
    ///
    /// # Complexity
    /// - When bucket is not pinned: O(n)
    /// - When bucket is pinned: O(1)
    pub fn clear_bucket(&self, app_class: &str) {
        let is_pinned = self.is_pinned(app_class).unwrap_or(false);

        if !is_pinned {
            self.remove_bucket(app_class);
            return;
        }

        if let Some(bucket) = self.find_bucket(app_class) {
            bucket.clear();
        }
    }

    /// Move a bucket to the `pinned_buckets` at the `index` if it is
    /// not `None`.
    ///
    /// # Complexity
    /// O(n)
    pub fn pin_bucket(&self, app_class: &str, index: Option<u32>) {
        match self.is_pinned(app_class) {
            Some(false) => {
                let bucket = self
                    .remove_bucket(app_class)
                    .expect("The removed item must be a DockBucket");

                self.insert_bucket(bucket, index, true);
                self.save_pinned_buckets();
            }
            // make a new bucket
            None => {
                let icon = self.resolve_icon_from_class(app_class);
                let bucket = DockBucket::new(app_class, icon);

                self.insert_bucket(bucket, index, true);
                self.save_pinned_buckets();
            }
            // already pinned
            Some(true) => {}
        }
    }

    /// Move a bucket to the free `buckets` at the `index` if it is
    /// not `None`.
    ///
    /// # Complexity
    /// O(n)
    pub fn unpin_bucket(&self, app_class: &str, index: Option<u32>) {
        if let Some(is_pinned) = self.is_pinned(app_class) {
            if is_pinned {
                let bucket = self
                    .remove_bucket(app_class)
                    .expect("The removed item must be a DockBucket");

                self.insert_bucket(bucket, index, false);
                self.save_pinned_buckets();
            }
        }
    }

    /// Find a bucket index by the app class.
    ///
    /// # Complexity
    /// O(n)
    fn find_bucket_location(&self, app_class: &str) -> Option<(ListStore, u32)> {
        let buckets = match self.is_pinned(app_class)? {
            false => self.buckets(),
            true => self.pinned_buckets(),
        };

        let n = buckets.n_items();

        for i in 0..n {
            let item = buckets.item(i)?;

            if let Ok(bucket) = item.downcast::<DockBucket>() {
                if bucket.app_class() == app_class {
                    return Some((buckets, i));
                }
            }
        }

        None
    }

    /// Find a bucket by the app class.
    ///
    /// # Complexity
    /// O(n)
    pub fn find_bucket(&self, app_class: &str) -> Option<DockBucket> {
        if let Some((buckets, bucket_idx)) = self.find_bucket_location(app_class) {
            let obj = buckets.item(bucket_idx)?;

            return obj.downcast::<DockBucket>().ok();
        }

        None
    }

    /// Set the current buckets list.
    ///
    /// If the `is_pinned` is `true`, then the bucket is on the
    /// `pinned_buckets`.
    ///
    /// # Complexity
    /// O(1)
    fn set_bucket_on_map(&self, app_class: &str, is_pinned: bool) {
        self.map_is_pinned_mut()
            .insert(app_class.to_string(), is_pinned);
    }

    /// Remove the bucket entry from the map.
    ///
    /// The bucket `app_class` will be removed from the map.
    ///
    /// # Complexity
    /// O(1)
    fn remove_bucket_from_map(&self, app_class: &str) {
        self.map_is_pinned_mut().remove(app_class);
    }

    /// Build a cache to find app icons
    ///
    /// # Complexity
    /// O(n)
    fn resolve_entry(&self, app_class: &str, bucket: &DockBucket) {
        let cache = self.imp().app_entries_cache.clone();
        let app_class_string = app_class.to_string();

        if let Some(_cached_entry) = cache.borrow().get(&app_class_string) {
            return;
        }

        let proxy_ref = self.imp().appd_proxy.clone();
        let bucket_cloned = bucket.clone();

        glib::MainContext::default().spawn_local(async move {
            if let Some(proxy) = proxy_ref.get() {
                match proxy.get_entry(&app_class_string).await {
                    Ok(entry) => {
                        debug!("Inserting app entry {} in cache.", app_class_string);

                        let gicon = gio::ThemedIcon::from_names(&[&entry.icon]);

                        bucket_cloned.set_app_icon(gicon);

                        cache.borrow_mut().insert(app_class_string, entry);
                    }
                    Err(_) => {
                        warn!(
                            "No entry found to app {}. Creating a default Entry.",
                            app_class_string
                        );

                        let default_entry = AppEntryDTO::new(
                            app_class_string.clone(),
                            app_class_string.clone(),
                            "application-x-executable".to_string(),
                            "".to_string(),
                            "".to_string(),
                            "".to_string(),
                            Vec::new(),
                            Vec::new(),
                        );
                        cache.borrow_mut().insert(app_class_string, default_entry);
                        return;
                    }
                };
            } else {
                warn!("Failed to get the AppdProxy. The Proxy is None.");
            }
        });
    }

    /// Find the icon to by the app class.
    ///
    /// # Complexity
    /// O(1)
    fn resolve_icon_from_class(&self, app_class: &str) -> gio::Icon {
        if let Some(entry) = self.imp().app_entries_cache.borrow().get(app_class) {
            return gio::ThemedIcon::from_names(&[&entry.icon]).upcast::<gio::Icon>();
        }

        let icon_names = [
            &app_class.to_lowercase(),
            "application-x-executable", // generic icon
        ];

        gio::ThemedIcon::from_names(&icon_names).upcast::<gio::Icon>()
    }

    /// Load all pinned apps from the default json.
    fn load_pinned_buckets(&self) {
        let Ok(home_path) = std::env::var("HOME") else {
            error!("HOME environment variable is not set. Can not load pinned apps.");
            return;
        };

        let pinned_path = PathBuf::from(home_path).join(Self::XDG_PINNED_JSON_PATH);

        if !pinned_path.exists() {
            warn!("pinned.json file not found. Starting with a empty dock.");
            return;
        }

        let Ok(loaded_file) = std::fs::read_to_string(&pinned_path) else {
            error!("Failed to read pinned.json. Check file permissions.");
            return;
        };

        let Ok(pinned_json) = serde_json::from_str::<PinnedJson>(&loaded_file) else {
            error!("Error to parse the pinned.json file. The file might be corrupted.");
            return;
        };

        for pinned_app in pinned_json.pinned_apps {
            debug!("Loading pinned bucket: {}", pinned_app.app_class);
            self.pin_bucket(&pinned_app.app_class, None);
        }

        info!("Pinned buckets were successfully loaded.");
    }

    /// Save all pinned buckets in a json to read when starts.
    fn save_pinned_buckets(&self) {
        let mut pinned_apps = Vec::<PinnedApp>::new();
        let n = self.pinned_buckets().n_items();
        let buckets = self.pinned_buckets();

        let Ok(home_path) = std::env::var("HOME") else {
            error!("HOME environment variable is not set. Can not save pinned apps.");
            return;
        };

        let pinned_path = PathBuf::from(home_path).join(Self::XDG_PINNED_JSON_PATH);

        if let Some(parent) = pinned_path.parent() {
            if !parent.exists() {
                if let Err(e) = std::fs::create_dir_all(&parent) {
                    error!(
                        "Fail to create the base dir {:?} to save the pinned.json: {}",
                        parent, e
                    );
                    return;
                }

                debug!("Creted the base dir {:?} to save the pinned.json", parent);
            }
        }

        for i in 0..n {
            if let Some(pinned_obj) = buckets.item(i) {
                if let Some(pinned_bucket) = pinned_obj.downcast_ref::<DockBucket>() {
                    pinned_apps.push(PinnedApp {
                        app_class: pinned_bucket.app_class(),
                    });
                }
            }
        }

        let Ok(pinned_json) = serde_json::to_string(&PinnedJson { pinned_apps }) else {
            error!("Error to convert pinned buckets to json.");
            return;
        };

        let file = gtk::gio::File::for_path(&pinned_path);

        let bytes = gtk::glib::Bytes::from(pinned_json.as_bytes());

        gtk::glib::MainContext::default().spawn_local(async move {
            match file
                .replace_contents_bytes_future(
                    &bytes,
                    None,
                    false,
                    gtk::gio::FileCreateFlags::REPLACE_DESTINATION,
                )
                .await
            {
                Ok(_) => {
                    debug!("Pinned buckets saved successfully at {:?}", pinned_path);
                }
                Err(e) => {
                    error!("Error saving pinned buckets to {:?}: {}", pinned_path, e);
                }
            };
        });
    }

    /// Get the pinned status by a `app_class`.
    ///
    /// # Complexity
    /// O(1)
    pub fn is_pinned(&self, app_class: &str) -> Option<bool> {
        self.map_is_pinned().get(app_class).cloned()
    }

    /// Is `true` if have no backets in any list view.
    pub fn is_empty(&self) -> bool {
        self.pinned_buckets().n_items() == 0 && self.buckets().n_items() == 0
    }

    pub fn set_wayland_bridge(&self, bridge: Bridge) {
        self.imp()
            .wayland_bridge
            .set(bridge)
            .expect("The Bridge has already been initialized!");
    }

    /// Set the [`rain_client::appd::entry::AppdProxy`] and retry resolve all
    /// bucket entries.
    pub fn set_appd_proxy(&self, proxy: AppdProxy<'static>) {
        self.imp()
            .appd_proxy
            .set(proxy)
            .expect("The AppdProxy has already been initialized!");

        self.retry_unresolved_entries();
    }

    /// Retry resolve all entries after the Appd connection as soon as it is
    /// established.
    fn retry_unresolved_entries(&self) {
        let buckets = self.buckets();
        let n = buckets.n_items();
        for bucket_idx in 0..n {
            if let Some(bucket_obj) = buckets.item(bucket_idx) {
                if let Some(bucket) = bucket_obj.downcast_ref::<DockBucket>() {
                    self.resolve_entry(&bucket.app_class(), bucket);
                }
            }
        }

        let buckets = self.pinned_buckets();
        let n = buckets.n_items();
        for bucket_idx in 0..n {
            if let Some(bucket_obj) = buckets.item(bucket_idx) {
                if let Some(bucket) = bucket_obj.downcast_ref::<DockBucket>() {
                    self.resolve_entry(&bucket.app_class(), bucket);
                }
            }
        }
    }

    /// Send a command to a Wayland Client.
    ///
    /// This method is a way to send
    /// [`rain_client::wayland::protocols::toplevel::ToplevelCommand`] to
    /// Wayland Thread by the [`rain_client::wayland::Bridge`].
    pub fn send_command(&self, cmd: ToplevelCommand) {
        if let Some(sender) = self.imp().wayland_bridge.get() {
            let _ = sender.send(cmd);
        } else {
            warn!("The Wayland Bridge is not set.");
        }
    }

    /// Receive wayland events using the [`rain_client::wayland::Bridge`].
    ///
    /// Start a task to listen the Wayland Thread and update the dock state.
    ///
    /// # Async
    /// This method is not blocking.
    pub fn recv_wayland_events(&self) {
        if let Some(bridge_ref) = self.imp().wayland_bridge.get() {
            let bridge = bridge_ref.clone();
            let state = self.clone();

            glib::MainContext::default().spawn_local(async move {
                while let Ok(event_raw) = bridge.recv().await {
                    // if is a Erro, then is not a toplevel event
                    if let Ok(event) = ToplevelEvent::try_from(event_raw) {
                        match event {
                            ToplevelEvent::Opened(data) => {
                                let app = DockApp::new(
                                    data.window_id,
                                    &data.header.app_title,
                                    data.state,
                                );
                                state.add_app(&data.header.app_id, app);
                            }
                            ToplevelEvent::Closed(data) => {
                                state.remove_bucket(&data.header.app_id);
                            }
                            ToplevelEvent::StateChanged(data) => {
                                state.process_state_changed(data);
                            }
                        };
                    }
                }
            });
        } else {
            error!("The Wayland Bridge is None. Can not receive wayland events.");
        }
    }

    pub fn change_focus(&self, app_class: &str) {
        // if has a old focus
        if let Some(old_focus_bucket) = self
            .bucket_focused()
            .and_then(|focus| self.find_bucket(&focus))
        {
            old_focus_bucket.set_is_focused(false);
        }

        if let Some(bucket) = self.find_bucket(app_class) {
            bucket.set_is_focused(true);
            self.set_bucket_focused(Some(app_class));
        } else {
            self.set_bucket_focused(None);
        }
    }

    pub fn process_state_changed(&self, data: WindowData) {
        let app_class = &data.header.app_id;
        let is_focused = data.state.is_focused;

        let Some(bucket) = self.find_bucket(app_class) else {
            return;
        };

        bucket.update_app(data.window_id, &data.header.app_title, data.state);

        let current_focus = self.bucket_focused();
        let is_currently_focused = current_focus.as_deref() == Some(app_class);

        match (is_focused, is_currently_focused) {
            // now is focused and before no
            (true, false) => {
                self.change_focus(app_class);
            }
            // now not is focused and before yes
            (false, true) => {
                bucket.set_is_focused(false);
                self.set_bucket_focused(None);
            }
            _ => {}
        };
    }

    // getters //

    fn map_is_pinned(&self) -> Ref<'_, HashMap<String, bool>> {
        self.imp().map_is_pinned.borrow()
    }

    fn map_is_pinned_mut(&self) -> RefMut<'_, HashMap<String, bool>> {
        self.imp().map_is_pinned.borrow_mut()
    }

    // setters //

    fn set_bucket_focused(&self, app_class: Option<&str>) -> Option<String> {
        let app_class = app_class.map(|name| name.to_string());
        self.imp().bucket_focused.replace(app_class)
    }
}
