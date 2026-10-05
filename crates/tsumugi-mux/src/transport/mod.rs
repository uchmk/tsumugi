//! A local, per-user channel between a client and the server: a Unix domain
//! socket, or a named pipe on Windows. Either way the server listens at one
//! [`Address`], each [`Conn`] is a byte stream both ways, and it splits into a
//! reader and a writer that two threads use at once.

use std::io::{self, Read, Write};
use std::path::PathBuf;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as imp;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as imp;

/// Where the server listens: a socket file, or `\\.\pipe\<name>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Address(pub PathBuf);

impl Address {
    /// This user's server. `TSUMUGI_ADDRESS` overrides it (a second server for
    /// trying something, or the tests).
    pub fn for_user() -> Self {
        if let Some(a) = std::env::var_os("TSUMUGI_ADDRESS").filter(|a| !a.is_empty()) {
            return Self(PathBuf::from(a));
        }
        Self(imp::default_address())
    }
}

pub struct Listener(imp::Listener);

impl Listener {
    /// Listen at `at`. Fails with `AddrInUse` while another server answers
    /// there, so a second server started by accident goes away.
    pub fn bind(at: &Address) -> io::Result<Self> {
        imp::Listener::bind(at).map(Self)
    }

    /// The next client, waiting for one.
    pub fn accept(&self) -> io::Result<Conn> {
        self.0.accept().map(Conn)
    }
}

pub struct Conn(imp::Conn);

/// Connect to the server at `at`; `NotFound` or `ConnectionRefused` when none
/// is listening.
pub fn connect(at: &Address) -> io::Result<Conn> {
    imp::connect(at).map(Conn)
}

impl Conn {
    /// A reader and a writer over the one connection, for two threads.
    pub fn split(self) -> io::Result<(Box<dyn Read + Send>, Box<dyn Write + Send>)> {
        self.0.split()
    }
}
