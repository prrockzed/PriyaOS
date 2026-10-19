//! PriyaOS domain logic. Pure and portable: no system calls, so it builds and
//! tests on macOS. System access lives in `priya-sys` behind `priya-helper`.

pub mod clock;
pub mod paths;
pub mod secret;
