//! Fakes, fixtures and helpers shared by PriyaOS tests. A dev-dependency
//! only; nothing here ships.
//!
//! - [`FakeClock`]: time that moves only when a test says so.
//! - [`TempPriyaHome`]: throwaway config, data, runtime and state folders.
//! - [`golden`]: snapshot assertions with ULIDs, timestamps and durations
//!   redacted ([`assert_golden_json!`], [`assert_golden_text!`]).
//! - [`fixture`]: recorded test data, which must say how it was recorded.
//! - [`LoopChild`]: re-runs the test binary as a child that loops until
//!   killed, for crash tests.
//! - [`assert_events!`]: which events were recorded, in order, and what their
//!   payloads say; [`events::setting_event`] makes valid events in bulk.
//!
//! - [`TestDaemon`]: the real priyad in a temporary home, for end-to-end
//!   tests ([`workspace_binary`] finds it).
//!
//! - [`FakeModuleProcess`]: a module that misbehaves on purpose (crashes,
//!   hangs, garbage, lies), for module host tests.
//!
//! - [`FakeModelProvider`]: scripted model answers, and [`model_contract`],
//!   what every model provider must do.
//! - [`http_script::ScriptedHttp`]: a local HTTP server playing scripted
//!   replies, for HTTP adapters, and [`cassette::Cassette`], a real
//!   provider exchange recorded once and replayed through it.
//!
//! Later sub-phases add `mock-priyad` (0.12), `FakeMachine`
//! (2.3) and the fake agent (4.1).

pub mod cassette;
mod child;
mod clock;
#[cfg(unix)]
mod daemon;
pub mod events;
#[cfg(unix)]
pub mod fake_module;
mod fixture;
pub mod golden;
mod home;
pub mod http_script;
mod model;
pub mod model_contract;

pub use child::{LoopChild, ack, until_stdin_closes};
pub use clock::FakeClock;
#[cfg(unix)]
pub use daemon::{TestDaemon, workspace_binary};
#[cfg(unix)]
pub use fake_module::FakeModuleProcess;
pub use fixture::{RECORDED_NOTE, fixture};
pub use home::TempPriyaHome;
pub use model::{FakeModelProvider, Reply, Step};

/// A mutex's value, even if a test panicked while holding it: the fakes
/// keep plain data, which stays valid.
pub(crate) fn lock<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// Re-exported so the macros work without a direct dependency.
#[doc(hidden)]
pub use insta;
#[doc(hidden)]
pub use serde_json;
