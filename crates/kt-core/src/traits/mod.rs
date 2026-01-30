//! Core trait definitions

mod connection;
mod session;

pub use connection::{Connection, ConnectionPool};
pub use session::{Session, SessionManager, SessionState};
