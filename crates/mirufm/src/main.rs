use gpui::{div, prelude::*, rgb, App, Context, Render, Window, WindowOptions};

struct Mirufm;

impl Render for Mirufm {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .bg(rgb(0x1e1e1e))
            .size_full()
            .justify_center()
            .items_center()
            .text_color(rgb(0xffffff))
            .child("mirufm")
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Mirufm))
            .unwrap();
        cx.activate(true);
    });
}
