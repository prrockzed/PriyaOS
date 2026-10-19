//! A wrapper that keeps secrets (API keys, tokens) out of logs, debug output
//! and serialized data, and wipes them from memory when dropped.

use std::fmt;

use zeroize::Zeroize;

/// A secret value. It prints as `[redacted]`, can't be serialized, and is
/// zeroed on drop. Read it with [`Secret::expose`], so every read is easy to
/// find in review.
pub struct Secret<T: Zeroize>(T);

impl<T: Zeroize> Secret<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }

    pub fn expose(&self) -> &T {
        &self.0
    }
}

impl<T: Zeroize> Drop for Secret<T> {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl<T: Zeroize> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

impl<T: Zeroize> fmt::Display for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

impl From<String> for Secret<String> {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: infra
    // suite: unit
    // acceptance: 0.2
    // regression: 0.2
    #[test]
    fn secret_debug_and_display_are_redacted() {
        let key = Secret::from(String::from("sk-ant-very-secret"));
        assert_eq!(format!("{key:?}"), "[redacted]");
        assert_eq!(format!("{key}"), "[redacted]");
        assert_eq!(key.expose(), "sk-ant-very-secret");

        // Also when nested in something that derives Debug.
        #[derive(Debug)]
        #[allow(dead_code)] // only formatted
        struct Provider {
            name: &'static str,
            key: Secret<String>,
        }
        let provider = Provider {
            name: "anthropic",
            key,
        };
        let printed = format!("{provider:?}");
        assert!(printed.contains("anthropic"));
        assert!(!printed.contains("very-secret"), "{printed}");
    }
}
