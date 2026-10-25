//! A JSON API connection: one request per line, one answer per line.

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use monarch_proto::{AgentReport, Request, Response, RpcError};
use serde_json::Value;
use tokio::net::UnixStream;
use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec, LinesCodecError};

use crate::{ServerContext, ServerError};

/// Longest request line accepted, in bytes.
pub const MAX_REQUEST: usize = 64 * 1024;

/// Serves API requests (`agent.report`, …) until the caller hangs up.
#[derive(Debug, Clone, Copy, Default)]
pub struct ApiConnection;

impl ApiConnection {
    /// Answers requests on `stream` until it closes.
    ///
    /// # Errors
    /// The socket failed.
    pub async fn serve(ctx: Arc<ServerContext>, stream: UnixStream) -> Result<(), ServerError> {
        let io = |e| ServerError::io("api connection", e);
        let (rd, wr) = stream.into_split();
        let mut lines = FramedRead::new(rd, LinesCodec::new_with_max_length(MAX_REQUEST));
        let mut out = FramedWrite::new(wr, LinesCodec::new());
        while let Some(line) = lines.next().await {
            let response = match line {
                Ok(line) => Self::answer(&ctx, &line),
                Err(LinesCodecError::MaxLineLengthExceeded) => {
                    Response::err(None, RpcError::new(RpcError::PARSE, "request too long"))
                }
                Err(LinesCodecError::Io(e)) => return Err(io(e)),
            };
            let text = serde_json::to_string(&response).map_err(|e| io(e.into()))?;
            out.send(text).await.map_err(|e| match e {
                LinesCodecError::Io(e) => io(e),
                LinesCodecError::MaxLineLengthExceeded => io(std::io::Error::other(e)),
            })?;
        }
        Ok(())
    }

    /// The answer to one request line.
    pub fn answer(ctx: &ServerContext, line: &str) -> Response {
        match serde_json::from_str::<Request>(line) {
            Ok(request) => {
                let id = request.id;
                match dispatch(ctx, request) {
                    Ok(result) => Response::ok(id, result),
                    Err(error) => Response::err(Some(id), error),
                }
            }
            Err(e) => Response::err(None, RpcError::new(RpcError::PARSE, e.to_string())),
        }
    }
}

fn dispatch(ctx: &ServerContext, request: Request) -> Result<Value, RpcError> {
    match request.method.as_str() {
        AgentReport::METHOD => {
            let report: AgentReport = serde_json::from_value(request.params)
                .map_err(|e| RpcError::new(RpcError::INVALID_PARAMS, e.to_string()))?;
            if !ctx.lock().report_agent(&report) {
                let message = format!("no pane {} in this session", report.pane);
                return Err(RpcError::new(RpcError::NO_SUCH_PANE, message));
            }
            Ok(Value::Object(serde_json::Map::new()))
        }
        other => Err(RpcError::new(
            RpcError::METHOD_NOT_FOUND,
            format!("unknown method {other:?}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::{AgentKind, AgentState, PaneId};

    use super::*;
    use crate::PaneSpec;
    use crate::test_support::{FakePane, context};

    fn code(response: &Response) -> Option<i32> {
        response.error.as_ref().map(|e| e.code)
    }

    #[test]
    fn agent_reports_change_the_pane() {
        let ctx = context();
        let pane = ctx.insert_for_test(PaneSpec::shell_command("sh"), FakePane::shell());
        let report = AgentReport {
            state: Some(AgentState::Blocked),
            ..AgentReport::new(pane, AgentKind::Claude)
        };
        let line =
            serde_json::to_string(&Request::new(7, AgentReport::METHOD, &report).expect("request"))
                .expect("json");
        let response = ApiConnection::answer(&ctx, &line);
        assert_eq!(response, Response::ok(7, serde_json::json!({})));
        assert_eq!(ctx.lock().agent_status(pane).state, AgentState::Blocked);
    }

    #[test]
    fn bad_requests_get_errors() {
        let ctx = context();
        let report = AgentReport::new(PaneId(42), AgentKind::Claude);
        let missing = Request::new(1, AgentReport::METHOD, &report).expect("request");
        let line = serde_json::to_string(&missing).expect("json");
        assert_eq!(
            code(&ApiConnection::answer(&ctx, &line)),
            Some(RpcError::NO_SUCH_PANE)
        );
        let unknown = r#"{"id":2,"method":"pane.explode"}"#;
        assert_eq!(
            code(&ApiConnection::answer(&ctx, unknown)),
            Some(RpcError::METHOD_NOT_FOUND)
        );
        let params = r#"{"id":3,"method":"agent.report","params":{"pane":"x"}}"#;
        assert_eq!(
            code(&ApiConnection::answer(&ctx, params)),
            Some(RpcError::INVALID_PARAMS)
        );
        let garbage = ApiConnection::answer(&ctx, "{");
        assert_eq!((garbage.id, code(&garbage)), (None, Some(RpcError::PARSE)));
    }
}
