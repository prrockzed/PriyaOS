//! Asking questions of the real priyad (0.12): `v1.intent.*` over its
//! socket and `priya ask`, with a scripted local server standing in for
//! the model (an OpenAI-compatible endpoint, as Ollama is).

use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use priya_api::client::Client;
use priya_api::methods::events::{EVENT_NOTIFICATION, EventNotification, SubscribeParams};
use priya_api::methods::intent::{
    self, CancelParams, IntentContext, STREAM_NOTIFICATION, StepKind, StreamStep, SubmitParams,
};
use priya_api::methods::{NOTIFICATIONS, events, settings};
use priya_testkit::http_script::{BodyStep, HttpReply, ScriptedHttp};
use priya_testkit::{TestDaemon, workspace_binary};
use serde_json::{Value, json};

const PRIYAD: &str = env!("CARGO_BIN_EXE_priyad");
/// Long enough for a loaded CI machine, short enough to catch a hang.
const PATIENCE: Duration = Duration::from_secs(20);

fn chunk(text: &str) -> String {
    json!({"model": "scripted-1", "choices": [{"delta": {"content": text}, "finish_reason": null}]})
        .to_string()
}

fn event(data: &str) -> BodyStep {
    BodyStep::Send(format!("data: {data}\n\n").into_bytes())
}

/// A whole answer in these pieces, with usage.
fn answer(pieces: &[&str]) -> HttpReply {
    let mut events: Vec<String> = pieces.iter().map(|p| chunk(p)).collect();
    events.push(json!({"choices": [{"delta": {}, "finish_reason": "stop"}]}).to_string());
    events.push(
        json!({"choices": [], "usage": {"prompt_tokens": 12, "completion_tokens": 3}}).to_string(),
    );
    events.push("[DONE]".into());
    HttpReply::sse(&events.iter().map(String::as_str).collect::<Vec<_>>())
}

/// One piece of text, then nothing more, ever.
fn hanging() -> HttpReply {
    HttpReply::sse_steps(vec![event(&chunk("Half")), BodyStep::Hang])
}

/// priyad with `local` pointing at `server` and chosen to answer.
async fn daemon_answering_from(server: &ScriptedHttp) -> (TestDaemon, Client) {
    let daemon = TestDaemon::start(PRIYAD).expect("priyad starts");
    let mut client = daemon.client().await.expect("a client");
    for (key, value) in [
        (
            "ai.providers.local",
            json!({"kind": "openai_compatible", "base_url": server.base_url()}),
        ),
        ("ai.strategy", json!("fixed:local/scripted-1")),
    ] {
        client
            .call::<settings::Set>(&settings::SetParams {
                key: key.into(),
                value,
            })
            .await
            .expect("the setting is taken");
    }
    (daemon, client)
}

async fn ask(client: &mut Client, text: &str) -> String {
    let params = SubmitParams {
        text: text.into(),
        context: Some(IntentContext {
            surface: Some("test".into()),
            ..IntentContext::default()
        }),
        overrides: None,
    };
    client
        .call::<intent::Submit>(&params)
        .await
        .expect("asked")
        .intent_id
}

/// The next stream step of any intent.
async fn next_step(client: &mut Client) -> StreamStep {
    loop {
        let notification = tokio::time::timeout(PATIENCE, client.next_notification())
            .await
            .expect("a step in time")
            .expect("the connection works")
            .expect("priyad is still there");
        if notification.method == STREAM_NOTIFICATION {
            let params = notification.params.expect("a step has params");
            return serde_json::from_value(params).expect("a stream step");
        }
    }
}

/// Steps of one intent until its last.
async fn steps_until_end(client: &mut Client, intent_id: &str) -> Vec<StreamStep> {
    let mut steps = Vec::new();
    loop {
        let step = next_step(client).await;
        if step.intent_id != intent_id {
            continue;
        }
        let last = matches!(
            step.kind,
            StepKind::Done | StepKind::Error | StepKind::Cancelled
        );
        steps.push(step);
        if last {
            return steps;
        }
    }
}

