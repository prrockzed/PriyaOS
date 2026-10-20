//! Where provider API keys live: the `SecretStore` port (05-security §3).
//!
//! Keys are kept only in the Secret Service (gnome-keyring, KeePassXC), as
//! items with the attributes `{app: "priya", provider: <name>}`, and read
//! on demand. They never go into settings, events or logs; in memory they
//! are a `Secret`, wiped when dropped.
//!
//! Items go in the default collection rather than one of our own: a new
//! collection has its own password, which gnome-keyring asks for after
//! every login, while the default one is unlocked by logging in.

use std::collections::HashMap;
use std::sync::Mutex;

use priya_api::error::{ErrorKind, UserFacing};
use priya_core::secret::Secret;

use crate::provider::BoxFuture;

/// Why the keyring couldn't be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
    /// No Secret Service is running (or this isn't Linux).
    #[error("no Secret Service: {0}")]
    Unavailable(String),
    /// The keyring is locked and the unlock prompt was dismissed.
    #[error("the keyring stayed locked")]
    Locked,
    #[error("the keyring failed: {0}")]
    Failed(String),
}

impl UserFacing for KeyError {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::Unavailable(_) | Self::Failed(_) => ErrorKind::Unavailable,
            Self::Locked => ErrorKind::PermissionDenied,
        }
    }

    fn user_message(&self) -> String {
        match self {
            Self::Unavailable(_) => "There's no keyring to keep keys in. Install gnome-keyring, or KeePassXC with its Secret Service turned on, then try again.".into(),
            Self::Locked => "The keyring stayed locked, so the key couldn't be used.".into(),
            Self::Failed(_) => "The keyring didn't work. `priya doctor` can check it.".into(),
        }
    }
}

/// Keys by provider name (the name in `ai.providers`).
pub trait SecretStore: Send + Sync {
    fn get(&self, provider: &str) -> BoxFuture<'_, Result<Option<Secret<String>>, KeyError>>;
    /// Stores the key, replacing any earlier one.
    fn set(&self, provider: &str, key: Secret<String>) -> BoxFuture<'_, Result<(), KeyError>>;
    /// Removes the key; false if there was none.
    fn remove(&self, provider: &str) -> BoxFuture<'_, Result<bool, KeyError>>;
}

/// Keys in memory, for tests.
#[derive(Default)]
pub struct MemorySecretStore {
    keys: Mutex<HashMap<String, Secret<String>>>,
}

impl MemorySecretStore {
    fn keys(&self) -> std::sync::MutexGuard<'_, HashMap<String, Secret<String>>> {
        self.keys.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl SecretStore for MemorySecretStore {
    fn get(&self, provider: &str) -> BoxFuture<'_, Result<Option<Secret<String>>, KeyError>> {
        let key = self
            .keys()
            .get(provider)
            .map(|key| Secret::new(key.expose().clone()));
        Box::pin(async move { Ok(key) })
    }

    fn set(&self, provider: &str, key: Secret<String>) -> BoxFuture<'_, Result<(), KeyError>> {
        self.keys().insert(provider.to_owned(), key);
        Box::pin(async { Ok(()) })
    }

    fn remove(&self, provider: &str) -> BoxFuture<'_, Result<bool, KeyError>> {
        let removed = self.keys().remove(provider).is_some();
        Box::pin(async move { Ok(removed) })
    }
}

/// Keys in the Secret Service, over D-Bus. Each call connects anew, so a
/// keyring started after priyad still works.
#[derive(Debug, Default, Clone, Copy)]
pub struct SecretServiceStore;

#[cfg(target_os = "linux")]
mod secret_service {
    use std::time::Duration;

    use oo7::dbus::{Collection, Error, Service, ServiceError};

    use super::{KeyError, Secret};

    /// Unlocking may show a password prompt, which the owner may leave
    /// open; nothing waits on the keyring longer than this.
    const PATIENCE: Duration = Duration::from_secs(60);

