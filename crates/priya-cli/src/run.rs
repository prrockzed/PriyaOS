//! Running a command: call the daemon, then print the answer for people
//! (or as JSON with `--json`).

use std::io::{IsTerminal, Write};
use std::path::Path;

use priya_api::client::{Client, ClientError};
use priya_api::methods::events::{
    ENDED_NOTIFICATION, EVENT_NOTIFICATION, EndedNotification, EventNotification, SubscribeParams,
    UnsubscribeParams,
};
use priya_api::methods::settings::{GetParams, GetResult, SetParams, SettingLayer, UnsetParams};
use priya_api::methods::system::{
    CheckStatus, DoctorParams, DoctorReport, FixMode, FixScope, StatusResult,
};
use priya_api::methods::{NoParams, audit, events, settings, system};
use priya_api::rpc::Request;
use serde::Serialize;
use serde_json::Value;

use crate::cli::{ApiCommand, AuditCommand, Command, EventsCommand, SettingsCommand};

/// How a command ended, as an exit code.
#[derive(Debug)]
pub enum Failure {
    /// The daemon (or the command) refused, with a plain message: exit 1.
    Refused(String),
    /// The daemon isn't running: exit 3.
    Unreachable,
}

impl From<ClientError> for Failure {
    fn from(e: ClientError) -> Self {
        match e {
            ClientError::Unreachable(_) => Self::Unreachable,
            ClientError::Api(error) => Self::Refused(error.user_message),
            other => Self::Refused(other.to_string()),
        }
    }
}

/// Output, so tests can capture it.
pub struct Out<'a> {
    pub json: bool,
    pub stdout: &'a mut dyn Write,
}

impl Out<'_> {
    fn line(&mut self, text: impl AsRef<str>) {
        let _ = writeln!(self.stdout, "{}", text.as_ref());
    }

    /// One line of compact JSON.
    fn json(&mut self, value: &impl Serialize) {
        if let Ok(text) = serde_json::to_string(value) {
            self.line(text);
        }
    }
}

pub async fn run(command: Command, socket: &Path, out: &mut Out<'_>) -> Result<(), Failure> {
    let mut client = Client::connect(socket).await?;
    match command {
        Command::Status => {
            let status = client.call::<system::Status>(&NoParams {}).await?;
            if out.json {
                out.json(&status);
            } else {
                print_status(&status, out);
            }
        }
        Command::Doctor { fix, dry_run, yes } => {
            doctor(&mut client, fix, dry_run, yes, out).await?
        }
        Command::Settings(command) => settings(&mut client, command, out).await?,
        Command::Events(EventsCommand::Tail {
            types,
            stream,
            from,
        }) => {
            tail(
                &mut client,
                SubscribeParams {
                    types,
                    stream_prefix: stream,
                    from_seq: from,
                },
                out,
            )
            .await?;
        }
        Command::Audit(AuditCommand::Verify { from, to }) => {
            let report = client
                .call::<audit::Verify>(&audit::VerifyParams { from, to })
                .await?;
            if out.json {
                out.json(&report);
            } else if report.ok {
                out.line(format!(
                    "The history is intact: {} events checked.",
                    report.checked
                ));
            }
            if !report.ok {
                let seq = report.first_bad_seq.unwrap_or_default();
                let what = match report.reason {
                    Some(audit::BreakReason::SeqGap) => {
                        format!("Event {seq} is missing from the history.")
                    }
                    Some(audit::BreakReason::PrevLinkBroken) => {
                        format!("Event {seq} was moved or the one before it removed.")
                    }
                    _ => format!("Event {seq} was changed after it was written."),
                };
                return Err(Failure::Refused(what));
            }
        }
        Command::Api(ApiCommand::Methods) => {
            let methods = client.call::<system::Methods>(&NoParams {}).await?;
            if out.json {
                out.json(&methods);
            } else {
                let width = methods
                    .methods
                    .iter()
                    .map(|m| m.name.len())
                    .max()
                    .unwrap_or(0);
                for method in &methods.methods {
                    out.line(format!("{:width$}  {}", method.name, method.summary));
                }
            }
        }
        Command::Completions { .. } => unreachable!("handled before connecting"),
    }
    Ok(())
}

