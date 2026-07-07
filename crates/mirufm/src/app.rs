use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};
use std::time::SystemTime;

use gpui::{
    div, prelude::*, px, rgb, uniform_list, Context, IntoElement, Render, ScrollHandle, Window,
};
use mirufm_core::fs::{read_dir, EntryKind};
use mirufm_core::scheduler::{Priority, Scheduler};
use mirufm_core::sort::{sort, SortKey};
use mirufm_core::state::AppState;
use mirufm_core::watch;

pub struct Mirufm {
    state: AppState,
    scheduler: Arc<Scheduler>,
    strip_scroll: ScrollHandle,
    // Parallel to `state.columns`: watches the loaded directory at each depth.
    watchers: Vec<Option<watch::Watcher>>,
}

impl Mirufm {
    pub fn new(root: PathBuf, cx: &mut Context<Self>) -> Self {
        let scheduler = Arc::new(Scheduler::new(4));
        let mut me = Mirufm {
            state: AppState::new(root.clone()),
            scheduler,
            strip_scroll: ScrollHandle::new(),
            watchers: vec![None],
        };
        me.load(root, cx);
        me
    }

    fn descend(&mut self, col: usize, entry_index: usize, cx: &mut Context<Self>) {
        let to_load = self.state.descend(col, entry_index);
        // state.descend() always truncates columns to col + 1; drop the
        // watchers for the columns it just discarded to match.
        self.watchers.truncate(col + 1);
        if let Some(path) = to_load {
            self.watchers.push(None); // matches the Loading column just pushed
            self.load(path, cx);
            self.strip_scroll.scroll_to_item(col + 1);
        }
        cx.notify();
    }

    fn load(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        // Reused entries make back-navigation instant; still refreshed below
        // in case the directory changed since it was cached.
        if let Some(cached) = self.state.cache.get(&path).cloned() {
            self.state
                .set_loaded(&path, cached.entries, cached.loaded_at);
            self.watch_column(&path, cx);
            cx.notify();
        }

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
                this.watch_column(&path, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Starts watching `path` if its column isn't already watched, and hands
    /// each change notification back to the foreground the same way `load`
    /// hands off its background read: an mpsc channel drained by a
    /// `cx.background_spawn` future inside a `cx.spawn` loop.
    fn watch_column(&mut self, path: &Path, cx: &mut Context<Self>) {
        let Some(idx) = self.state.columns.iter().position(|c| c.path == path) else {
            return;
        };
        if matches!(self.watchers.get(idx), Some(Some(_))) {
            return; // already watching this column
        }

        let (tx, rx) = mpsc::channel::<()>();
        let watcher = match watch::watch(path, move || {
            let _ = tx.send(());
        }) {
            Ok(w) => w,
            Err(_) => return,
        };
        self.watchers[idx] = Some(watcher);

        let watched = path.to_path_buf();
        cx.spawn(async move |this, cx| {
            let mut rx = rx;
            loop {
                let (returned_rx, changed) = cx
                    .background_spawn(async move {
                        let changed = rx.recv().is_ok();
                        (rx, changed)
                    })
                    .await;
                rx = returned_rx;
                if !changed {
                    return; // watcher dropped (column truncated or replaced)
                }
                let reload = watched.clone();
                if this.update(cx, |this, cx| this.load(reload, cx)).is_err() {
                    return; // view gone
                }
            }
        })
        .detach();
    }
}

impl Render for Mirufm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let breadcrumb = self
            .state
            .columns
            .last()
            .map(|c| c.path.display().to_string())
            .unwrap_or_default();

        let columns = (0..self.state.columns.len())
            .map(|col| {
                let entries = self.state.columns[col].entries.clone();
                let selection = self.state.columns[col].selection;
                div()
                    .w(px(256.))
                    .flex_none()
                    .h_full()
                    .border_r_1()
                    .border_color(rgb(0x333333))
                    .child(
                        uniform_list(
                            ("col", col),
                            entries.len(),
                            cx.processor(move |_this, range: Range<usize>, _window, cx| {
                                range
                                    .map(|i| {
                                        let e = &entries[i];
                                        let label = if e.kind == EntryKind::Dir {
                                            format!("{}/", e.name)
                                        } else {
                                            e.name.clone()
                                        };
                                        div()
                                            .id(i)
                                            .px_2()
                                            .py_1()
                                            .cursor_pointer()
                                            .when(Some(i) == selection, |d| d.bg(rgb(0x3a5fcd)))
                                            .text_color(rgb(0xdddddd))
                                            .on_click(cx.listener(
                                                move |this, _event, _window, cx| {
                                                    this.descend(col, i, cx);
                                                },
                                            ))
                                            .child(label)
                                    })
                                    .collect::<Vec<_>>()
                            }),
                        )
                        .h_full(),
                    )
            })
            .collect::<Vec<_>>();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1e1e1e))
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_color(rgb(0x999999))
                    .child(breadcrumb),
            )
            .child(
                div()
                    .id("strip")
                    .flex()
                    .flex_row()
                    .flex_1()
                    .overflow_x_scroll()
                    .track_scroll(&self.strip_scroll)
                    .children(columns),
            )
    }
}
