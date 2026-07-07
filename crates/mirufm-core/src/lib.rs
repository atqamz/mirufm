//! mirufm-core: pure filesystem logic for the mirufm file explorer.
//! No gpui, no async runtime. Callers supply threading.

pub mod fs;
pub mod scheduler;
pub mod sort;
pub mod state;
pub mod watch;
