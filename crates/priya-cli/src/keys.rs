//! `priya ai key set|remove`: provider keys in the keyring. These talk to
//! the Secret Service directly, not to priyad, so a key never crosses the
//! daemon's socket and works before priyad has started.

use std::io::{self, BufRead, IsTerminal};

use priya_ai::keys::SecretStore;
use priya_api::error::UserFacing;
use priya_core::secret::Secret;
use priya_core::settings::ProviderName;

use crate::run::{Failure, Out};

/// Where the key comes from: typed at a terminal without echo, or piped
/// in (`pass show gemini | priya ai key set gemini`).
pub enum KeyInput<'a> {
    Terminal,
    Lines(&'a mut dyn BufRead),
}

pub async fn set(
    store: &dyn SecretStore,
    provider: &ProviderName,
    input: KeyInput<'_>,
    out: &mut Out<'_>,
) -> Result<(), Failure> {
    let key = match input {
        KeyInput::Terminal => {
            read_hidden(&format!("Paste the key for {provider} (it won't show): ")).await
        }
        KeyInput::Lines(lines) => first_line(lines).map(Some),
    }
    .map_err(|e| Failure::Refused(format!("The key couldn't be read: {e}")))?
    .ok_or_else(|| Failure::Refused("Stopped; no key was stored.".into()))?;
    // Keys are one token; spaces around a pasted key are never part of it.
    let key = Secret::new(key.expose().trim().to_owned());
    if key.expose().is_empty() {
        return Err(Failure::Refused(
            "No key was given, so nothing changed.".into(),
        ));
    }
    store
        .set(provider.as_str(), key)
        .await
        .map_err(|e| Failure::Refused(e.user_message()))?;
    if out.json {
        out.json(&serde_json::json!({"provider": provider.as_str(), "stored": true}));
    } else {
        out.line(format!("Stored the key for {provider} in the keyring."));
    }
    Ok(())
}

pub async fn remove(
    store: &dyn SecretStore,
    provider: &ProviderName,
    out: &mut Out<'_>,
) -> Result<(), Failure> {
    let removed = store
        .remove(provider.as_str())
        .await
        .map_err(|e| Failure::Refused(e.user_message()))?;
    if out.json {
        out.json(&serde_json::json!({"provider": provider.as_str(), "removed": removed}));
    } else if removed {
        out.line(format!("Removed the key for {provider}."));
    } else {
        out.line(format!("There was no key for {provider}."));
    }
    Ok(())
}

/// Where `set` reads from when run for real.
pub fn stdin_input<'a>(stdin: &'a mut io::StdinLock<'static>) -> KeyInput<'a> {
    if stdin.is_terminal() {
        KeyInput::Terminal
    } else {
        KeyInput::Lines(stdin)
    }
}

fn first_line(lines: &mut dyn BufRead) -> io::Result<Secret<String>> {
    // Room for any key, so growing the buffer leaves no copies behind.
    let mut line = String::with_capacity(1024);
    lines.read_line(&mut line)?;
    Ok(Secret::new(line))
}

/// Reads a line from the terminal with echo off; `None` if Ctrl+C was
/// pressed. Echo comes back however it ends: Ctrl+C is caught here rather
/// than killing the process with echo still off.
async fn read_hidden(prompt: &str) -> io::Result<Option<Secret<String>>> {
    use rustix::termios::{LocalModes, OptionalActions, tcgetattr, tcsetattr};

    struct Restore(rustix::termios::Termios);
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = tcsetattr(io::stdin(), OptionalActions::Now, &self.0);
            eprintln!();
        }
    }

    let interrupted = tokio::signal::ctrl_c();
    eprint!("{prompt}");
    let before = tcgetattr(io::stdin())?;
    let mut hidden = before.clone();
    hidden.local_modes.remove(LocalModes::ECHO);
    tcsetattr(io::stdin(), OptionalActions::Now, &hidden)?;
    let _restore = Restore(before);
    // A plain thread, not the runtime's blocking pool: after Ctrl+C the
    // read is abandoned, and the runtime would wait for it on exit.
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(first_line(&mut io::stdin().lock()));
    });
    tokio::select! {
        line = rx => line.map_err(io::Error::other)?.map(Some),
        _ = interrupted => Ok(None),
    }
}

/// clap's parser for a provider name.
pub fn provider_name(text: &str) -> Result<ProviderName, String> {
    ProviderName::try_from(text.to_owned())
}

#[cfg(test)]
mod tests {
    use priya_ai::keys::MemorySecretStore;

    use super::*;

    fn gemini() -> ProviderName {
        provider_name("gemini").unwrap()
    }

    // covers: AI-21
    // suite: unit
    #[tokio::test]
    async fn key_set_stores_the_trimmed_piped_key_and_remove_deletes_it() {
        let store = MemorySecretStore::default();
        let mut printed = Vec::new();
        let mut out = Out::new(false, &mut printed);
        let mut piped: &[u8] = b"  sk-test-123  \nignored\n";
        set(&store, &gemini(), KeyInput::Lines(&mut piped), &mut out)
            .await
            .unwrap();
        assert_eq!(
            store.get("gemini").await.unwrap().unwrap().expose(),
            "sk-test-123"
        );

        remove(&store, &gemini(), &mut out).await.unwrap();
        remove(&store, &gemini(), &mut out).await.unwrap();
        assert_eq!(
            String::from_utf8(printed).unwrap(),
            "Stored the key for gemini in the keyring.\nRemoved the key for gemini.\nThere was no key for gemini.\n"
        );
    }

    // covers: AI-21
    // suite: unit
    #[tokio::test]
    async fn an_empty_key_or_a_bad_provider_name_is_refused() {
        let store = MemorySecretStore::default();
        let mut printed = Vec::new();
        let mut out = Out::new(false, &mut printed);
        let mut empty: &[u8] = b"   \r\n";
        let refused = set(&store, &gemini(), KeyInput::Lines(&mut empty), &mut out).await;
        assert!(
            matches!(refused, Err(Failure::Refused(m)) if m == "No key was given, so nothing changed.")
        );
        assert!(store.get("gemini").await.unwrap().is_none());
        assert!(
            provider_name("Gemini")
                .unwrap_err()
                .contains("can't be a provider name")
        );
    }

    // covers: AI-21
    // suite: unit
    #[tokio::test]
    async fn json_output_says_what_happened() {
        let store = MemorySecretStore::default();
        let mut printed = Vec::new();
        let mut out = Out::new(true, &mut printed);
        let mut piped: &[u8] = b"sk-test\r\n";
        set(&store, &gemini(), KeyInput::Lines(&mut piped), &mut out)
            .await
            .unwrap();
        assert_eq!(
            store.get("gemini").await.unwrap().unwrap().expose(),
            "sk-test"
        );
        remove(&store, &gemini(), &mut out).await.unwrap();
        assert_eq!(
            String::from_utf8(printed).unwrap(),
            "{\"provider\":\"gemini\",\"stored\":true}\n{\"provider\":\"gemini\",\"removed\":true}\n"
        );
    }
}