fn print_status(status: &StatusResult, out: &mut Out<'_>) {
    out.line(format!(
        "PriyaOS is running (version {}, up {}).",
        status.version,
        duration(status.uptime_seconds)
    ));
    match &status.model {
        Some(model) => out.line(format!("Model: {} from {}", model.model, model.provider)),
        None => out.line("Model: none yet. Set one with: priya settings set ai.strategy fixed:<provider>/<model>"),
    }
    out.line(format!(
        "History: {} events in {}",
        status.event_count, status.db_path
    ));
    if status.modules.is_empty() {
        out.line("Modules: none running");
    } else {
        out.line(format!("Modules: {}", status.modules.join(", ")));
    }
}

/// "40 s", "3 min", "2 h 5 min".
fn duration(seconds: u64) -> String {
    match seconds {
        0..60 => format!("{seconds} s"),
        60..3600 => format!("{} min", seconds / 60),
        _ => format!("{} h {} min", seconds / 3600, seconds % 3600 / 60),
    }
}

async fn settings(
    client: &mut Client,
    command: SettingsCommand,
    out: &mut Out<'_>,
) -> Result<(), Failure> {
    match command {
        SettingsCommand::Get { key, explain } => {
            let result = client
                .call::<settings::Get>(&GetParams { key, explain })
                .await?;
            if out.json {
                out.json(&result);
            } else {
                print_settings(&result, out);
            }
        }
        SettingsCommand::Set { key, value } => {
            // A bare word is text; anything that reads as JSON is that.
            let value = serde_json::from_str(&value).unwrap_or(Value::String(value));
            client
                .call::<settings::Set>(&SetParams {
                    key: key.clone(),
                    value,
                })
                .await?;
            if !out.json {
                out.line(format!("Set {key}."));
            } else {
                out.json(&serde_json::json!({}));
            }
        }
        SettingsCommand::Unset { key } => {
            client
                .call::<settings::Unset>(&UnsetParams { key: key.clone() })
                .await?;
            if !out.json {
                out.line(format!("Removed {key}; its default applies."));
            } else {
                out.json(&serde_json::json!({}));
            }
        }
        SettingsCommand::Schema => {
            let schema = client.call::<settings::GetSchema>(&NoParams {}).await?;
            if out.json {
                out.json(&schema);
            } else {
                out.line(serde_json::to_string_pretty(&schema.schema).unwrap_or_default());
            }
        }
    }
    Ok(())
}

fn print_settings(result: &GetResult, out: &mut Out<'_>) {
    // A real setting with no value, such as `ai.strategy` before a model
    // is chosen.
    if result.value.is_null() {
        out.line("not set");
        return;
    }
    let Some(origins) = &result.origins else {
        match &result.value {
            Value::String(text) => out.line(text),
            Value::Object(_) => {
                out.line(serde_json::to_string_pretty(&result.value).unwrap_or_default())
            }
            other => out.line(other.to_string()),
        }
        return;
    };
    let width = origins.iter().map(|o| o.key.len()).max().unwrap_or(0);
    for origin in origins {
        let from = match origin.layer {
            SettingLayer::BuiltIn => "built in".to_owned(),
            SettingLayer::Task => "this request".to_owned(),
            layer => {
                let name = match layer {
                    SettingLayer::Defaults => "shipped default",
                    SettingLayer::System => "administrator",
                    SettingLayer::User => "yours",
                    _ => "project",
                };
                match (&origin.file, origin.line) {
                    (Some(file), Some(line)) => format!("{name}: {} line {line}", home_short(file)),
                    (Some(file), None) => format!("{name}: {}", home_short(file)),
                    _ => name.to_owned(),
                }
            }
        };
        out.line(format!(
            "{:width$} = {}   ({from})",
            origin.key, origin.value
        ));
    }
}

