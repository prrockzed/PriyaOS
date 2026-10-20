//! One run of a module, from handshake to exit (02-core §5.2):
//! `initialize` must answer with the manifest's name, version and methods
//! within the handshake time; then `ping` every so often must be answered
//! in time; to stop, `shutdown`, then SIGTERM, then SIGKILL.

use std::collections::BTreeSet;
use std::sync::Arc;

use priya_api::module::protocol::{
    DeliverEvent, Empty, EventDelivery, Initialize, InitializeParams, InitializeResult, Ping,
    Shutdown,
};
use priya_api::module::{API_VERSION, ModuleManifest};
use priya_api::peer::{CallError, Ended, Peer};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::{MissedTickBehavior, interval_at, timeout};

use super::Timings;
use super::host_api::{HostApi, HostServices};
use super::launch::{Exit, Launched, Running};

/// Why a module didn't get going.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartError {
    /// It failed: it may do better next time.
    Failed(String),
    /// It isn't what its manifest says: trying again won't help.
    Refused(String),
}

/// How a running module stopped being one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Someone asked it to stop.
    Stop,
    /// It failed; the reason, when it didn't just exit.
    Failed(Option<String>),
}

/// A module that has said who it is and is being looked after.
pub struct Session {
    peer: Peer,
    peer_task: JoinHandle<Ended>,
    running: Running,
    delivery: Option<JoinHandle<()>>,
}

impl Session {
    /// Talks to a freshly launched module until it says who it is. On
    /// failure the module is handed back, to be stopped and reported.
    pub async fn start(
        manifest: Arc<ModuleManifest>,
        launched: Launched,
        services: Arc<dyn HostServices>,
        timings: &Timings,
    ) -> Result<Self, (StartError, Running)> {
        let Launched {
            inbound,
            outbound,
            running,
        } = launched;
        let handler = HostApi {
            manifest: manifest.clone(),
            services: services.clone(),
        };
        let (peer, peer_task) = Peer::spawn(inbound, outbound, |_| handler);
        let params = InitializeParams {
            api: API_VERSION.to_owned(),
            host_version: env!("CARGO_PKG_VERSION").to_owned(),
        };
        let hello = match timeout(timings.handshake, peer.call::<Initialize>(&params)).await {
            Ok(Ok(hello)) => hello,
            Ok(Err(error)) => {
                peer_task.abort();
                return Err((StartError::Failed(start_failure(&error)), running));
            }
            Err(_) => {
                peer_task.abort();
                let reason = format!(
                    "it didn't say who it is within {}",
                    seconds(timings.handshake)
                );
                return Err((StartError::Failed(reason), running));
            }
        };
        if let Err(reason) = matches_manifest(&manifest, &hello) {
            peer_task.abort();
            return Err((StartError::Refused(reason), running));
        }
        let delivery = (!manifest.subscribes.events.is_empty()).then(|| {
            let mut events = services.subscribe(manifest.subscribes.events.clone());
            let peer = peer.clone();
            tokio::spawn(async move {
                // One at a time: the next waits for the module's answer.
                while let Some(event) = events.recv().await {
                    if peer
                        .call::<DeliverEvent>(&EventDelivery { event })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            })
        });
        Ok(Self {
            peer,
            peer_task,
            running,
            delivery,
        })
    }

    pub fn peer(&self) -> &Peer {
        &self.peer
    }

    pub fn pid(&self) -> Option<u32> {
        self.running.pid()
    }

    /// Looks after the module until it fails or `stop` is set.
    pub async fn watch(
        &mut self,
        stop: &mut watch::Receiver<Option<String>>,
        timings: &Timings,
    ) -> Outcome {
        let start = tokio::time::Instant::now() + timings.ping_every;
        let mut pings = interval_at(start, timings.ping_every);
        pings.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                // In this order: a broken conversation makes priyad close
                // the module's stdin, so its exit may follow at once, and
                // the break is the reason worth reporting.
                biased;
                // The `Ref` it returns holds a lock, so it is dropped here.
                () = stopped(stop) => return Outcome::Stop,
                ended = &mut self.peer_task => {
                    return match ended {
                        Ok(Ended::Broken(reason)) => {
                            Outcome::Failed(Some(format!("it broke the protocol: {reason}")))
                        }
                        // Its output closed: it is exiting, or should be.
                        _ => match timeout(timings.stop_grace, self.running.exited()).await {
                            Ok(_) => Outcome::Failed(None),
                            Err(_) => Outcome::Failed(Some("it closed its output but kept running".into())),
                        },
                    };
                }
                _ = self.running.exited() => return Outcome::Failed(None),
                _ = pings.tick() => {
                    let pong = timeout(timings.ping_timeout, self.peer.call::<Ping>(&Empty {})).await;
                    if !matches!(pong, Ok(Ok(_))) {
                        return Outcome::Failed(Some(format!(
                            "it didn't answer a ping within {}",
                            seconds(timings.ping_timeout)
                        )));
                    }
                }
            }
        }
    }

    /// Stops it politely: `shutdown`, then SIGTERM, then SIGKILL, each
    /// after the stop grace time. Returns how it ended.
    pub async fn stop(mut self, timings: &Timings) -> (Exit, Option<String>) {
        self.stop_delivery();
        let _ = timeout(timings.stop_grace, self.peer.call::<Shutdown>(&Empty {})).await;
        let exit = stop_running(&mut self.running, timings).await;
        self.peer_task.abort();
        let tail = self.running.stderr_tail().await;
        (exit, tail)
    }

    /// Ends a module that failed: whatever is left of it is killed.
    pub async fn end(mut self, timings: &Timings) -> (Exit, Option<String>) {
        self.stop_delivery();
        self.peer_task.abort();
        let exit = kill(&mut self.running, timings).await;
        let tail = self.running.stderr_tail().await;
        (exit, tail)
    }

    fn stop_delivery(&mut self) {
        if let Some(delivery) = self.delivery.take() {
            delivery.abort();
        }
    }
}

