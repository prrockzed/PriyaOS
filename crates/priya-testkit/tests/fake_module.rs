//! The fake module binary on its own, over stdio: it answers the handshake
//! and calls, and misbehaves as scripted (0.8). Also makes cargo build the
//! binary, which only a package's own integration tests do, for the priyad
//! tests that install it.

use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};

use priya_api::framing::lsp::{Decoder, encode};
use priya_testkit::FakeModuleProcess;
use serde_json::{Value, json};

struct Running {
    child: Child,
    decoder: Decoder,
    queue: Vec<Vec<u8>>,
}

impl Running {
    fn start(fake: &FakeModuleProcess, name: &str) -> (Self, tempfile::TempDir) {
        let modules = tempfile::tempdir().expect("a folder");
        let dir = fake.install(modules.path());
        let manifest = std::fs::read_to_string(dir.join("module.toml")).expect("the manifest");
        let exec = manifest
            .lines()
            .find_map(|line| line.strip_prefix("exec = \""))
            .and_then(|rest| rest.strip_suffix('"'))
            .expect("an exec line")
            .to_owned();
        let child = Command::new(exec)
            .current_dir(&dir)
            .env("PRIYA_MODULE_NAME", name)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the fake module starts");
        let running = Self {
            child,
            decoder: Decoder::default(),
            queue: Vec::new(),
        };
        (running, modules)
    }

    fn send(&mut self, message: &Value) {
        let stdin = self.child.stdin.as_mut().expect("stdin");
        stdin
            .write_all(&encode(message).expect("encoded"))
            .expect("written");
        stdin.flush().expect("flushed");
    }

    /// The next message, or `None` when the module's output ends.
    fn next(&mut self) -> Option<Value> {
        let stdout = self.child.stdout.as_mut().expect("stdout");
        loop {
            if !self.queue.is_empty() {
                let body = self.queue.remove(0);
                return Some(serde_json::from_slice(&body).expect("JSON"));
            }
            let mut buf = [0u8; 4096];
            let n = stdout.read(&mut buf).expect("read");
            if n == 0 {
                return None;
            }
            self.queue = self.decoder.push(&buf[..n]).expect("framed");
        }
    }

    fn call(&mut self, id: i64, method: &str) -> Option<Value> {
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": {}}));
        self.next()
    }
}

// covers: CORE-02
// suite: integration
#[test]
fn the_fake_module_answers_then_crashes_on_the_scripted_call() {
    let (mut module, _dir) =
        Running::start(&FakeModuleProcess::new("fake").crash_on_call(2), "fake");
    module.send(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"api": "v1", "host_version": "0.0.0"}}));
    let hello = module.next().expect("a handshake reply");
    assert_eq!(hello["result"]["name"], json!("fake"));
    assert_eq!(
        hello["result"]["methods"],
        json!(["fake.call", "fake.host"])
    );
    assert_eq!(module.call(2, "ping").expect("a pong")["result"], json!({}));
    assert_eq!(
        module.call(3, "fake.call").expect("an answer")["result"],
        json!({"calls": 1})
    );
    assert_eq!(
        module.call(4, "nope").expect("an error")["error"]["code"],
        json!(-32601)
    );
    // The second call: it dies without answering.
    assert_eq!(module.call(5, "fake.call"), None);
    assert_eq!(module.child.wait().expect("it exits").code(), Some(3));
}

// covers: CORE-02
// suite: integration
#[test]
fn a_lying_fake_module_claims_a_method_it_does_not_declare() {
    let (mut module, _dir) = Running::start(&FakeModuleProcess::new("fake").lie(), "fake");
    module.send(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"api": "v1", "host_version": "0.0.0"}}));
    let hello = module.next().expect("a handshake reply");
    assert_eq!(
        hello["result"]["methods"],
        json!(["fake.call", "fake.host", "fake.secret"])
    );
    assert_eq!(
        module.call(2, "shutdown").expect("an answer")["result"],
        json!({})
    );
    assert!(module.child.wait().expect("it exits").success());
}
