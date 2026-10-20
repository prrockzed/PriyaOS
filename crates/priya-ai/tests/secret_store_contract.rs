//! `SecretStore` contract (principle P2): both stores keep one key per
//! provider, replace it on `set` and report what `remove` did. The
//! Secret Service run needs a real keyring, so it runs only with
//! `PRIYA_LIVE=1` (on the Zenbook), and touches only its own provider.

use priya_ai::keys::{MemorySecretStore, SecretServiceStore, SecretStore};
use priya_core::secret::Secret;

async fn contract(store: &dyn SecretStore, provider: &str) {
    // Clean up after any earlier run that stopped half way.
    store.remove(provider).await.expect("the keyring works");
    assert!(
        store
            .get(provider)
            .await
            .expect("the keyring works")
            .is_none()
    );

    store
        .set(provider, Secret::new("first-test-key".into()))
        .await
        .expect("the keyring works");
    store
        .set(provider, Secret::new("second-test-key".into()))
        .await
        .expect("the keyring works");
    let key = store
        .get(provider)
        .await
        .expect("the keyring works")
        .expect("stored");
    assert_eq!(key.expose(), "second-test-key", "set replaces");
    assert!(
        store
            .get(&format!("{provider}-other"))
            .await
            .expect("the keyring works")
            .is_none()
    );

    assert!(store.remove(provider).await.expect("the keyring works"));
    assert!(
        !store.remove(provider).await.expect("the keyring works"),
        "only once"
    );
    assert!(
        store
            .get(provider)
            .await
            .expect("the keyring works")
            .is_none()
    );
}

// covers: AI-21
// suite: contract
// acceptance: 0.12
#[tokio::test]
async fn memory_store_keeps_replaces_and_removes_keys() {
    contract(&MemorySecretStore::default(), "contract-test").await;
}

// covers: AI-21
// suite: live
#[tokio::test]
async fn secret_service_store_keeps_replaces_and_removes_keys() {
    if std::env::var_os("PRIYA_LIVE").is_none() {
        eprintln!("skipped: set PRIYA_LIVE=1 to use the real keyring");
        return;
    }
    contract(&SecretServiceStore, "priyaos-contract-test").await;
}
