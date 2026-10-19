//! PriyaOS event store: the hash-chained, append-only log of everything that
//! happens, with projections, subscriptions and blobs (sub-phase 0.5,
//! CORE-01; design in docs/claude/architecture/02-core.md §1).
//!
//! [`EventStore`] appends validated, hash-chained events and reads them back;
//! [`chain::verify`] proves no row was edited, deleted or reordered;
//! [`projections`] keep read models in step inside each append; and
//! [`EventStore::subscribe`] streams events to modules and surfaces.

pub mod blobs;
pub mod chain;
pub mod db;
pub mod error;
pub mod projections;
mod store;
pub mod subscriptions;

pub use blobs::{BlobRef, BlobStore, LocalBlobStore};
pub use chain::{ChainBreak, ChainReport};
pub use error::{Result, StoreError};
pub use projections::Projection;
pub use store::{BLOB_KEY, BLOB_THRESHOLD, EventRecord, EventStore, StoreConfig};
pub use subscriptions::Filter;
