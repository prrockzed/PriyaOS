//! The module host (02-core §5): finds modules, decides what they may do,
//! and restarts them when they fail. Built over sub-phase 0.8.

pub mod access;
pub mod discovery;
pub mod policy;
