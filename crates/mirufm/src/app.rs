use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};
use std::time::SystemTime;

use gpui::{div, prelude::*, rgb, uniform_list, Context, IntoElement, Render, Window};
use mirufm_core::fs::{read_dir, EntryKind};
use mirufm_core::scheduler::{Priority, Scheduler};
use mirufm_core::sort::{sort, SortKey};
use mirufm_core::state::AppState;

pub struct Mirufm {
    state: AppState,
    scheduler: Arc<Scheduler>,
}

impl Mirufm {
    pub fn new(root: PathBuf, cx: &mut Context<Self>) -> Self {
        let scheduler = Arc::new(Scheduler::new(4));
        let mut me = Mirufm {
            state: AppState::new(root.clone()),
            scheduler,
        };
        me.load(root, cx);
        me
    }

    fn load(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let p = path.clone();
        self.scheduler.spawn(Priority::Visible, move |_cancel| {
            if let Ok(mut entries) = read_dir(&p, &cancel) {
                sort(&mut entries, SortKey::Name, true);
                let _ = tx.send(entries);
            }
        });
        // Scheduler runs on its own thread pool; hand the result to the UI
        // thread via a background executor task that blocks on the channel.
        cx.spawn(async move |this, cx| {
            let Ok(entries) = cx.background_spawn(async move { rx.recv() }).await else {
                return;
            };
            this.update(cx, |this, cx| {
                this.state.set_loaded(&path, entries, SystemTime::now());
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for Mirufm {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let entries = self.state.columns[0].entries.clone();
        div().flex().size_full().bg(rgb(0x1e1e1e)).child(
            uniform_list("col0", entries.len(), move |range, _window, _cx| {
                range
                    .map(|i| {
                        let e = &entries[i];
                        let label = if e.kind == EntryKind::Dir {
                            format!("{}/", e.name)
                        } else {
                            e.name.clone()
                        };
                        div().px_2().py_1().text_color(rgb(0xdddddd)).child(label)
                    })
                    .collect::<Vec<_>>()
            })
            .h_full(),
        )
    }
}
