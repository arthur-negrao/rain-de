use gtk::ApplicationWindow;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

#[derive(Debug, Clone)]
pub struct LayerShellConfig {
    layer: Layer,
    namespace: String,
    keyboard_mode: KeyboardMode,
    anchors: [(Edge, bool); 4],
    margins: [(Edge, i32); 4],
    exclusive_zone: bool,
}

impl LayerShellConfig {
    /// Create a new Config instance.
    pub fn new() -> Self {
        Self {
            layer: Layer::Top,
            namespace: String::from("rain"),
            keyboard_mode: KeyboardMode::None,
            anchors: [
                (Edge::Left, false),
                (Edge::Right, false),
                (Edge::Top, false),
                (Edge::Bottom, false),
            ],
            margins: [
                (Edge::Left, 0),
                (Edge::Right, 0),
                (Edge::Top, 0),
                (Edge::Bottom, 0),
            ],
            exclusive_zone: false,
        }
    }

    pub fn layer(mut self, layer: Layer) -> Self {
        self.layer = layer;

        self
    }

    pub fn namespace(mut self, namespace: &str) -> Self {
        self.namespace = namespace.to_string();

        self
    }

    pub fn margin(mut self, edge: Edge, value: i32) -> Self {
        for (e, v) in &mut self.margins {
            if *e == edge {
                *v = value;
            }
        }

        self
    }

    pub fn margins(mut self, margins: &[(Edge, i32)]) -> Self {
        for (e, v) in &mut self.margins {
            for (edge, value) in margins {
                if *e == *edge {
                    *v = *value;
                }
            }
        }

        self
    }

    pub fn anchor(mut self, edge: Edge, state: bool) -> Self {
        for (e, s) in &mut self.anchors {
            if *e == edge {
                *s = state;
            }
        }

        self
    }

    pub fn anchors(mut self, anchors: &[(Edge, bool)]) -> Self {
        for (e, s) in &mut self.anchors {
            for (edge, state) in anchors {
                if *e == *edge {
                    *s = *state;
                }
            }
        }

        self
    }

    pub fn keyboard_mode(mut self, mode: KeyboardMode) -> Self {
        self.keyboard_mode = mode;

        self
    }

    pub fn exclusive_zone(mut self, state: bool) -> Self {
        self.exclusive_zone = state;

        self
    }

    /// Apply config in a [`gtk::ApplicationWindow`].
    pub fn apply(&self, window: &ApplicationWindow) {
        window.init_layer_shell();

        window.set_layer(self.layer);
        window.set_namespace(Some(&self.namespace));
        window.set_keyboard_mode(self.keyboard_mode);

        if self.exclusive_zone {
            window.auto_exclusive_zone_enable();
        }

        for (anchor, state) in self.anchors {
            window.set_anchor(anchor, state);
        }

        for (edge, margin) in self.margins {
            window.set_margin(edge, margin);
        }
    }
}
