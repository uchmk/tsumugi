//! The local, per-user channel between a client and the server, from
//! tsumugi-ipc: a Unix domain socket, or a named pipe on Windows.

pub use tsumugi_ipc::{connect, Address, Conn, Listener};

/// This user's server. `TSUMUGI_ADDRESS` overrides it (a second server for
/// trying something, or the tests).
pub fn address() -> Address {
    Address::per_user("tsumugi", "TSUMUGI_ADDRESS")
}
