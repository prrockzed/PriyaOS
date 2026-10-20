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
//! Later sub-phases add `TestDaemon` (0.7), `FakeModuleProcess` (0.8),
//! `FakeModelProvider`, `Cassette` and `mock-priyad` (0.12), `FakeMachine`
//! (2.3) and the fake agent (4.1).

mod child;
mod clock;
pub mod events;
mod fixture;
pub mod golden;
mod home;

pub use child::{LoopChild, ack, until_stdin_closes};
pub use clock::FakeClock;
pub use fixture::{RECORDED_NOTE, fixture};
pub use home::TempPriyaHome;

/// Re-exported so the macros work without a direct dependency.
#[doc(hidden)]
pub use insta;
#[doc(hidden)]
pub use serde_json;
