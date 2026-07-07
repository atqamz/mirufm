mod app;

use app::Mirufm;
use gpui::{prelude::*, App, WindowOptions};
use std::sync::Mutex;

fn state_dir() -> std::path::PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".local/state"))
        })
        .unwrap_or_else(|| std::path::PathBuf::from(".local/state"));
    base.join("mirufm")
}

fn init_observability() {
    let dir = state_dir();
    std::fs::create_dir_all(&dir).ok();
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("log"))
        .ok();

    if let Some(log_file) = log_file {
        tracing_subscriber::fmt()
            .with_writer(Mutex::new(log_file))
            .with_ansi(false)
            .init();
    }

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(target: "panic", "{info}");
        default_hook(info);
    }));
}

fn main() {
    init_observability();
    tracing::info!("mirufm starting");
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
