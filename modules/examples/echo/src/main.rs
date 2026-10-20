//! The echo module as its own process: JSON-RPC on stdin and stdout.

fn main() -> std::process::ExitCode {
    priya_module_sdk::run_stdio(priya_echo_example::Echo)
}
