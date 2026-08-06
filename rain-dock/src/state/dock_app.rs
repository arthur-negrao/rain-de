use std::cell::{Cell, RefCell};

use gtk::glib;
use gtk::glib::subclass::prelude::*;
use gtk::prelude::*;

use rain_client::wayland::protocols::toplevel::WindowState;

mod imp {
    use std::cell::OnceCell;

    use gtk::glib::{Properties, subclass::types::ObjectSubclass};

    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::DockApp)]
    pub struct DockApp {
        #[property(get)]
        pub id: OnceCell<u32>,

        #[property(get)]
        pub title: RefCell<String>,

        #[property(get, set)]
        pub is_focused: Cell<bool>,

        #[property(get, set)]
        pub is_fullscreen: Cell<bool>,

        #[property(get, set)]
        pub is_minimized: Cell<bool>,

        #[property(get, set)]
        pub is_maximized: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DockApp {
        const NAME: &'static str = "DockApp";
        type Type = super::DockApp;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for DockApp {
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
    /// A state of a app instance.
    pub struct DockApp(ObjectSubclass<imp::DockApp>);
}

impl DockApp {
    /// Create a new instance of a DockApp.
    pub fn new(id: u32, title: &str, state: WindowState) -> Self {
        let obj: Self = glib::Object::builder().build();

        let imp = obj.imp();
        imp.id.set(id).expect("Error to init the App instance ID");
        imp.title.replace(title.to_string());
        imp.is_focused.set(state.is_focused);
        imp.is_fullscreen.set(state.is_fullscreen);
        imp.is_maximized.set(state.is_maximized);
        imp.is_minimized.set(state.is_minimized);

        obj
    }

    pub fn update_title(&self, title: &str) {
        self.imp().title.replace(title.to_string());
    }
}
