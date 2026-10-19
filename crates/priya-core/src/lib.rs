//! PriyaOS domain logic. Pure and portable: no system calls, so it builds and
//! tests on macOS. System access lives in `priya-sys` behind `priya-helper`.

pub mod clock;
pub mod paths;
pub mod secret;

#[cfg(test)]
mod demo {
    // covers: infra
    // suite: unit
    #[test]
    fn demo_failing_test() {
        assert_eq!(1 + 1, 3, "demo: this test fails on purpose");
    }
}
