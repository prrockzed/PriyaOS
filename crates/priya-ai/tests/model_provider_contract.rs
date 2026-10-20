//! `model_provider_contract` against [`FakeModelProvider`], so the fake
//! that other tests rely on behaves like a real provider. The
//! OpenAI-compatible adapter runs the same checks in its own file.
//!
//! [`FakeModelProvider`]: priya_testkit::FakeModelProvider

mod fake {
    use priya_testkit::model_contract::{self as contract, FakeSetup};

    // covers: AI-21
    // suite: contract
    // acceptance: 0.12
    #[tokio::test]
    async fn streams_text_deltas_in_order() {
        contract::streams_text_deltas_in_order(FakeSetup::new).await;
    }

    // covers: AI-21
    // suite: contract
    // acceptance: 0.12
    #[tokio::test]
    async fn reports_usage_at_end() {
        contract::reports_usage_at_end(FakeSetup::new).await;
    }

    // covers: AI-21
    // suite: contract
    // acceptance: 0.12
    #[tokio::test]
    async fn maps_401_to_auth_error_with_plain_message() {
        contract::maps_401_to_auth_error_with_plain_message(FakeSetup::new).await;
    }

    // covers: AI-21
    // suite: contract
    // acceptance: 0.12
    // Paused time: these only wait on tokio timers.
    #[tokio::test(start_paused = true)]
    async fn maps_429_with_retry_after() {
        contract::maps_429_with_retry_after(FakeSetup::new).await;
    }

    // covers: AI-21
    // suite: contract
    // acceptance: 0.12
    // Paused time: these only wait on tokio timers.
    #[tokio::test(start_paused = true)]
    async fn retries_overloaded_once_before_first_token() {
        contract::retries_overloaded_once_before_first_token(FakeSetup::new).await;
    }

    // covers: AI-21
    // suite: contract
    // acceptance: 0.12
    #[tokio::test]
    async fn never_retries_after_first_token() {
        contract::never_retries_after_first_token(FakeSetup::new).await;
    }

    // covers: AI-21
    // suite: contract
    // acceptance: 0.12
    // Paused time: these only wait on tokio timers.
    #[tokio::test(start_paused = true)]
    async fn idle_timeout_yields_timeout_error() {
        contract::idle_timeout_yields_timeout_error(FakeSetup::new).await;
    }

    // covers: AI-21
    // suite: contract
    // acceptance: 0.12
    #[tokio::test]
    async fn cancel_drops_connection_within_100ms() {
        contract::cancel_drops_connection_within_100ms(FakeSetup::new).await;
    }
}
