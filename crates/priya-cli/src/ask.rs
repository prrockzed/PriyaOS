//! `priya ask "<question>"`: the answer streams to stdout as plain text, or
//! as one `v1.intent.stream` step per line with `--json`. Ctrl+C stops the
//! answer (a second one leaves at once); `-c` continues the last
//! conversation; `--model <provider>/<model>` overrides `ai.strategy`.

use std::path::{Path, PathBuf};

use priya_api::client::Client;
use priya_api::methods::intent::{
    self, CancelParams, IntentContext, IntentOverrides, STREAM_NOTIFICATION, StepKind, StreamStep,
    SubmitParams,
};
use priya_api::rpc::Request;
use tokio::signal::unix::{SignalKind, signal};

use crate::run::{Failure, Out};

/// The file that remembers the last conversation, for `-c`.
pub fn last_conversation_file(state: &Path) -> PathBuf {
    state.join("last-conversation")
}

pub struct Ask {
    pub question: String,
    pub model: Option<String>,
    /// Continue this conversation (from `-c`).
    pub conversation: Option<String>,
    /// Where to note the conversation for the next `-c`.
    pub remember: Option<PathBuf>,
}

/// How long stopping may take before `ask` gives up waiting.
const CANCEL_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// Asks and prints the answer as it streams.
pub async fn ask(client: &mut Client, ask: Ask, out: &mut Out<'_>) -> Result<(), Failure> {
    // Listening before asking, so an early Ctrl+C isn't lost.
    let mut interrupt = signal(SignalKind::interrupt())
        .map_err(|e| Failure::Refused(format!("Ctrl+C can't be caught: {e}")))?;
    let params = SubmitParams {
        text: ask.question,
        context: Some(IntentContext {
            conversation_id: ask.conversation,
            cwd: std::env::current_dir()
                .ok()
                .map(|d| d.display().to_string()),
            surface: Some("cli".into()),
        }),
        overrides: ask
            .model
            .map(|model| IntentOverrides { model: Some(model) }),
    };
    let submitted = client.call::<intent::Submit>(&params).await?;
    // Noted now, so `-c` follows this question even if it fails or stops.
    // Best effort: without it a follow-up starts a new conversation.
    if let Some(file) = &ask.remember {
        let _ = file
            .parent()
            .map(std::fs::create_dir_all)
            .transpose()
            .and_then(|_| std::fs::write(file, &submitted.conversation_id));
    }
    let cancel = CancelParams {
        intent_id: submitted.intent_id.clone(),
    };
    let mut stopping = false;
    // Whether text was printed, so the answer's line gets its end.
    let mut printed = false;
    loop {
        let next = tokio::select! {
            next = client.next_notification() => next?,
            _ = interrupt.recv() => {
                if stopping {
                    return Err(Failure::Interrupted);
                }
                stopping = true;
                // It may have just finished: the last step says how it ended.
                // A second Ctrl+C while waiting leaves at once.
                tokio::select! {
                    _ = tokio::time::timeout(CANCEL_WAIT, client.call::<intent::Cancel>(&cancel)) => {}
                    _ = interrupt.recv() => return Err(Failure::Interrupted),
                }
                continue;
            }
        };
        let Some(notification) = next else {
            return Err(Failure::Refused(
                "Lost the connection to Priya's background service, so the answer stopped.".into(),
            ));
        };
        let Some(step) = step_of(notification, &submitted.intent_id) else {
            continue;
        };
        if out.json {
            out.json(&step);
        }
        match step.kind {
            StepKind::Delta if !out.json => {
                out.text(step.text.as_deref().unwrap_or_default());
                printed = true;
            }
            StepKind::Done => {
                if printed {
                    out.line("");
                }
                return Ok(());
            }
            StepKind::Error => {
                if printed {
                    out.line("");
                }
                let message = step.error.map(|e| e.user_message);
                return Err(Failure::Refused(
                    message.unwrap_or_else(|| "The answer stopped.".into()),
                ));
            }
            StepKind::Cancelled => {
                if printed {
                    out.line("");
                }
                // Our Ctrl+C, or someone else (priyad stopping).
                return Err(if stopping {
                    Failure::Interrupted
                } else {
                    Failure::Refused("Stopped.".into())
                });
            }
            _ => {}
        }
        if out.closed {
            // The reader went away (`| head`): stop paying for the rest.
            let _ = tokio::time::timeout(CANCEL_WAIT, client.call::<intent::Cancel>(&cancel)).await;
            return Ok(());
        }
    }
}

/// The step in a notification, if it is one of this intent's.
fn step_of(notification: Request, intent_id: &str) -> Option<StreamStep> {
    if notification.method != STREAM_NOTIFICATION {
        return None;
    }
    let step: StreamStep = serde_json::from_value(notification.params?).ok()?;
    (step.intent_id == intent_id).then_some(step)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    // covers: CORE-04
    // suite: unit
    #[test]
    fn only_this_intents_steps_are_read() {
        let step = |id: &str| {
            Request::notification(
                STREAM_NOTIFICATION,
                json!({"intent_id": id, "seq": 1, "kind": "delta", "text": "Hi"}),
            )
        };
        let mine = step_of(step("01A"), "01A").unwrap();
        assert_eq!(mine.kind, StepKind::Delta);
        assert_eq!(mine.text.as_deref(), Some("Hi"));
        assert!(step_of(step("01B"), "01A").is_none());
        let other = Request::notification("v1.events.event", json!({}));
        assert!(step_of(other, "01A").is_none());
        assert_eq!(
            last_conversation_file(Path::new("/s")),
            PathBuf::from("/s/last-conversation")
        );
    }
}
