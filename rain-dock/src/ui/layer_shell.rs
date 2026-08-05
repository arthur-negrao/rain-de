use gtk::ApplicationWindow;
use gtk4_layer_shell::{Edge, Layer, LayerShell};

pub fn apply_layer_shell(window: &ApplicationWindow) {
    window.init_layer_shell();

    window.set_layer(Layer::Top);

    window.set_namespace(Some("rain-dock"));

    // push others windows out of the way
    // window.auto_exclusive_zone_enable();

    window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::None);

    let anchors = [
        (Edge::Left, false),
        (Edge::Right, false),
        (Edge::Top, false),
        (Edge::Bottom, true),
    ];

    for (anchor, state) in anchors {
        window.set_anchor(anchor, state);
    }

    window.set_margin(Edge::Bottom, 0);
}