/// The events in a stopped priyad's store, as {type, actor, payload}.
async fn stored(daemon: &TestDaemon) -> Vec<Value> {
    let store = priya_store::EventStore::open(
        &priya_store::StoreConfig::in_data_dir(&daemon.home().paths().data),
        Arc::new(priya_core::clock::SystemClock),
    )
    .expect("the store opens");
    store
        .read_after(0, 1000)
        .await
        .expect("events")
        .into_iter()
        .map(|r| json!({"type": r.event.event_type, "actor": r.event.actor, "payload": r.event.payload}))
        .collect()
}

/// The types of the events recorded so far.
async fn recorded(daemon: &TestDaemon, until: &str) -> Vec<Value> {
    let mut client = daemon.client().await.expect("a client");
    let params = SubscribeParams {
        types: vec!["intent.*".into(), "model.*".into()],
        stream_prefix: None,
        from_seq: Some(0),
    };
    client
        .call::<events::Subscribe>(&params)
        .await
        .expect("subscribed");
    let mut seen = Vec::new();
    loop {
        let notification = tokio::time::timeout(PATIENCE, client.next_notification())
            .await
            .expect("events in time")
            .expect("the connection works")
            .expect("priyad is still there");
        if notification.method != EVENT_NOTIFICATION {
            continue;
        }
        let params = notification.params.expect("an event has params");
        let event: EventNotification = serde_json::from_value(params).expect("an event");
        let event = event.event.event;
        let done = event.event_type == until;
        seen.push(json!({"type": event.event_type, "payload": event.payload}));
        if done {
            return seen;
        }
    }
}

// covers: AI-21, CORE-04
// suite: integration
// acceptance: 0.12
#[tokio::test]
async fn ask_streams_and_records_submitted_called_answered() {
    let server = ScriptedHttp::start(vec![answer(&["Btrfs is", " a filesystem."])])
        .await
        .unwrap();
    let (daemon, mut client) = daemon_answering_from(&server).await;
    let id = ask(&mut client, "what is btrfs?").await;
    let steps = steps_until_end(&mut client, &id).await;

    let kinds: Vec<StepKind> = steps.iter().map(|s| s.kind).collect();
    assert_eq!(
        kinds,
        [
            StepKind::Started,
            StepKind::Delta,
            StepKind::Delta,
            StepKind::Usage,
            StepKind::Done
        ]
    );
    let seqs: Vec<u64> = steps.iter().map(|s| s.seq).collect();
    assert_eq!(seqs, [1, 2, 3, 4, 5]);
    assert_eq!(steps[0].model.as_deref(), Some("scripted-1"));
    assert_eq!(steps[0].local, Some(true));
    assert_eq!(steps[4].text.as_deref(), Some("Btrfs is a filesystem."));

    let events = recorded(&daemon, "intent.answered").await;
    let types: Vec<&str> = events.iter().map(|e| e["type"].as_str().unwrap()).collect();
    assert_eq!(
        types,
        ["intent.submitted", "model.called", "intent.answered"]
    );
    assert_eq!(events[1]["payload"]["outcome"], "ok");
    assert_eq!(events[1]["payload"]["tokens_out"], 3);
    assert_eq!(events[2]["payload"]["answer"], "Btrfs is a filesystem.");
}

