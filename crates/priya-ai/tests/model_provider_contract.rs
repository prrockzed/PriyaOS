//! `model_provider_contract` against [`FakeModelProvider`], so the fake
//! that other tests rely on behaves like a real provider. The
//! OpenAI-compatible adapter runs the same checks in its own file.
//!
//! [`FakeModelProvider`]: priya_testkit::FakeModelProvider

use priya_testkit::model_contract::{self as contract, FakeSetup};

// covers: AI-21
// suite: contract
// acceptance: 0.12
#[tokio::test]
async fn fake_streams_text_deltas_in_order() {
    contract::streams_text_deltas_in_order(FakeSetup::new).await;
}

// covers: AI-21
// suite: contract
// acceptance: 0.12
#[tokio::test]
async fn fake_reports_usage_at_end() {
    contract::reports_usage_at_end(FakeSetup::new).await;
}
