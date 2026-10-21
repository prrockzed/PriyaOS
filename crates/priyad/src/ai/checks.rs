//! The Priya AI doctor checks (0.12): a model is chosen, its key is
//! stored, its provider answers, and there is a keyring for keys. None of
//! them spends tokens, and each reads the keyring at most once (2 s at
//! most, see [`KEYRING_WAIT`](super::status::KEYRING_WAIT)). A fresh
//! install, with no model chosen yet, is `info`, not a warning: nothing is
//! broken.

use priya_ai::Health;
use priya_ai::gateway::{Route, RouteError, route};
use priya_ai::keys::KeyError;
use priya_api::error::UserFacing;
use priya_api::methods::ai::KeyState;
use priya_api::methods::system::CheckStatus;

use super::status::{health, key_state, keyring};
use crate::doctor::{BoxFuture, Check, Context, Finding};

/// The model chosen now. Reads settings only, not the keyring.
fn chosen(context: &Context) -> Result<Route, RouteError> {
    route(&context.daemon.settings.handle().current().ai, None)
}

/// Why a check that needs a model has nothing to check.
fn nothing_to_check(e: &RouteError) -> Finding {
    let summary = match e {
        RouteError::NoModel => "No model is chosen yet, so there's nothing to check.",
        _ => "The model setup needs fixing first (see \"A model is chosen to answer\").",
    };
    Finding::new(CheckStatus::Info, summary)
}

/// A keyring problem, with what to do next.
fn keyring_finding(status: CheckStatus, e: &KeyError) -> Finding {
    let summary = match e {
        KeyError::Unavailable(_) => e.user_message(),
        KeyError::Locked => {
            "The keyring is locked. Unlock it (logging in again does), then run `priya doctor` again.".into()
        }
        KeyError::Failed(_) => {
            "The keyring didn't work. Log out and back in to restart it, then run `priya doctor` again.".into()
        }
    };
    Finding::new(status, summary).details(e.to_string())
}

pub struct Configured;

impl Check for Configured {
    fn id(&self) -> &'static str {
        "ai.configured"
    }

    fn title(&self) -> &'static str {
        "A model is chosen to answer"
    }

    fn run<'a>(&'a self, context: &'a Context) -> BoxFuture<'a, Finding> {
        Box::pin(async move {
            match chosen(context) {
                Ok(route) => Finding::new(
                    CheckStatus::Ok,
                    format!("{} answers, from {}.", route.model, route.label),
                ),
                Err(RouteError::NoModel) => {
                    Finding::new(CheckStatus::Info, RouteError::NoModel.user_message())
                }
                Err(e) => Finding::new(CheckStatus::Fail, e.user_message()),
            }
        })
    }
}

pub struct KeyPresent;

impl Check for KeyPresent {
    fn id(&self) -> &'static str {
        "ai.key_present"
    }

    fn title(&self) -> &'static str {
        "The model's key is in the keyring"
    }

    fn run<'a>(&'a self, context: &'a Context) -> BoxFuture<'a, Finding> {
        Box::pin(async move {
            let route = match chosen(context) {
                Ok(route) => route,
                Err(e) => return nothing_to_check(&e),
            };
            match key_state(&context.daemon.ai, &route).await {
                Ok(KeyState::Stored) => Finding::new(CheckStatus::Ok, "It's stored."),
                Ok(KeyState::NotNeeded) => {
                    Finding::new(CheckStatus::Ok, "A model on this computer needs no key.")
                }
                Ok(KeyState::Missing) => Finding::new(
                    CheckStatus::Warn,
                    format!(
                        "There's no key for {}. Store one with: priya ai key set {}",
                        route.provider, route.provider
                    ),
                ),
                Err(e) => keyring_finding(CheckStatus::Warn, &e),
            }
        })
    }
}

pub struct ProviderReachable;

impl Check for ProviderReachable {
    fn id(&self) -> &'static str {
        "ai.provider_reachable"
    }

    fn title(&self) -> &'static str {
        "The model's provider answers"
    }

    fn run<'a>(&'a self, context: &'a Context) -> BoxFuture<'a, Finding> {
        Box::pin(async move {
            let route = match chosen(context) {
                Ok(route) => route,
                Err(e) => return nothing_to_check(&e),
            };
            match health(&context.daemon.ai, &route).await {
                Health::Ok => Finding::new(CheckStatus::Ok, "It answered."),
                Health::Down(e) => {
                    let finding = Finding::new(CheckStatus::Warn, e.user_message.clone());
                    match e.detail {
                        Some(detail) => finding.details(detail),
                        None => finding,
                    }
                }
            }
        })
    }
}

pub struct SecretServiceAvailable;

impl Check for SecretServiceAvailable {
    fn id(&self) -> &'static str {
        "secret_service.available"
    }

    fn title(&self) -> &'static str {
        "There is a keyring for keys"
    }

    fn run<'a>(&'a self, context: &'a Context) -> BoxFuture<'a, Finding> {
        Box::pin(async move {
            // Reading a key nobody stores shows whether the keyring works.
            let probe = keyring(
                context
                    .daemon
                    .ai
                    .providers()
                    .key_stored("priya-doctor-probe"),
            )
            .await;
            let Err(e) = probe else {
                return Finding::new(CheckStatus::Ok, "The keyring answers.");
            };
            // Only a cloud model needs one.
            let cloud = matches!(chosen(context), Ok(route) if !route.local);
            let status = if cloud {
                CheckStatus::Warn
            } else {
                CheckStatus::Info
            };
            keyring_finding(status, &e)
        })
    }
}