/// `/home/p/x` as `~/x`.
fn home_short(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() => match path.strip_prefix(&home) {
            Some(rest) => format!("~{rest}"),
            None => path.to_owned(),
        },
        _ => path.to_owned(),
    }
}

async fn doctor(
    client: &mut Client,
    fix: bool,
    dry_run: bool,
    yes: bool,
    out: &mut Out<'_>,
) -> Result<(), Failure> {
    let cli_version = Some(env!("CARGO_PKG_VERSION").to_owned());
    let ask = |fix| DoctorParams {
        fix,
        cli_version: cli_version.clone(),
    };
    let report = client
        .call::<system::Doctor>(&ask(fix.then_some(FixMode::DryRun)))
        .await?;
    if !fix || dry_run {
        print_doctor(&report, fix, out);
        return outcome(&report);
    }
    let fixes = report
        .checks
        .iter()
        .filter(|c| c.fix.as_ref().is_some_and(|f| f.scope == FixScope::User))
        .count();
    if fixes == 0 {
        print_doctor(&report, true, out);
        if !out.json {
            out.line("Nothing to fix here.");
        }
        return outcome(&report);
    }
    if !yes {
        print_doctor(&report, true, out);
        // No one to ask, and a question would spoil JSON output.
        if out.json || !std::io::stdin().is_terminal() {
            return Err(Failure::Refused(
                "Add --yes to make the fixes without being asked.".into(),
            ));
        }
        let _ = write!(
            out.stdout,
            "Make {fixes} fix{}? [y/N] ",
            if fixes == 1 { "" } else { "es" }
        );
        let _ = out.stdout.flush();
        let mut answer = String::new();
        let _ = std::io::stdin().read_line(&mut answer);
        if !matches!(answer.trim(), "y" | "Y" | "yes") {
            out.line("Nothing changed.");
            return Ok(());
        }
    }
    let applied = client
        .call::<system::Doctor>(&ask(Some(FixMode::Apply)))
        .await?;
    print_doctor(&applied, false, out);
    if !out.json {
        out.line(format!(
            "Fixed: {}.",
            if applied.fixed.is_empty() {
                "nothing".to_owned()
            } else {
                applied.fixed.join(", ")
            }
        ));
    }
    outcome(&applied)
}

fn print_doctor(report: &DoctorReport, with_fixes: bool, out: &mut Out<'_>) {
    if out.json {
        out.json(report);
        return;
    }
    for check in &report.checks {
        let mark = match check.status {
            CheckStatus::Ok => "✓",
            CheckStatus::Info => "i",
            CheckStatus::Warn => "!",
            CheckStatus::Fail => "✗",
        };
        out.line(format!("{mark} {}: {}", check.title, check.summary));
        if check.status >= CheckStatus::Warn
            && let Some(details) = &check.details
        {
            for line in details.lines() {
                out.line(format!("    {line}"));
            }
        }
        if with_fixes && let Some(fix) = &check.fix {
            match fix.scope {
                FixScope::User => out.line(format!("    Fix: {}", fix.description)),
                FixScope::System => out.line(format!(
                    "    Fix (needs the system helper, from Phase 2): {}",
                    fix.description
                )),
            }
        }
    }
}

/// Failing checks make the command fail, for scripts.
fn outcome(report: &DoctorReport) -> Result<(), Failure> {
    let failed = report
        .checks
        .iter()
        .filter(|c| c.status == CheckStatus::Fail)
        .count();
    if failed == 0 {
        Ok(())
    } else {
        Err(Failure::Refused(format!(
            "{failed} check{} failed.",
            if failed == 1 { "" } else { "s" }
        )))
    }
}

