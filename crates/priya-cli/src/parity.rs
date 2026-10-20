//! Every API method has a command, and every command a method (02-core §4,
//! CORE-04). The parity test keeps this table, the API and the CLI in step.

/// API method → the command that calls it.
pub const PARITY: &[(&str, &str)] = &[
    ("v1.system.status", "status"),
    ("v1.system.methods", "api methods"),
    ("v1.system.doctor", "doctor"),
    ("v1.settings.get", "settings get"),
    ("v1.settings.set", "settings set"),
    ("v1.settings.unset", "settings unset"),
    ("v1.settings.schema", "settings schema"),
    ("v1.events.subscribe", "events tail"),
    // Ctrl+C in `events tail` unsubscribes.
    ("v1.events.unsubscribe", "events tail"),
    ("v1.audit.verify", "audit verify"),
];

/// Commands that need no daemon, so have no method.
pub const LOCAL: &[&str] = &["completions"];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use clap::CommandFactory;
    use priya_api::methods::ALL;

    use super::*;
    use crate::cli::Cli;

    /// Every leaf command, as "group name".
    fn commands(command: &clap::Command, prefix: &str, out: &mut BTreeSet<String>) {
        for sub in command.get_subcommands().filter(|c| c.get_name() != "help") {
            let path = format!("{prefix}{}", sub.get_name());
            if sub.has_subcommands() {
                commands(sub, &format!("{path} "), out);
            } else {
                out.insert(path);
            }
        }
    }

    // covers: CORE-04
    // suite: contract
    // acceptance: 0.7
    // regression: 0.7
    #[test]
    fn cli_parity_covers_every_api_method() {
        let methods: BTreeSet<&str> = ALL.iter().map(|m| m.name).collect();
        let mapped: BTreeSet<&str> = PARITY.iter().map(|(method, _)| *method).collect();
        assert_eq!(
            methods, mapped,
            "every method has a command, and only real methods"
        );

        let mut cli = BTreeSet::new();
        commands(&Cli::command(), "", &mut cli);
        let used: BTreeSet<String> = PARITY
            .iter()
            .map(|(_, command)| (*command).to_owned())
            .chain(LOCAL.iter().map(|c| (*c).to_owned()))
            .collect();
        assert_eq!(cli, used, "every command calls a method (or is local)");
    }
}
