//! The fake module process ([`priya_testkit::FakeModuleProcess`]): speaks
//! the module protocol by hand, so it can break it on purpose. It reads
//! `script.json` from the folder priyad starts it in. stderr is a module's
//! log, so printing to it is fine here.

#![allow(clippy::print_stderr)]

use std::io::{Read, Write};
use std::process::ExitCode;

use priya_api::framing::lsp::{Decoder, encode};
use priya_testkit::fake_module::Script;
use serde_json::{Value, json};

fn main() -> ExitCode {
    let script: Script = std::fs::read_to_string("script.json")
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let name = std::env::var("PRIYA_MODULE_NAME").unwrap_or_else(|_| "fake".into());
    let mut module = Fake {
        name,
        script,
        stdin: std::io::stdin().lock(),
        decoder: Decoder::default(),
        queue: Vec::new(),
        calls: 0,
        silent: false,
        next_id: 1,
    };
    match module.run() {
        Ok(code) => code,
        Err(reason) => {
            eprintln!("{reason}");
            ExitCode::FAILURE
        }
    }
}

struct Fake<R> {
    name: String,
    script: Script,
    stdin: R,
    decoder: Decoder,
    queue: Vec<Vec<u8>>,
    calls: u32,
    /// Hanging: answers nothing more.
    silent: bool,
    next_id: i64,
}

impl<R: Read> Fake<R> {
    fn run(&mut self) -> Result<ExitCode, String> {
        while let Some(message) = self.next()? {
            let Some(method) = message["method"].as_str().map(str::to_owned) else {
                continue; // an answer nobody waits for
            };
            let id = message["id"].clone();
            if id.is_null() || self.silent {
                continue;
            }
            let params = message["params"].clone();
            match method.as_str() {
                "initialize" => {
                    let mut methods =
                        vec![format!("{}.call", self.name), format!("{}.host", self.name)];
                    if self.script.lie {
                        methods.push(format!("{}.secret", self.name));
                    }
                    let hello = json!({"api": "v1", "name": self.name, "version": "0.1.0", "methods": methods});
                    write(&json!({"jsonrpc": "2.0", "id": id, "result": hello}))?;
                    self.after_handshake()?;
                }
                "shutdown" => {
                    write(&json!({"jsonrpc": "2.0", "id": id, "result": {}}))?;
                    return Ok(ExitCode::SUCCESS);
                }
                "ping" => write(&json!({"jsonrpc": "2.0", "id": id, "result": {}}))?,
                _ if method.ends_with(".call") || method.ends_with(".host") => {
                    self.calls += 1;
                    if self.script.crash_on_call == Some(self.calls) {
                        eprintln!("crashing on purpose on call {}", self.calls);
                        return Ok(ExitCode::from(3));
                    }
                    let answer = if method.ends_with(".host") {
                        self.host_call(&params)?
                    } else {
                        json!({"result": {"calls": self.calls}})
                    };
                    let mut reply = json!({"jsonrpc": "2.0", "id": id});
                    for (key, value) in answer.as_object().cloned().unwrap_or_default() {
                        reply[key] = value;
                    }
                    write(&reply)?;
                }
                _ => write(&json!({"jsonrpc": "2.0", "id": id,
                    "error": {"code": -32601, "message": "Method not found"}}))?,
            }
        }
        Ok(ExitCode::SUCCESS)
    }

    fn after_handshake(&mut self) -> Result<(), String> {
        let mut stdout = std::io::stdout().lock();
        if self.script.garbage {
            let _ = stdout.write_all(b"this is not a message\r\n\r\n");
            let _ = stdout.flush();
        }
        if self.script.oversize {
            let _ = stdout.write_all(b"Content-Length: 17000000\r\n\r\n");
            let _ = stdout.flush();
        }
        self.silent = self.script.hang;
        Ok(())
    }

    /// Makes the host call in `params` and returns priyad's answer: its
    /// `result` or its `error`.
    fn host_call(&mut self, params: &Value) -> Result<Value, String> {
        let id = Value::from(1_000_000 + self.next_id);
        self.next_id += 1;
        write(
            &json!({"jsonrpc": "2.0", "id": id, "method": params["method"], "params": params["params"]}),
        )?;
        // Requests that arrive meanwhile (pings) wait their turn.
        let mut held = Vec::new();
        let answer = loop {
            let Some(message) = self.next()? else {
                return Err("priyad went away during a host call".into());
            };
            if message.get("method").is_none() && message["id"] == id {
                let mut answer = json!({});
                if let Some(result) = message.get("result") {
                    answer["result"] = result.clone();
                }
                if let Some(error) = message.get("error") {
                    answer["error"] = error.clone();
                }
                break answer;
            }
            held.push(serde_json::to_vec(&message).unwrap_or_default());
        };
        // `queue` is popped from the end: the first held comes out first.
        self.queue.extend(held.into_iter().rev());
        Ok(answer)
    }

    /// The next message, or none at the end of input.
    fn next(&mut self) -> Result<Option<Value>, String> {
        loop {
            if let Some(body) = self.queue.pop() {
                return serde_json::from_slice(&body)
                    .map(Some)
                    .map_err(|e| e.to_string());
            }
            let mut buffer = [0_u8; 64 * 1024];
            let n = self.stdin.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                return Ok(None);
            }
            let mut bodies = self.decoder.push(&buffer[..n]).map_err(|e| e.to_string())?;
            bodies.reverse();
            self.queue.extend(bodies);
        }
    }
}

fn write(message: &Value) -> Result<(), String> {
    let bytes = encode(message).map_err(|e| e.to_string())?;
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&bytes).map_err(|e| e.to_string())?;
    stdout.flush().map_err(|e| e.to_string())
}