/// Resolves once a stop is asked for.
pub async fn stopped(stop: &mut watch::Receiver<Option<String>>) {
    let _ = stop.wait_for(Option::is_some).await;
}

/// Waits out a module that was asked to stop, then SIGTERM, then SIGKILL.
async fn stop_running(running: &mut Running, timings: &Timings) -> Exit {
    if let Ok(exit) = timeout(timings.stop_grace, running.exited()).await {
        return exit;
    }
    running.terminate();
    if let Ok(exit) = timeout(timings.stop_grace, running.exited()).await {
        return exit;
    }
    kill(running, timings).await
}

/// Kills what is left of a module and says how it ended.
pub async fn kill(running: &mut Running, timings: &Timings) -> Exit {
    running.kill();
    timeout(timings.stop_grace, running.exited())
        .await
        .unwrap_or_else(|_| Exit {
            error: Some("it couldn't be stopped".into()),
            ..Exit::default()
        })
}

/// Whether a module's answer to `initialize` matches its manifest.
pub fn matches_manifest(manifest: &ModuleManifest, hello: &InitializeResult) -> Result<(), String> {
    let info = &manifest.module;
    if hello.api != API_VERSION {
        return Err(format!(
            "it speaks module API `{}`, not `{API_VERSION}`",
            hello.api
        ));
    }
    if hello.name != info.name || hello.version != info.version {
        return Err(format!(
            "it says it is {} {}, but its module.toml says {} {}",
            hello.name, hello.version, info.name, info.version
        ));
    }
    let said: BTreeSet<&String> = hello.methods.iter().collect();
    let listed: BTreeSet<&String> = manifest.provides.methods.iter().collect();
    if said != listed {
        let missing: Vec<&str> = listed.difference(&said).map(|m| m.as_str()).collect();
        let extra: Vec<&str> = said.difference(&listed).map(|m| m.as_str()).collect();
        return Err(format!(
            "its methods don't match its module.toml (missing: {}; not listed: {})",
            list(&missing),
            list(&extra)
        ));
    }
    Ok(())
}

fn list(items: &[&str]) -> String {
    if items.is_empty() {
        "none".into()
    } else {
        items.join(", ")
    }
}

fn start_failure(error: &CallError) -> String {
    match error {
        CallError::Rpc(error) => format!("it refused to start: {}", error.data.user_message),
        CallError::Closed => "it stopped before saying who it is".into(),
        other => format!("its answer to `initialize` couldn't be read: {other}"),
    }
}

/// A duration as people read it: `5 s`, `200 ms`.
pub fn seconds(duration: std::time::Duration) -> String {
    if duration.subsec_millis() == 0 {
        format!("{} s", duration.as_secs())
    } else {
        format!("{} ms", duration.as_millis())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: CORE-02
    // suite: unit
    #[test]
    fn the_handshake_must_match_the_manifest() {
        let manifest = ModuleManifest::parse(
            "[module]\nname = \"echo\"\nversion = \"0.1.0\"\napi = \"v1\"\nkind = \"builtin\"\n\
             [provides]\nmethods = [\"echo.say\", \"echo.emit\"]\n",
        )
        .unwrap();
        let hello = |name: &str, version: &str, methods: &[&str]| {
            InitializeResult::v1(name, version, methods.iter().copied())
        };
        assert!(
            matches_manifest(
                &manifest,
                &hello("echo", "0.1.0", &["echo.emit", "echo.say"])
            )
            .is_ok(),
            "any order"
        );
        let mut other_api = hello("echo", "0.1.0", &["echo.say", "echo.emit"]);
        other_api.api = "v2".into();
        assert!(
            matches_manifest(&manifest, &other_api)
                .unwrap_err()
                .contains("speaks module API `v2`")
        );
        let renamed = matches_manifest(&manifest, &hello("ekko", "0.1.0", &[])).unwrap_err();
        assert_eq!(
            renamed,
            "it says it is ekko 0.1.0, but its module.toml says echo 0.1.0"
        );
        assert!(matches_manifest(&manifest, &hello("echo", "0.2.0", &[])).is_err());
        let methods =
            matches_manifest(&manifest, &hello("echo", "0.1.0", &["echo.say"])).unwrap_err();
        assert!(
            methods.ends_with("(missing: echo.emit; not listed: none)"),
            "{methods}"
        );

        assert_eq!(seconds(std::time::Duration::from_secs(5)), "5 s");
        assert_eq!(seconds(std::time::Duration::from_millis(300)), "300 ms");
        let closed = start_failure(&CallError::Closed);
        assert_eq!(closed, "it stopped before saying who it is");
        assert!(start_failure(&CallError::Mismatch("x".into())).contains("couldn't be read"));
    }
}
