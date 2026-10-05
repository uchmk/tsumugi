//! One message on the wire: its length as a little-endian `u32`, then the
//! message in bincode.

use std::io::{self, Read, Write};

use serde::de::DeserializeOwned;
use serde::Serialize;

/// Larger than any screen (a 500 x 200 grid of cells is under 10 MB): a
/// length past this is a stream out of step, not a message.
const MAX: u32 = 64 << 20;

pub fn write<W: Write, T: Serialize>(w: &mut W, msg: &T) -> io::Result<()> {
    let body = bincode::serialize(msg).map_err(io::Error::other)?;
    let len = u32::try_from(body.len()).ok().filter(|&n| n <= MAX).ok_or_else(|| io::Error::other("message too large"))?;
    let mut out = Vec::with_capacity(body.len() + 4);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&body);
    w.write_all(&out)?;
    w.flush()
}

/// The next message, or `UnexpectedEof` when the other end has gone.
pub fn read<R: Read, T: DeserializeOwned>(r: &mut R) -> io::Result<T> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    let len = u32::from_le_bytes(len);
    if len > MAX {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("a message of {len} bytes")));
    }
    let mut body = vec![0u8; len as usize];
    r.read_exact(&mut body)?;
    bincode::deserialize(&body).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{ToServer, VERSION};

    #[test]
    fn a_message_comes_back_as_it_went() {
        let mut buf = Vec::new();
        write(&mut buf, &ToServer::Hello { version: VERSION }).unwrap();
        write(&mut buf, &ToServer::Input { id: 7, bytes: b"ls\r".to_vec() }).unwrap();
        let mut r = &buf[..];
        assert!(matches!(read::<_, ToServer>(&mut r).unwrap(), ToServer::Hello { version: VERSION }));
        assert!(matches!(read::<_, ToServer>(&mut r).unwrap(), ToServer::Input { id: 7, bytes } if bytes == b"ls\r"));
        assert_eq!(read::<_, ToServer>(&mut r).unwrap_err().kind(), io::ErrorKind::UnexpectedEof, "then the end");
    }

    #[test]
    fn a_length_out_of_step_is_refused() {
        let mut r = &[0xff, 0xff, 0xff, 0xff, 1, 2][..];
        assert_eq!(read::<_, ToServer>(&mut r).unwrap_err().kind(), io::ErrorKind::InvalidData);
    }
}
