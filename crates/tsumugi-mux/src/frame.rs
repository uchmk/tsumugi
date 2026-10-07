//! One message on the wire: its length as a little-endian `u32`, then the
//! message in postcard (serde's compact binary form: varints, no field names).

use std::io::{self, Read, Write};

use serde::de::DeserializeOwned;
use serde::Serialize;

/// Larger than any screen (a 500 x 200 grid of cells is under 10 MB): a
/// length past this is a stream out of step, not a message.
const MAX: u32 = 64 << 20;

pub fn write<W: Write, T: Serialize>(w: &mut W, msg: &T) -> io::Result<()> {
    let body = postcard::to_allocvec(msg).map_err(io::Error::other)?;
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
    postcard::from_bytes(&body).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
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

    /// What a server of another version says at `Hello` must read in every
    /// version: `Hello` first and `Error` sixteenth, as they were in
    /// protocol 16. Moving either breaks "Stop it and start this version".
    #[test]
    fn hello_and_error_keep_their_places() {
        use crate::proto::ToClient;
        let tag = |m: &ToClient| postcard::to_allocvec(m).unwrap()[0];
        assert_eq!(tag(&ToClient::Hello { version: 1 }), 0);
        assert_eq!(tag(&ToClient::Error("x".into())), 15);
        assert_eq!(postcard::to_allocvec(&ToServer::Hello { version: 1 }).unwrap()[0], 0);
        // The answer a protocol-16 server gives a newer client.
        let mut buf = Vec::new();
        let old = (15u8, "the server speaks version 16, the client 18".to_string());
        let body = postcard::to_allocvec(&old).unwrap();
        buf.extend((body.len() as u32).to_le_bytes());
        buf.extend(body);
        assert!(matches!(read::<_, ToClient>(&mut &buf[..]).unwrap(), ToClient::Error(e) if e.contains("version 16")));
    }

    #[test]
    fn a_length_out_of_step_is_refused() {
        let mut r = &[0xff, 0xff, 0xff, 0xff, 1, 2][..];
        assert_eq!(read::<_, ToServer>(&mut r).unwrap_err().kind(), io::ErrorKind::InvalidData);
    }
}
