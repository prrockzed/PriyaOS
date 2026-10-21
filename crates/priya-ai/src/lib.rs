//! Priya AI: model providers, the model catalog and routing. Sub-phase 0.12
//! adds minimal model access for `priya ask`; Phase 1 builds the rest.

pub mod error;
pub mod gateway;
pub mod keys;
pub mod model;
pub mod openai_compatible;
pub mod provider;
pub mod reliability;

pub use error::{ProviderError, ProviderErrorKind};
pub use model::{FinishReason, Message, ModelRequest, ReasoningEffort, Role, StreamEvent, Usage};
pub use provider::{Health, ModelProvider, ModelStream};
pub use reliability::{Reliable, Timeouts};
