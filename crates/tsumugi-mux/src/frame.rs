//! One message on the wire, in `ito_ipc::frame`: its length as a
//! little-endian `u32`, then the message in postcard. Here are the tests that
//! hold the tsumugi protocol's own messages to it.

pub use ito_ipc::frame::{read, write};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{ToServer, VERSION};

    #[test]
    fn protocol_messages_come_back_as_they_went() {
        let mut buf = Vec::new();
        write(&mut buf, &ToServer::Hello { version: VERSION }).unwrap();
        write(&mut buf, &ToServer::Input { id: 7, bytes: b"ls\r".to_vec() }).unwrap();
        let mut r = &buf[..];
        assert!(matches!(read::<_, ToServer>(&mut r).unwrap(), ToServer::Hello { version: VERSION }));
        assert!(matches!(read::<_, ToServer>(&mut r).unwrap(), ToServer::Input { id: 7, bytes } if bytes == b"ls\r"));
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
}
