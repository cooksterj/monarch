//! A blocking client for the session's JSON API, for short-lived callers
//! like hooks.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, bail};
use monarch_proto::{Hello, HelloAck, Request, Response};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// How long any step may take before the caller gives up.
const TIMEOUT: Duration = Duration::from_secs(2);
/// Longest answer line read.
const MAX_LINE: u64 = 64 * 1024;

/// One connection that has completed the JSON hello.
pub(crate) struct ApiClient {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
    next_id: u64,
}

impl ApiClient {
    /// Connects to the session at `socket` and says hello.
    pub fn connect(socket: &Path) -> anyhow::Result<Self> {
        let stream = UnixStream::connect(socket)
            .with_context(|| format!("connecting to {}", socket.display()))?;
        stream.set_read_timeout(Some(TIMEOUT))?;
        stream.set_write_timeout(Some(TIMEOUT))?;
        let mut client = Self {
            reader: BufReader::new(stream.try_clone()?),
            writer: stream,
            next_id: 1,
        };
        client.send(&Hello::json(env!("CARGO_PKG_VERSION")))?;
        let ack: HelloAck = client.receive()?;
        if !ack.ok {
            bail!("server refused: {}", ack.error.unwrap_or_default());
        }
        Ok(client)
    }

    /// Calls `method` and waits for its answer.
    pub fn call(&mut self, method: &str, params: &impl Serialize) -> anyhow::Result<()> {
        let request = Request::new(self.next_id, method, params)?;
        self.next_id += 1;
        self.send(&request)?;
        let response: Response = self.receive()?;
        match response.error {
            Some(error) => bail!("{method}: {}", error.message),
            None => Ok(()),
        }
    }

    fn send(&mut self, value: &impl Serialize) -> anyhow::Result<()> {
        let mut line = serde_json::to_vec(value)?;
        line.push(b'\n');
        self.writer.write_all(&line)?;
        Ok(())
    }

    fn receive<T: DeserializeOwned>(&mut self) -> anyhow::Result<T> {
        let mut line = String::new();
        Read::take(&mut self.reader, MAX_LINE).read_line(&mut line)?;
        if line.is_empty() {
            bail!("the server closed the connection");
        }
        Ok(serde_json::from_str(&line)?)
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixListener;
    use std::thread;

    use super::*;

    /// A server that acks the hello and answers one request with `answer`.
    fn serve(
        answer: &'static str,
    ) -> (
        tempfile::TempDir,
        std::path::PathBuf,
        thread::JoinHandle<String>,
    ) {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("s.sock");
        let listener = UnixListener::bind(&path).expect("bind");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut writer = stream;
            let mut hello = String::new();
            reader.read_line(&mut hello).expect("hello");
            writer
                .write_all(b"{\"ok\":true,\"server\":\"t\"}\n")
                .expect("ack");
            let mut request = String::new();
            reader.read_line(&mut request).expect("request");
            writer.write_all(answer.as_bytes()).expect("answer");
            hello + &request
        });
        (dir, path, handle)
    }

    #[test]
    fn calls_send_a_request_after_the_hello() {
        let (_dir, path, server) = serve("{\"id\":1,\"result\":{}}\n");
        let mut client = ApiClient::connect(&path).expect("connect");
        client
            .call("agent.report", &serde_json::json!({"pane": 1}))
            .expect("call");
        let seen = server.join().expect("server");
        assert!(
            seen.starts_with("{\"monarch\":1,\"proto\":\"json\""),
            "{seen}"
        );
        assert!(seen.contains("\"method\":\"agent.report\""), "{seen}");
    }

    #[test]
    fn errors_and_missing_servers_fail() {
        let (_dir, path, _server) =
            serve("{\"id\":1,\"error\":{\"code\":-32001,\"message\":\"no pane p9\"}}\n");
        let mut client = ApiClient::connect(&path).expect("connect");
        let err = client.call("agent.report", &()).expect_err("refused");
        assert!(err.to_string().contains("no pane p9"), "{err}");
        assert!(ApiClient::connect(Path::new("/nonexistent/monarch.sock")).is_err());
    }
}