async fn tail(
    client: &mut Client,
    params: SubscribeParams,
    out: &mut Out<'_>,
) -> Result<(), Failure> {
    let subscription = client.call::<events::Subscribe>(&params).await?;
    loop {
        let next = tokio::select! {
            next = client.next_notification() => next?,
            _ = tokio::signal::ctrl_c() => {
                let _ = client
                    .call::<events::Unsubscribe>(&UnsubscribeParams { subscription: subscription.subscription.clone() })
                    .await;
                return Ok(());
            }
        };
        let Some(notification) = next else {
            return Err(Failure::Refused(
                "The connection to priyad closed, so the stream stopped.".to_owned(),
            ));
        };
        let event = match Streamed::read(notification, &subscription.subscription) {
            Streamed::Event(event) => event,
            // Stopped without being asked to: a failure, for scripts.
            Streamed::Ended(message) => return Err(Failure::Refused(message)),
            Streamed::Other => continue,
        };
        if out.json {
            out.json(&event.event);
        } else {
            let e = &event.event.event;
            let when =
                e.ts.format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_default();
            out.line(format!(
                "{:>6}  {when}  {:<22} {:<16} {}",
                event.event.seq, e.event_type, e.stream, e.payload
            ));
        }
    }
}

/// A notification, as `events tail` sees it.
#[derive(Debug, PartialEq)]
enum Streamed {
    Event(Box<EventNotification>),
    /// The stream stopped on its own, with why.
    Ended(String),
    /// For another subscription, or not understood.
    Other,
}

impl Streamed {
    fn read(notification: Request, subscription: &str) -> Self {
        let params = notification.params.unwrap_or_default();
        match notification.method.as_str() {
            EVENT_NOTIFICATION => match serde_json::from_value::<EventNotification>(params) {
                Ok(event) if event.subscription == subscription => Self::Event(Box::new(event)),
                _ => Self::Other,
            },
            ENDED_NOTIFICATION => match serde_json::from_value::<EndedNotification>(params) {
                Ok(ended) if ended.subscription == subscription => Self::Ended(ended.message),
                _ => Self::Other,
            },
            _ => Self::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    // covers: CORE-03
    // suite: unit
    #[test]
    fn an_unset_setting_reads_as_not_set() {
        let printed = |value: Value, explain: bool| {
            let mut buffer = Vec::new();
            let result = GetResult {
                value,
                origins: explain.then(Vec::new),
            };
            print_settings(
                &result,
                &mut Out {
                    json: false,
                    stdout: &mut buffer,
                },
            );
            String::from_utf8(buffer).unwrap()
        };
        assert_eq!(printed(Value::Null, false), "not set\n");
        assert_eq!(printed(Value::Null, true), "not set\n");
        assert_eq!(printed(json!("dark"), false), "dark\n");
    }

    // covers: CORE-04
    // suite: unit
    #[test]
    fn tail_follows_only_its_own_stream_and_stops_when_it_ends() {
        let ended = |subscription: &str| {
            Request::notification(
                ENDED_NOTIFICATION,
                json!({"subscription": subscription, "reason": "cancelled", "message": "priyad is stopping."}),
            )
        };
        assert_eq!(
            Streamed::read(ended("sub-1"), "sub-1"),
            Streamed::Ended("priyad is stopping.".into())
        );
        assert_eq!(Streamed::read(ended("sub-2"), "sub-1"), Streamed::Other);
        let unreadable =
            Request::notification(EVENT_NOTIFICATION, json!({"subscription": "sub-1"}));
        assert_eq!(Streamed::read(unreadable, "sub-1"), Streamed::Other);
        let unknown = Request::notification("v1.something.new", json!({}));
        assert_eq!(Streamed::read(unknown, "sub-1"), Streamed::Other);
    }

    // covers: CORE-04
    // suite: unit
    #[test]
    fn durations_read_plainly() {
        assert_eq!(duration(40), "40 s");
        assert_eq!(duration(200), "3 min");
        assert_eq!(duration(7500), "2 h 5 min");
    }
}