    fn attributes(provider: &str) -> [(&'static str, &str); 2] {
        [("app", "priya"), ("provider", provider)]
    }

    /// No session bus, or no Secret Service on it.
    fn missing(error: &zbus::Error) -> bool {
        const MISSING: [&str; 2] = [
            "org.freedesktop.DBus.Error.ServiceUnknown",
            "org.freedesktop.DBus.Error.NameHasNoOwner",
        ];
        match error {
            zbus::Error::Address(_) => true,
            zbus::Error::MethodError(name, _, _) => MISSING.contains(&name.as_str()),
            zbus::Error::FDO(fdo) => matches!(
                **fdo,
                zbus::fdo::Error::ServiceUnknown(_) | zbus::fdo::Error::NameHasNoOwner(_)
            ),
            zbus::Error::InputOutput(io) => matches!(
                io.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
            ),
            _ => false,
        }
    }

    pub(super) fn failed(error: Error) -> KeyError {
        match &error {
            Error::Dismissed | Error::Service(ServiceError::IsLocked(_)) => KeyError::Locked,
            Error::ZBus(e) | Error::Service(ServiceError::ZBus(e)) if missing(e) => {
                KeyError::Unavailable(error.to_string())
            }
            _ => KeyError::Failed(error.to_string()),
        }
    }

    async fn patiently<T>(work: impl Future<Output = Result<T, KeyError>>) -> Result<T, KeyError> {
        tokio::time::timeout(PATIENCE, work)
            .await
            .unwrap_or_else(|_| Err(KeyError::Failed("the keyring didn't answer in time".into())))
    }

    /// The default collection, unlocked; with `create`, made if there is
    /// none (which may prompt for a new keyring password).
    async fn collection(create: bool) -> Result<Option<Collection>, KeyError> {
        let service = Service::new().await.map_err(failed)?;
        let collection = if create {
            Some(service.default_collection().await.map_err(failed)?)
        } else {
            service.with_alias("default").await.map_err(failed)?
        };
        if let Some(collection) = &collection
            && collection.is_locked().await.map_err(failed)?
        {
            collection.unlock(None).await.map_err(failed)?;
        }
        Ok(collection)
    }

    pub async fn get(provider: &str) -> Result<Option<Secret<String>>, KeyError> {
        patiently(async {
            let Some(collection) = collection(false).await? else {
                return Ok(None);
            };
            let items = collection
                .search_items(&attributes(provider))
                .await
                .map_err(failed)?;
            let Some(item) = items.first() else {
                return Ok(None);
            };
            let secret = item.secret().await.map_err(failed)?;
            // Straight from the wiped buffer into a Secret, with no copy
            // left behind on failure.
            let text = std::str::from_utf8(&secret)
                .map_err(|_| KeyError::Failed("the stored key isn't text".into()))?;
            Ok(Some(Secret::new(text.to_owned())))
        })
        .await
    }

    pub async fn set(provider: &str, key: Secret<String>) -> Result<(), KeyError> {
        patiently(async {
            let label = format!("PriyaOS key for {provider}");
            let Some(collection) = collection(true).await? else {
                return Err(KeyError::Failed("no default collection".into()));
            };
            collection
                .create_item(
                    &label,
                    &attributes(provider),
                    key.expose().as_str(),
                    true,
                    None,
                )
                .await
                .map_err(failed)?;
            Ok(())
        })
        .await
    }

    pub async fn remove(provider: &str) -> Result<bool, KeyError> {
        patiently(async {
            let Some(collection) = collection(false).await? else {
                return Ok(false);
            };
            let items = collection
                .search_items(&attributes(provider))
                .await
                .map_err(failed)?;
            for item in &items {
                item.delete(None).await.map_err(failed)?;
            }
            Ok(!items.is_empty())
        })
        .await
    }

    #[cfg(test)]
    mod tests {
        use std::sync::Arc;

        use super::*;

        // covers: AI-21
        // suite: unit
        #[test]
        fn keyring_errors_are_told_apart() {
            let unknown = zbus::Error::FDO(Box::new(zbus::fdo::Error::ServiceUnknown(
                "org.freedesktop.secrets".into(),
            )));
            assert!(matches!(
                failed(Error::ZBus(unknown)),
                KeyError::Unavailable(_)
            ));
            let no_bus = zbus::Error::Address("no session bus".into());
            assert!(matches!(
                failed(Error::Service(ServiceError::ZBus(no_bus))),
                KeyError::Unavailable(_)
            ));
            let refused = std::io::Error::from(std::io::ErrorKind::ConnectionRefused);
            let refused = zbus::Error::InputOutput(Arc::new(refused));
            assert!(matches!(
                failed(Error::ZBus(refused)),
                KeyError::Unavailable(_)
            ));
            let slow = std::io::Error::from(std::io::ErrorKind::TimedOut);
            let slow = zbus::Error::InputOutput(Arc::new(slow));
            assert!(matches!(failed(Error::ZBus(slow)), KeyError::Failed(_)));
            assert_eq!(failed(Error::Dismissed), KeyError::Locked);
            assert_eq!(
                failed(Error::Service(ServiceError::IsLocked("x".into()))),
                KeyError::Locked
            );
            assert!(matches!(failed(Error::Deleted), KeyError::Failed(_)));
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod secret_service {
    use super::{KeyError, Secret};

    fn unavailable() -> KeyError {
        KeyError::Unavailable("the Secret Service needs Linux".into())
    }

    pub async fn get(_: &str) -> Result<Option<Secret<String>>, KeyError> {
        Err(unavailable())
    }

    pub async fn set(_: &str, _: Secret<String>) -> Result<(), KeyError> {
        Err(unavailable())
    }

    pub async fn remove(_: &str) -> Result<bool, KeyError> {
        Err(unavailable())
    }
}

impl SecretStore for SecretServiceStore {
    fn get(&self, provider: &str) -> BoxFuture<'_, Result<Option<Secret<String>>, KeyError>> {
        let provider = provider.to_owned();
        Box::pin(async move { secret_service::get(&provider).await })
    }

    fn set(&self, provider: &str, key: Secret<String>) -> BoxFuture<'_, Result<(), KeyError>> {
        let provider = provider.to_owned();
        Box::pin(async move { secret_service::set(&provider, key).await })
    }

    fn remove(&self, provider: &str) -> BoxFuture<'_, Result<bool, KeyError>> {
        let provider = provider.to_owned();
        Box::pin(async move { secret_service::remove(&provider).await })
    }
}

#[cfg(test)]
mod tests {
    use priya_api::error::ApiError;

