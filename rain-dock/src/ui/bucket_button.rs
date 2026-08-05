use gtk::glib;
use gtk::glib::Properties;
use gtk::glib::subclass::types::ObjectSubclass;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::{OnceCell, RefCell};

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::BucketButton)]
    pub struct BucketButton {
        #[property(get)]
        pub box_container: OnceCell<gtk::Box>,

        #[property(get)]
        pub icon: OnceCell<gtk::Image>,

        #[property(get)]
        pub indicator: OnceCell<gtk::Box>,

        #[property(get)]
        pub popover: OnceCell<gtk::Popover>,

        pub signals_handlers: RefCell<Vec<(glib::Object, glib::SignalHandlerId)>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BucketButton {
        const NAME: &'static str = "BucketButton";
        type Type = super::BucketButton;
        type ParentType = gtk::Button;
    }

    impl ObjectImpl for BucketButton {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }
    }
    impl WidgetImpl for BucketButton {}
    impl ButtonImpl for BucketButton {}
}

glib::wrapper! {
    pub struct BucketButton(ObjectSubclass<imp::BucketButton>)
        @extends gtk::Button, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl BucketButton {
    pub fn new(
        box_container: &gtk::Box,
        icon: &gtk::Image,
        indicator: &gtk::Box,
        popover: &gtk::Popover,
    ) -> Self {
        let obj: Self = glib::Object::builder().build();

        let imp = obj.imp();

        imp.box_container.set(box_container.clone()).unwrap();
        imp.icon.set(icon.clone()).unwrap();
        imp.indicator.set(indicator.clone()).unwrap();
        imp.popover.set(popover.clone()).unwrap();

        obj.add_css_class("bucket-btn");

        obj
    }

    pub fn store_signal(&self, source: &glib::Object, signal: gtk::glib::SignalHandlerId) {
        self.imp()
            .signals_handlers
            .borrow_mut()
            .push((source.clone(), signal));
    }

    pub fn clear_signals(&self) {
        for (source, signal) in self.imp().signals_handlers.borrow_mut().drain(..) {
            source.disconnect(signal);
        }
    }
}
