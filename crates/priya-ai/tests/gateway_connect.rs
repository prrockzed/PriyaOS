//! The gateway's connector: which key goes with a question, and when a
//! question isn't sent at all.

use std::sync::Arc;

use priya_ai::gateway::{ConnectError, Connector, Route};
use priya_ai::keys::{KeyError, MemorySecretStore, SecretStore};
use priya_ai::provider::BoxFuture;
use priya_ai::{Message, ModelRequest, StreamEvent};
use priya_api::error::UserFacing;
use priya_core::secret::Secret;
use priya_core::settings::ProviderName;
use priya_testkit::http_script::{HttpReply, ScriptedHttp};
use priya_testkit::model_contract::TIMEOUTS;
use tokio_stream::StreamExt;

const ANSWER: [&str; 2] = [
    r#"{"choices":[{"delta":{"content":"Yes."},"finish_reason":"stop"}]}"#,
    "[DONE]",
];

fn route(server: &ScriptedHttp, provider: &str, local: bool) -> Route {
    Route {
        provider: ProviderName::try_from(provider.to_owned()).expect("a valid name"),
        model: "m-1".into(),
        // The scripted server is on this computer; `local` says how to
        // treat it.
        base_url: server.base_url(),
        local,
        label: priya_ai::gateway::route::label(provider),
    }
}

async fn ask(connector: &Connector, route: &Route) -> Result<String, ConnectError> {
    let provider = connector.provider(route).await?;
    let request = ModelRequest::new(route.model.clone(), vec![Message::user("hi")]);
    let events: Vec<_> = provider
        .complete(request)
        .await
        .expect("it answers")
        .collect()
        .await;
    Ok(events
        .into_iter()
        .filter_map(|e| match e {
            Ok(StreamEvent::TextDelta(text)) => Some(text),
            _ => None,
        })
        .collect())
}

/// A keyring that can't be reached.
struct NoKeyring;

impl SecretStore for NoKeyring {
    fn get(&self, _: &str) -> BoxFuture<'_, Result<Option<Secret<String>>, KeyError>> {
        Box::pin(async { Err(KeyError::Unavailable("no bus".into())) })
    }
    fn set(&self, _: &str, _: Secret<String>) -> BoxFuture<'_, Result<(), KeyError>> {
        Box::pin(async { Err(KeyError::Unavailable("no bus".into())) })
    }
    fn remove(&self, _: &str) -> BoxFuture<'_, Result<bool, KeyError>> {
        Box::pin(async { Err(KeyError::Unavailable("no bus".into())) })
    }
}

// covers: AI-21
// suite: integration
#[tokio::test]
async fn a_cloud_provider_without_a_key_is_never_asked() {
    let server = ScriptedHttp::start(vec![HttpReply::sse(&ANSWER)])
        .await
        .unwrap();
    let connector = Connector::new(Arc::new(MemorySecretStore::default()), TIMEOUTS).unwrap();
    let error = ask(&connector, &route(&server, "gemini", false))
        .await
        .unwrap_err();
    assert_eq!(
        error.user_message(),
        "Gemini needs a key. Store one with `priya ai key set gemini`."
    );
    assert!(server.requests().is_empty(), "nothing was sent");

    let broken = Connector::new(Arc::new(NoKeyring), TIMEOUTS).unwrap();
    let error = ask(&broken, &route(&server, "gemini", false))
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        ConnectError::Keyring(KeyError::Unavailable(_))
    ));
}

// covers: AI-21
// suite: integration
#[tokio::test]
async fn the_stored_key_goes_with_the_question() {
    let server = ScriptedHttp::start(vec![HttpReply::sse(&ANSWER), HttpReply::sse(&ANSWER)])
        .await
        .unwrap();
    let keys = Arc::new(MemorySecretStore::default());
    keys.set("gemini", Secret::new("test-key".into()))
        .await
        .unwrap();
    keys.set("local", Secret::new("proxy-key".into()))
        .await
        .unwrap();
    let connector = Connector::new(keys, TIMEOUTS).unwrap();
    assert_eq!(
        ask(&connector, &route(&server, "gemini", false))
            .await
            .unwrap(),
        "Yes."
    );
    // A local server gets a key too when one is stored (a proxy may want it).
    ask(&connector, &route(&server, "local", true))
        .await
        .unwrap();
    let sent: Vec<_> = server
        .requests()
        .iter()
        .map(|r| r.header("authorization").map(str::to_owned))
        .collect();
    assert_eq!(
        sent,
        [
            Some("Bearer test-key".to_owned()),
            Some("Bearer proxy-key".to_owned())
        ]
    );
}

// covers: AI-21
// suite: integration
#[tokio::test]
async fn a_local_server_answers_without_a_keyring() {
    let server = ScriptedHttp::start(vec![HttpReply::sse(&ANSWER)])
        .await
        .unwrap();
    let connector = Connector::new(Arc::new(NoKeyring), TIMEOUTS).unwrap();
    assert_eq!(
        ask(&connector, &route(&server, "local", true))
            .await
            .unwrap(),
        "Yes."
    );
    assert_eq!(server.requests()[0].header("authorization"), None);
}

// covers: AI-21
// suite: unit
#[tokio::test]
async fn key_stored_tells_a_stored_key_a_missing_one_and_no_keyring_apart() {
    let keys = Arc::new(MemorySecretStore::default());
    keys.set("gemini", Secret::new("k".into())).await.unwrap();
    let connector = Connector::new(keys, TIMEOUTS).unwrap();
    assert!(connector.key_stored("gemini").await.unwrap());
    assert!(!connector.key_stored("xai").await.unwrap());
    let broken = Connector::new(Arc::new(NoKeyring), TIMEOUTS).unwrap();
    assert!(matches!(
        broken.key_stored("gemini").await,
        Err(KeyError::Unavailable(_))
    ));
}