// covers: AI-21
// suite: integration
// acceptance: 0.12
#[tokio::test]
async fn cancel_mid_stream_records_cancelled() {
    let server = ScriptedHttp::start(vec![hanging()]).await.unwrap();
    let (daemon, mut client) = daemon_answering_from(&server).await;
    let id = ask(&mut client, "tell me a long story").await;
    while next_step(&mut client).await.kind != StepKind::Delta {}
    let cancel = CancelParams {
        intent_id: id.clone(),
    };
    client
        .call::<intent::Cancel>(&cancel)
        .await
        .expect("cancelled");
    let steps = steps_until_end(&mut client, &id).await;
    assert_eq!(steps.last().unwrap().kind, StepKind::Cancelled);

    let events = recorded(&daemon, "intent.cancelled").await;
    assert_eq!(events[1]["payload"]["outcome"], "cancelled");
    assert_eq!(events[2]["payload"]["reason"], "stopped by the asker");
    // The model's connection closed when the answer stopped.
    let deadline = Instant::now() + PATIENCE;
    while server.open() > 0 {
        assert!(
            Instant::now() < deadline,
            "the model's connection stayed open"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // Cancelling again: it's over.
    let again = client.call::<intent::Cancel>(&cancel).await;
    assert!(again.is_err());
}

// covers: CORE-04
// suite: integration
// acceptance: 0.12
#[tokio::test]
async fn two_concurrent_asks_on_one_connection_are_independent() {
    let server = ScriptedHttp::start(vec![hanging(), answer(&["Second."])])
        .await
        .unwrap();
    let (_daemon, mut client) = daemon_answering_from(&server).await;
    let first = ask(&mut client, "one").await;
    // Wait until the first holds its connection, so the replies pair up.
    while server.open() == 0 {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let second = ask(&mut client, "two").await;
    let steps = steps_until_end(&mut client, &second).await;
    assert_eq!(steps.last().unwrap().text.as_deref(), Some("Second."));
    // The first is still going, and stops on its own.
    client
        .call::<intent::Cancel>(&CancelParams {
            intent_id: first.clone(),
        })
        .await
        .expect("the first is still running");
    let first_steps = steps_until_end(&mut client, &first).await;
    assert_eq!(first_steps.last().unwrap().kind, StepKind::Cancelled);
    assert!(first_steps.iter().all(|s| s.intent_id == first));
}

// covers: CORE-04
// suite: integration
// acceptance: 0.12
#[tokio::test]
async fn daemon_restart_mid_stream_gives_client_error_not_hang() {
    let server = ScriptedHttp::start(vec![hanging()]).await.unwrap();
    let (mut daemon, _client) = daemon_answering_from(&server).await;
    let mut asking = daemon
        .command(workspace_binary("priya"))
        .args(["ask", "tell me a long story"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("priya runs");
    // Wait until the model is answering, then stop priyad under it.
    while server.open() == 0 {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    daemon.stop().expect("priyad stops");
    let begun = Instant::now();
    let output = tokio::task::spawn_blocking(move || {
        let deadline = Instant::now() + PATIENCE;
        while asking.try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline, "priya ask hung");
            std::thread::sleep(Duration::from_millis(20));
        }
        asking.wait_with_output().unwrap()
    })
    .await
    .unwrap();
    assert!(begun.elapsed() < PATIENCE);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Lost the connection"), "{stderr}");
    // Recorded as stopped by priyad, not by a closed connection.
    let events = stored(&daemon).await;
    let cancelled = events
        .iter()
        .find(|e| e["type"] == "intent.cancelled")
        .expect("the answer's end is recorded");
    assert_eq!(cancelled["payload"]["reason"], "priyad is stopping");
}

// covers: CORE-04, AI-21
// suite: integration
#[tokio::test]
async fn cli_ctrl_c_stops_the_answer_and_exits_130() {
    let server = ScriptedHttp::start(vec![hanging()]).await.unwrap();
    let (daemon, _client) = daemon_answering_from(&server).await;
    let asking = daemon
        .command(workspace_binary("priya"))
        .args(["ask", "tell me a long story"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("priya runs");
    while server.open() == 0 {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let pid = rustix::process::Pid::from_child(&asking);
    rustix::process::kill_process(pid, rustix::process::Signal::INT).expect("Ctrl+C sent");
    let output = tokio::task::spawn_blocking(move || asking.wait_with_output())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(130),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let events = recorded(&daemon, "intent.cancelled").await;
    let cancelled = events.last().unwrap();
    assert_eq!(cancelled["payload"]["reason"], "stopped by the asker");
    let deadline = Instant::now() + PATIENCE;
    while server.open() > 0 {
        assert!(
            Instant::now() < deadline,
            "the model's connection stayed open"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

// covers: AI-21
// suite: integration
#[tokio::test]
async fn one_connection_may_have_eight_answers_going() {
    let server = ScriptedHttp::start((0..8).map(|_| hanging()).collect())
        .await
        .unwrap();
    let (_daemon, mut client) = daemon_answering_from(&server).await;
    for i in 0..8 {
        ask(&mut client, &format!("question {i}")).await;
    }
    let params = SubmitParams {
        text: "one more".into(),
        context: None,
        overrides: None,
    };
    let refused = client
        .call::<intent::Submit>(&params)
        .await
        .expect_err("a ninth is refused");
    assert!(
        refused.to_string().contains("already has 8 answers"),
        "{refused}"
    );
}

// covers: CORE-04
// suite: integration
// acceptance: 0.12
#[tokio::test]
async fn cli_ask_json_matches_schema() {
    let server = ScriptedHttp::start(vec![answer(&["Hi", " there."])])
        .await
        .unwrap();
    let (daemon, _client) = daemon_answering_from(&server).await;
    // On a blocking thread: the scripted server runs on this test's
    // runtime and must keep answering meanwhile.
    let mut command = daemon.command(workspace_binary("priya"));
    command.args(["--json", "ask", "say", "hi"]);
    let output = tokio::task::spawn_blocking(move || command.output())
        .await
        .expect("the thread ends")
        .expect("priya runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let schema = NOTIFICATIONS
        .iter()
        .find(|n| n.name == STREAM_NOTIFICATION)
        .map(|n| (n.params)().to_value())
        .expect("the stream step has a schema");
    let validator = jsonschema::validator_for(&schema).expect("a valid schema");
    let lines: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).expect("one JSON value per line"))
        .collect();
    assert!(lines.len() >= 3, "{lines:?}");
    for line in &lines {
        assert!(validator.is_valid(line), "{line} doesn't match the schema");
    }
    assert_eq!(lines.last().unwrap()["kind"], "done");
    assert_eq!(lines.last().unwrap()["text"], "Hi there.");
    // The question asked of the model came with the shipped prompt.
    let body: Value = serde_json::from_slice(&server.requests()[0].body).unwrap();
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][1]["content"], "say hi");
}

// covers: AI-21, CORE-04
// suite: integration
#[tokio::test]
async fn closing_the_connection_mid_answer_stops_it() {
    let server = ScriptedHttp::start(vec![hanging()]).await.unwrap();
    let (daemon, mut client) = daemon_answering_from(&server).await;
    ask(&mut client, "tell me a long story").await;
    while next_step(&mut client).await.kind != StepKind::Delta {}
    drop(client);
    let events = recorded(&daemon, "intent.cancelled").await;
    let cancelled = events.last().unwrap();
    assert_eq!(
        cancelled["payload"]["reason"],
        "the asking connection closed"
    );
    let deadline = Instant::now() + PATIENCE;
    while server.open() > 0 {
        assert!(
            Instant::now() < deadline,
            "the model's connection stayed open"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

// covers: CORE-04
// suite: integration
#[tokio::test]
async fn questions_and_names_are_checked_before_anything_runs() {
    let server = ScriptedHttp::start(Vec::new()).await.unwrap();
    let (_daemon, mut client) = daemon_answering_from(&server).await;
    let context = |conversation: Option<&str>, surface: Option<&str>| IntentContext {
        conversation_id: conversation.map(str::to_owned),
        surface: surface.map(str::to_owned),
        cwd: None,
    };
    let too_long = "x".repeat(intent::MAX_TEXT_CHARS + 1);
    for (text, context, expected) in [
        ("   ", context(None, None), "Ask something first."),
        (too_long.as_str(), context(None, None), "too long"),
        ("hi", context(Some("not ok!"), None), "conversation id"),
        ("hi", context(None, Some("a b")), "surface name"),
    ] {
        let params = SubmitParams {
            text: text.into(),
            context: Some(context),
            overrides: None,
        };
        let error = client.call::<intent::Submit>(&params).await.unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }
    assert!(server.requests().is_empty(), "no model was asked");
}

// covers: CORE-04
// suite: integration
#[tokio::test]
async fn cli_ask_c_continues_the_last_conversation() {
    let server = ScriptedHttp::start(vec![answer(&["Btrfs."]), answer(&["Chris Mason."])])
        .await
        .unwrap();
    let (daemon, _client) = daemon_answering_from(&server).await;
    for args in [
        vec!["ask", "what is btrfs?"],
        vec!["ask", "-c", "who made it?"],
    ] {
        let mut command = daemon.command(workspace_binary("priya"));
        command.args(&args);
        let output = tokio::task::spawn_blocking(move || command.output())
            .await
            .expect("the thread ends")
            .expect("priya runs");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let body: Value = serde_json::from_slice(&server.requests()[1].body).unwrap();
    let said: Vec<&str> = body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .skip(1) // the system prompt
        .map(|m| m["content"].as_str().unwrap())
        .collect();
    assert_eq!(said, ["what is btrfs?", "Btrfs.", "who made it?"]);
}
