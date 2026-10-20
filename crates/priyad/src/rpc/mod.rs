//! The JSON-RPC side of priyad (02-core §4): a typed [`Router`] of methods,
//! a [`Session`] per connection for notifications and subscriptions, and
//! the socket [`server`].

mod router;
pub mod server;
mod session;

pub use router::Router;
pub use session::Session;
