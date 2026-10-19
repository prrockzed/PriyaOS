//! Fakes, fixtures and helpers shared by PriyaOS tests. A dev-dependency
//! only; nothing here ships.
//!
//! - [`FakeClock`]: time that moves only when a test says so.
//! - [`TempPriyaHome`]: throwaway config, data, runtime and state folders.
//! - [`golden`]: snapshot assertions with ULIDs, timestamps and durations
//!   redacted ([`assert_golden_json!`], [`assert_golden_text!`]).
//! - [`fixture`]: recorded test data, which must say how it was recorded.
//!
//! Later sub-phases add `assert_events!` (0.5, with the event store),
//! `TestDaemon` (0.7), `FakeModuleProcess` (0.8), `FakeModelProvider`,
//! `Cassette` and `mock-priyad` (0.12), `FakeMachine` (2.3) and the fake
//! agent (4.1).

mod clock;
mod fixture;
pub mod golden;
mod home;

pub use clock::FakeClock;
pub use fixture::{RECORDED_NOTE, fixture};
pub use home::TempPriyaHome;

/// Re-exported so the golden macros work without a direct dependency.
#[doc(hidden)]
pub use insta;
