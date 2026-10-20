//! `v1.intent.submit` and `v1.intent.cancel`: each question is answered in
//! its own task, and its steps go back to the asking connection as
//! `v1.intent.stream` notifications.
//!
//! Two tasks per question: the answer, which always runs to its end and
//! records it, and the forwarder, held by the connection like a
//! subscription. When the connection closes, the forwarder is stopped and
//! its guard cancels the answer, so nobody pays for an answer nobody reads.

use std::sync::Arc;

use priya_api::error::{ApiError, ErrorKind};
use priya_api::methods::intent::{
    MAX_TEXT_CHARS, STREAM_NOTIFICATION, StepKind, StreamStep, SubmitParams, Submitted, TokenUsage,
};
use priya_core::settings::Ai;
use tokio::sync::{mpsc, oneshot};
use ulid::Ulid;

use super::answer::{Answerer, Outcome, Question, Stop, Update};
use super::running::Running;
use crate::rpc::Session;

/// The most one connection may have running.
pub const MAX_PER_CONNECTION: usize = 8;
/// How the connection names an intent's forwarder among its holds.
pub const HOLD_PREFIX: &str = "intent-";

/// Starts answering; the reply comes before any step.
pub fn submit(
    answerer: &Arc<Answerer>,
    running: &Arc<Running>,
    ai: Ai,
    params: SubmitParams,
    session: &Session,
) -> Result<Submitted, ApiError> {
    let text = params.text.trim().to_owned();
    if text.is_empty() {
        return Err(ApiError::new(
            ErrorKind::InvalidInput,
            "Ask something first.",
        ));
    }
    if text.chars().count() > MAX_TEXT_CHARS {
        return Err(ApiError::new(
            ErrorKind::InvalidInput,
            format!("That's too long to ask at once; keep it under {MAX_TEXT_CHARS} characters."),
        ));
    }
    let context = params.context.unwrap_or_default();
    let conversation = match context.conversation_id {
        Some(id) if valid_name(&id) => id,
        Some(_) => {
            return Err(ApiError::new(
                ErrorKind::InvalidInput,
                "A conversation id is up to 64 letters, digits, -, _, : and .",
            ));
        }
        None => Ulid::generate().to_string(),
    };
    let surface = match context.surface {
        Some(surface) if valid_name(&surface) => surface,
        Some(_) => {
            return Err(ApiError::new(
                ErrorKind::InvalidInput,
                "A surface name is up to 64 letters, digits, -, _, : and .",
            ));
        }
        None => "api".into(),
    };
    if session.held_with_prefix(HOLD_PREFIX) >= MAX_PER_CONNECTION {
        return Err(ApiError::new(
            ErrorKind::Unavailable,
            format!(
                "This connection already has {MAX_PER_CONNECTION} answers going; wait for one to finish."
            ),
        ));
    }
    let id = Ulid::generate();
    let intent_id = id.to_string();
    let question = Question {
        id,
        text,
        conversation: conversation.clone(),
        surface,
        model: params.overrides.and_then(|o| o.model),
        role: None,
    };

    let stopped = running.add(&intent_id)?;
    let (updates, steps) = mpsc::channel(64);
    let (done, outcome) = oneshot::channel();
    running.spawn({
        let answerer = answerer.clone();
        let running = running.clone();
        let intent_id = intent_id.clone();
        async move {
            // Frees its place even if answering panics.
            let _place = RemoveOnDrop { running, intent_id };
            let outcome = answerer.answer(&ai, question, updates, stopped).await;
            let _ = done.send(outcome);
        }
    });
    let hold = format!("{HOLD_PREFIX}{intent_id}");
    // Asked for here, before the reply is sent: from inside the task it
    // could come too late and wait forever.
    let replied = session.after_reply();
    let forwarder = tokio::spawn(forward(
        replied,
        session.clone(),
        hold.clone(),
        intent_id.clone(),
        steps,
        outcome,
        CancelOnDrop {
            running: running.clone(),
            intent_id: intent_id.clone(),
        },
    ));
    session.hold(hold, forwarder.abort_handle());
    Ok(Submitted {
        intent_id,
        conversation_id: conversation,
    })
}

/// Names that go into events and stream names: ids (ULIDs or similar) and
/// surfaces such as `cli` or `module:weather`. No spaces or anything that
/// could confuse a stream name or a log line.
fn valid_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b':' | b'.'))
}

/// Frees an answer's place among the running ones when its task ends,
/// however it ends.
struct RemoveOnDrop {
    running: Arc<Running>,
    intent_id: String,
}

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        self.running.remove(&self.intent_id);
    }
}

/// Stops the answer when the forwarder ends early (its connection closed).
/// After a normal end the answer is already over and this does nothing.
struct CancelOnDrop {
    running: Arc<Running>,
    intent_id: String,
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.running.cancel(&self.intent_id, Stop::ConnectionClosed);
    }
}

async fn forward(
    replied: oneshot::Receiver<()>,
    session: Session,
    hold: String,
    intent_id: String,
    mut updates: mpsc::Receiver<Update>,
    outcome: oneshot::Receiver<Outcome>,
    _guard: CancelOnDrop,
) {
    // The asker needs the intent id before the first step.
    let _ = replied.await;
    let mut seq = 0;
    let mut send = |step: StreamStep| {
        seq += 1;
        let step = StreamStep { seq, ..step };
        let session = session.clone();
        let hold = hold.clone();
        async move { session.notify_for(&hold, STREAM_NOTIFICATION, &step).await }
    };
    let blank = |kind| StreamStep {
        intent_id: intent_id.clone(),
        seq: 0,
        kind,
        text: None,
        model: None,
        provider: None,
        local: None,
        usage: None,
        error: None,
    };
    while let Some(update) = updates.recv().await {
        let step = match update {
            Update::Started {
                provider,
                model,
                local,
            } => StreamStep {
                model: Some(model),
                provider: Some(provider),
                local: Some(local),
                ..blank(StepKind::Started)
            },
            Update::Delta(text) => StreamStep {
                text: Some(text),
                ..blank(StepKind::Delta)
            },
            Update::Usage(usage) => StreamStep {
                usage: Some(TokenUsage {
                    input: usage.input,
                    output: usage.output,
                    cached: usage.cached,
                }),
                ..blank(StepKind::Usage)
            },
        };
        if !send(step).await {
            return;
        }
    }
    let last = match outcome.await {
        Ok(Outcome::Answered { text, model }) => StreamStep {
            text: Some(text),
            model: Some(model),
            ..blank(StepKind::Done)
        },
        Ok(Outcome::Cancelled { .. }) => blank(StepKind::Cancelled),
        Ok(Outcome::Failed(error)) => StreamStep {
            error: Some(error),
            ..blank(StepKind::Error)
        },
        // The answer task panicked: say so rather than go quiet.
        Err(_) => StreamStep {
            error: Some(ApiError::new(
                ErrorKind::Internal,
                "Something went wrong while answering.",
            )),
            ..blank(StepKind::Error)
        },
    };
    send(last).await;
    session.cancel(&hold);
}
