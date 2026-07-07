mod app;

use app::Mirufm;
use gpui::{prelude::*, App, WindowOptions};

fn main() {
    let root = std::env::current_dir().unwrap_or_else(|_| "/".into());
    gpui_platform::application().run(move |cx: &mut App| {
        cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(|cx| Mirufm::new(root, cx))
        })
        .unwrap();
        cx.activate(true);
    });
}
