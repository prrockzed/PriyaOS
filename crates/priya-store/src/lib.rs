//! PriyaOS event store: the hash-chained, append-only log of everything that
//! happens, with projections, subscriptions and blobs (sub-phase 0.5,
//! CORE-01; design in docs/claude/architecture/02-core.md §1).
//!
//! This first part opens the database, keeps its schema current and stores
//! large payloads as blobs. Appending, verification, projections and
//! subscriptions build on it.

pub mod blobs;
pub mod db;
pub mod error;

pub use blobs::{BlobRef, BlobStore, LocalBlobStore};
pub use error::{Result, StoreError};
