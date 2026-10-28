//! One accepted socket: reads the JSON hello and hands the connection to
//! the protocol it asks for.

use std::sync::Arc;

use monarch_proto::{Hello, HelloAck};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::{ApiConnection, AttachConnection, ServerContext, ServerError};

const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Longest handshake line accepted.
const MAX_HELLO: usize = 4096;

/// Routes a new connection to the attach stream or the JSON API.
#[derive(Debug, Clone, Copy, Default)]
pub struct Connection;

impl Connection {
    /// Runs the connection to completion.
    ///
    /// # Errors
    /// The handshake or the stream failed.
    pub async fn serve(ctx: Arc<ServerContext>, mut stream: UnixStream) -> Result<(), ServerError> {
        let io = |e| ServerError::io("handshake", e);
        let hello = read_hello(&mut stream).await.map_err(io)?;
        let (attach, json) = hello
            .as_ref()
            .map_or((false, false), |h| (h.is_attach(), h.is_json()));
        let wire = hello.as_ref().map_or(0, |h| h.wire);
        let ack = if attach && wire != Hello::WIRE {
            let refusal = Hello::wire_mismatch(ctx.session(), Hello::WIRE, wire);
            HelloAck::refuse(VERSION, refusal)
        } else if attach || json {
            HelloAck::accept(VERSION)
        } else {
            HelloAck::refuse(VERSION, "unknown protocol; expected attach or json")
        };
        write_line(&mut stream, &ack).await.map_err(io)?;
        if !ack.ok {
            Ok(())
        } else if attach {
            AttachConnection::serve(ctx, stream).await
        } else if json {
            ApiConnection::serve(ctx, stream).await
        } else {
            Ok(())
        }
    }
}

/// Reads the JSON hello line byte by byte, so nothing after it is consumed.
async fn read_hello(stream: &mut UnixStream) -> std::io::Result<Option<Hello>> {
    let mut line = Vec::new();
    loop {
        let byte = stream.read_u8().await?;
        if byte == b'\n' {
            break;
        }
        line.push(byte);
        if line.len() > MAX_HELLO {
            return Ok(None);
        }
    }
    Ok(serde_json::from_slice(&line).ok())
}

async fn write_line(stream: &mut UnixStream, ack: &HelloAck) -> std::io::Result<()> {
    let mut line = serde_json::to_vec(ack).map_err(std::io::Error::other)?;
    line.push(b'\n');
    stream.write_all(&line).await
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncBufReadExt, BufReader};

    use super::*;
    use crate::test_support::context;

    async fn hello(line: &str) -> HelloAck {
        let (server, mut client) = UnixStream::pair().expect("pair");
        let task = tokio::spawn(Connection::serve(Arc::new(context()), server));
        client.write_all(line.as_bytes()).await.expect("hello");
        let mut ack = String::new();
        BufReader::new(&mut client)
            .read_line(&mut ack)
            .await
            .expect("ack");
        drop(client);
        let _ = task.await;
        serde_json::from_str(&ack).expect("ack json")
    }

    #[tokio::test]
    async fn json_hellos_are_accepted() {
        let line = format!(
            "{}\n",
            serde_json::to_string(&Hello::json("t")).expect("json")
        );
        assert!(hello(&line).await.ok);
    }

    #[tokio::test]
    async fn attach_clients_of_another_build_are_refused() {
        let old = hello("{\"monarch\":1,\"proto\":\"attach\",\"version\":\"t\"}\n").await;
        assert!(!old.ok);
        assert!(
            old.error.is_some_and(|e| e.contains("newer build")),
            "no wire: older"
        );
        let line = format!(
            "{}\n",
            serde_json::to_string(&Hello::attach("t")).expect("json")
        );
        let ack = hello(&line).await;
        assert!(ack.ok && ack.wire == Hello::WIRE);
    }

    #[tokio::test]
    async fn unknown_protocols_are_refused() {
        let ack = hello("{\"monarch\":1,\"proto\":\"x\",\"version\":\"t\"}\n").await;
        assert!(!ack.ok);
        assert!(!hello("not json\n").await.ok);
    }
}
