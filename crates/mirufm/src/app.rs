use std::future::Future;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc, Mutex};
use std::task::{Context as TaskContext, Poll, Waker};
use std::thread;
use std::time::SystemTime;

use gpui::{
    div, prelude::*, px, rgb, uniform_list, AnyElement, Context, IntoElement, Render, ScrollHandle,
    Window,
};
use mirufm_core::fs::{read_dir, Entry, EntryKind};
use mirufm_core::scheduler::{Priority, Scheduler};
use mirufm_core::sort::{sort, SortKey};
use mirufm_core::state::{AppState, Stage};
use mirufm_core::watch;

/// Runs `f` on its own dedicated thread and returns a future that completes
/// with its result. Unlike `cx.background_spawn(async { blocking_call() })`,
/// this never parks a gpui background-executor pool thread on a blocking
/// call: the blocking work happens on a throwaway thread (same pattern as
/// watch.rs's coalescing thread), and the future just waits to be woken.
fn spawn_blocking<T, F>(f: F) -> impl Future<Output = T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    struct Shared<T> {
        value: Option<T>,
        waker: Option<Waker>,
    }

    struct BlockingFuture<T>(Arc<Mutex<Shared<T>>>);

    impl<T> Future for BlockingFuture<T> {
        type Output = T;
        fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<T> {
            let mut shared = self.0.lock().unwrap();
            match shared.value.take() {
                Some(v) => Poll::Ready(v),
                None => {
                    shared.waker = Some(cx.waker().clone());
                    Poll::Pending
                }
            }
        }
    }

    let shared = Arc::new(Mutex::new(Shared {
        value: None,
        waker: None,
    }));
    let producer = Arc::clone(&shared);
    thread::spawn(move || {
        let value = f();
        let mut shared = producer.lock().unwrap();
        shared.value = Some(value);
        if let Some(waker) = shared.waker.take() {
            waker.wake();
        }
    });
    BlockingFuture(shared)
}

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
        // state.descend() may leave columns untouched (stale click index),
        // truncate to col + 1, or truncate then push a new column; derive
        // the watcher length from the actual result instead of assuming.
        self.watchers
            .truncate(self.state.columns.len() - usize::from(to_load.is_some()));
        if let Some(path) = to_load {
            self.watchers.push(None); // matches the Loading column just pushed
            self.load(path, cx);
            self.strip_scroll.scroll_to_item(col + 1);
        }
        debug_assert_eq!(self.watchers.len(), self.state.columns.len());
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

        let (tx, rx) = mpsc::channel::<Result<Vec<Entry>, String>>();
        let cancel = Arc::new(AtomicBool::new(false));
        let p = path.clone();
        self.scheduler.spawn(Priority::Visible, move |_cancel| {
            let result = read_dir(&p, &cancel).map(|mut entries| {
                sort(&mut entries, SortKey::Name, true);
                entries
            });
            let _ = tx.send(result.map_err(|e| e.to_string()));
        });
        // Scheduler runs on its own thread pool; hand the result to the UI
        // thread via spawn_blocking so no gpui executor thread blocks on the
        // channel recv.
        cx.spawn(async move |this, cx| {
            let Ok(result) = spawn_blocking(move || rx.recv()).await else {
                return;
            };
            this.update(cx, |this, cx| {
                match result {
                    Ok(entries) => {
                        this.state.set_loaded(&path, entries, SystemTime::now());
                        this.watch_column(&path, cx);
                    }
                    Err(message) => this.state.set_error(&path, message),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Starts watching `path` if its column isn't already watched, and hands
    /// each change notification back to the foreground the same way `load`
    /// hands off its background read: an mpsc channel drained via
    /// `spawn_blocking` inside a `cx.spawn` loop, so no gpui executor thread
    /// is parked for the column's lifetime waiting on the next change.
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
                let (returned_rx, changed) = spawn_blocking(move || {
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
                let column = &self.state.columns[col];
                let selection = column.selection;
                let entry_count = column.entries.len();

                let body: AnyElement = if let Stage::Error(message) = &column.stage {
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .h_full()
                        .px_2()
                        .text_color(rgb(0xcc4444))
                        .child(message.clone())
                        .into_any_element()
                } else {
                    uniform_list(
                        ("col", col),
                        entry_count,
                        cx.processor(move |this, range: Range<usize>, _window, cx| {
                            let Some(column) = this.state.columns.get(col) else {
                                return Vec::new();
                            };
                            range
                                .filter_map(|i| {
                                    let e = column.entries.get(i)?;
                                    let label = if e.kind == EntryKind::Dir {
                                        format!("{}/", e.name)
                                    } else {
                                        e.name.clone()
                                    };
                                    Some(
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
                                            .child(label),
                                    )
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                    .h_full()
                    .into_any_element()
                };

                div()
                    .w(px(256.))
                    .flex_none()
                    .h_full()
                    .border_r_1()
                    .border_color(rgb(0x333333))
                    .child(body)
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
