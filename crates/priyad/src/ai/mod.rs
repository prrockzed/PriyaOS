//! Priya AI inside priyad: answering questions (0.12).

pub mod answer;
pub mod record;
pub mod running;

use std::path::Path;
use std::sync::Arc;

use priya_ai::Timeouts;
use priya_ai::gateway::Connector;
use priya_ai::keys::SecretStore;
use priya_core::clock::Clock;
use priya_store::EventStore;
use time::UtcOffset;
use tracing::warn;

use self::answer::Answerer;

/// Where the answer prompt is installed.
pub const PROMPT_FILE: &str = "/usr/share/priya/prompts/answer.md";

/// The answer prompt from `path`, or the copy built into priyad when it
/// can't be read (a development run, or a broken install).
pub fn prompt(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| {
        warn!(path = %path.display(), error = %e, "using the built-in answer prompt");
        priya_ai::gateway::prompt::BUILT_IN.to_owned()
    })
}

/// The answerer priyad uses: real providers, keys from `keys`.
pub fn answerer(
    keys: Arc<dyn SecretStore>,
    store: EventStore,
    clock: Arc<dyn Clock>,
    prompt: String,
    offset: UtcOffset,
) -> Result<Answerer, priya_ai::ProviderError> {
    let connector = Connector::new(keys, Timeouts::default())?;
    Ok(Answerer::new(
        Arc::new(connector),
        store,
        clock,
        prompt,
        offset,
    ))
}
