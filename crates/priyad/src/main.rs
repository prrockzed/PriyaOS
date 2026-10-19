//! The priyad binary. Until sub-phase 0.7 gives it an API it starts, logs and
//! exits, so packaging and CI have a real binary to work with.

fn main() {
    priyad::logging::init("info");
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "priyad started; nothing to serve yet"
    );
}