    use super::*;

    // covers: AI-21
    // suite: unit
    #[tokio::test]
    async fn the_memory_store_keeps_one_key_per_provider() {
        let store = MemorySecretStore::default();
        assert!(store.get("gemini").await.unwrap().is_none());
        store
            .set("gemini", Secret::new("one".into()))
            .await
            .unwrap();
        store
            .set("gemini", Secret::new("two".into()))
            .await
            .unwrap();
        assert_eq!(store.get("gemini").await.unwrap().unwrap().expose(), "two");
        assert!(store.get("xai").await.unwrap().is_none());
        assert!(store.remove("gemini").await.unwrap());
        assert!(!store.remove("gemini").await.unwrap());
    }

    // covers: AI-21
    // suite: unit
    #[test]
    fn keyring_errors_say_what_to_do() {
        let missing = ApiError::from_user_facing(&KeyError::Unavailable("no bus".into()));
        assert_eq!(missing.kind, ErrorKind::Unavailable);
        assert!(missing.user_message.contains("gnome-keyring"));
        assert_eq!(missing.detail.as_deref(), Some("no Secret Service: no bus"));
        let locked = ApiError::from_user_facing(&KeyError::Locked);
        assert_eq!(locked.kind, ErrorKind::PermissionDenied);
        assert!(
            KeyError::Failed("x".into())
                .user_message()
                .contains("priya doctor")
        );
    }

    // covers: AI-21
    // suite: unit
    #[cfg(not(target_os = "linux"))]
    #[tokio::test]
    async fn the_secret_service_needs_linux() {
        let store = SecretServiceStore;
        assert!(matches!(
            store.get("x").await,
            Err(KeyError::Unavailable(_))
        ));
        assert!(matches!(
            store.set("x", Secret::new("k".into())).await,
            Err(KeyError::Unavailable(_))
        ));
        assert!(matches!(
            store.remove("x").await,
            Err(KeyError::Unavailable(_))
        ));
    }
}
