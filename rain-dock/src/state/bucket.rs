use std::cell::{OnceCell, Ref, RefCell, RefMut};
use std::collections::HashMap;

use crate::shared::event::WindowState;

use super::dock_app::DockApp;
use gtk::gio;
use gtk::glib;
use gtk::glib::subclass::prelude::*;
use gtk::prelude::*;

mod imp {
    use std::{cell::Cell, collections::HashMap};

    use gtk::glib::{Properties, subclass::types::ObjectSubclass};

    use super::*;

    #[derive(Debug, Properties)]
    #[properties(wrapper_type = super::DockBucket)]
    pub struct DockBucket {
        #[property(get)]
        pub app_class: OnceCell<String>,

        #[property(get, set)]
        pub app_icon: RefCell<gtk::gio::Icon>,

        #[property(get, set)]
        pub last_focus: Cell<u32>,

        #[property(get, set)]
        pub is_focused: Cell<bool>,

        #[property(get, set)]
        pub is_running: Cell<bool>,

        pub bucket: RefCell<HashMap<u32, DockApp>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DockBucket {
        const NAME: &'static str = "DockBucket";
        type Type = super::DockBucket;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for DockBucket {
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

    impl Default for DockBucket {
        fn default() -> Self {
            Self {
                app_class: OnceCell::new(),
                app_icon: RefCell::new(
                    gio::ThemedIcon::from_names(&["application-x-executable"])
                        .upcast::<gio::Icon>(),
                ),
                last_focus: Cell::new(0),
                is_focused: Cell::new(false),
                is_running: Cell::new(false),
                bucket: RefCell::new(HashMap::default()),
            }
        }
    }
}

glib::wrapper! {
    /// A bucket to store all app instances
    pub struct DockBucket(ObjectSubclass<imp::DockBucket>);
}

impl DockBucket {
    /// Create a new instance of a DockBucket.
    pub fn new(app_class: &str, app_icon: gio::Icon) -> Self {
        let obj: Self = glib::Object::builder().build();

        let imp = obj.imp();
        imp.app_class
            .set(app_class.to_string())
            .expect("Fail to init the app_class");
        imp.app_icon.replace(app_icon);
        imp.is_focused.set(false);
        imp.is_running.set(false);
        imp.bucket.replace(HashMap::new());

        obj
    }

    /// Insert a app instance into the bucket.
    ///
    /// # Complexity
    /// O(1)
    pub fn insert(&self, app: DockApp) {
        let mut bucket = self.bucket_mut();

        self.set_last_focus(app.id());

        bucket.insert(app.id(), app);

        self.set_is_running(true);
    }

    /// Remove a app instance into the bucket.
    ///
    /// # Complexity
    /// O(1)
    pub fn remove(&self, window_id: u32) -> Option<DockApp> {
        let mut bucket = self.bucket_mut();

        if self.last_focus() == window_id {
            self.set_last_focus(bucket.values().next().map(|app| app.id()).unwrap_or(0));
        }

        let removed = bucket.remove(&window_id);

        if bucket.is_empty() {
            self.set_is_running(false);
        }

        removed
    }

    pub fn update_app(&self, window_id: u32, app_title: &str, app_state: WindowState) {
        let mut bucket = self.bucket_mut();

        let app = DockApp::new(window_id, app_title, app_state);

        if let Some(old_app) = bucket.get_mut(&app.id()) {
            old_app.update_title(&app.title());
            old_app.set_is_focused(app.is_focused());
            old_app.set_is_fullscreen(app.is_fullscreen());
            old_app.set_is_minimized(app.is_minimized());
            old_app.set_is_maximized(app.is_maximized());
        }

        if app.is_focused() {
            self.set_last_focus(app.id());
        }
    }

    /// Remove all instances from the bucket.
    pub fn clear(&self) {
        self.bucket_mut().clear();
        self.set_last_focus(0);
    }

    pub fn is_empty(&self) -> bool {
        self.bucket().is_empty()
    }

    pub fn len(&self) -> usize {
        self.bucket().len()
    }

    // getters //

    /// Get a bucket ref
    pub fn bucket(&self) -> Ref<'_, HashMap<u32, DockApp>> {
        self.imp().bucket.borrow()
    }

    /// Get a mutable bucket ref
    pub fn bucket_mut(&self) -> RefMut<'_, HashMap<u32, DockApp>> {
        self.imp().bucket.borrow_mut()
    }
}
