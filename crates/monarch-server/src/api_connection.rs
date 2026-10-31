//! A JSON API connection: one request per line, one answer per line.

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use monarch_proto::{
    AgentReport, AgentWait, ExplainPanes, KillSession, NewWorktree, PaneSend, Request, Response,
    RpcError, SessionInfo, TabRename,
};
use serde::de::DeserializeOwned;
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
                // Methods may run git or start programs: keep them off
                // the async threads.
                Ok(line) => {
                    let ctx = Arc::clone(&ctx);
                    tokio::task::spawn_blocking(move || Self::answer(&ctx, &line))
                        .await
                        .map_err(|e| io(std::io::Error::other(e)))?
                }
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
        NewWorktree::METHOD => {
            let created = ctx
                .new_worktree(&params(request.params)?)
                .map_err(|e| RpcError::new(RpcError::FAILED, e.to_string()))?;
            json(created)
        }
        AgentReport::METHOD => report(ctx, &params(request.params)?),
        ExplainPanes::METHOD => {
            let ExplainPanes { pane } = params(request.params)?;
            let name = pane.map(|p| p.to_string()).unwrap_or_default();
            json(ctx.explain(pane).ok_or_else(|| no_such_pane(&name))?)
        }
        AgentWait::METHOD => {
            let wait: AgentWait = params(request.params)?;
            let reply = ctx.lock().agent_wait(&wait);
            json(reply.ok_or_else(|| no_such_pane(&wait.pane.to_string()))?)
        }
        PaneSend::METHOD => {
            let send: PaneSend = params(request.params)?;
            if !ctx.type_text(&send) {
                return Err(no_such_pane(&send.pane.to_string()));
            }
            Ok(Value::Object(serde_json::Map::new()))
        }
        TabRename::METHOD => rename_tab(ctx, &params(request.params)?),
        SessionInfo::METHOD => json(ctx.info()),
        KillSession::METHOD => {
            ctx.kill();
            Ok(Value::Object(serde_json::Map::new()))
        }
        other => Err(RpcError::new(
            RpcError::METHOD_NOT_FOUND,
            format!("unknown method {other:?}"),
        )),
    }
}

/// The error for a method naming a pane the session doesn't have.
/// Names a tab; clients redraw with the new name.
fn rename_tab(ctx: &ServerContext, rename: &TabRename) -> Result<Value, RpcError> {
    let renamed = ctx.lock().rename_tab_for(rename);
    ctx.wake();
    json(renamed.ok_or_else(|| no_such_tab(rename))?)
}

fn no_such_tab(rename: &TabRename) -> RpcError {
    let which = match (rename.tab, rename.pane) {
        (Some(number), _) => format!("tab {number}"),
        (None, Some(pane)) => format!("tab holding {pane}"),
        (None, None) => "tab".to_owned(),
    };
    RpcError::new(RpcError::FAILED, format!("no {which} in this session"))
}

fn no_such_pane(pane: &str) -> RpcError {
    RpcError::new(
        RpcError::NO_SUCH_PANE,
        format!("no pane {pane} in this session"),
    )
}

/// `agent.report`: applies a hook report to the pane it is about.
fn report(ctx: &ServerContext, report: &AgentReport) -> Result<Value, RpcError> {
    let Some(pane) = ctx.lock().report_agent(report) else {
        let message = match report.pane {
            Some(pane) => return Err(no_such_pane(&pane.to_string())),
            None => format!("no {} pane here for this report", report.agent.name()),
        };
        return Err(RpcError::new(RpcError::NO_SUCH_PANE, message));
    };
    ctx.record_session(pane, report);
    Ok(Value::Object(serde_json::Map::new()))
}

/// A method's result as JSON.
fn json(result: impl serde::Serialize) -> Result<Value, RpcError> {
    serde_json::to_value(result).map_err(|e| RpcError::new(RpcError::FAILED, e.to_string()))
}

fn params<T: DeserializeOwned>(value: Value) -> Result<T, RpcError> {
    serde_json::from_value(value)
        .map_err(|e| RpcError::new(RpcError::INVALID_PARAMS, e.to_string()))
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
    fn agent_panes_remember_the_reported_session() {
        let ctx = context();
        let codex = ctx.insert_for_test(PaneSpec::agent(AgentKind::Codex), FakePane::shell());
        let shell = ctx.insert_for_test(PaneSpec::shell_command("sh"), FakePane::shell());
        for pane in [codex, shell] {
            let report = AgentReport {
                session_id: Some("019a-thread".into()),
                ..AgentReport::new(pane, AgentKind::Codex)
            };
            let request = Request::new(1, AgentReport::METHOD, &report).expect("request");
            ApiConnection::answer(&ctx, &serde_json::to_string(&request).expect("json"));
        }
        let specs = ctx.specs();
        let session = |p| specs[&p].agent.as_ref().and_then(|a| a.session_id.clone());
        assert_eq!(session(codex).as_deref(), Some("019a-thread"));
        assert_eq!(
            session(shell),
            None,
            "a shell pane is not made an agent pane"
        );
    }

    #[test]
    fn agent_methods_name_missing_panes() {
        let ctx = context();
        let explain = r#"{"id":5,"method":"agent.explain","params":{"pane":42}}"#;
        assert_eq!(
            code(&ApiConnection::answer(&ctx, explain)),
            Some(RpcError::NO_SUCH_PANE)
        );
        let wait = r#"{"id":7,"method":"agent.wait","params":{"pane":42,"until":["idle"]}}"#;
        assert_eq!(
            code(&ApiConnection::answer(&ctx, wait)),
            Some(RpcError::NO_SUCH_PANE)
        );
        let send = r#"{"id":8,"method":"pane.send","params":{"pane":42,"text":"hi"}}"#;
        assert_eq!(
            code(&ApiConnection::answer(&ctx, send)),
            Some(RpcError::NO_SUCH_PANE)
        );
        let all = r#"{"id":6,"method":"agent.explain","params":{}}"#;
        let listed = ApiConnection::answer(&ctx, all).result;
        assert_eq!(listed, Some(serde_json::json!([])), "no panes yet");
    }

    #[test]
    fn tab_rename_names_the_tab_or_says_there_is_none() {
        let ctx = context();
        let none = r#"{"id":9,"method":"tab.rename","params":{"tab":3,"name":"api"}}"#;
        let failed = ApiConnection::answer(&ctx, none);
        assert_eq!(code(&failed), Some(RpcError::FAILED));
        assert!(failed.error.is_some_and(|e| e.message.contains("no tab 3")));
        let missing = r#"{"id":10,"method":"tab.rename","params":{"tab":1}}"#;
        assert_eq!(
            code(&ApiConnection::answer(&ctx, missing)),
            Some(RpcError::INVALID_PARAMS)
        );
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
        let outside = r#"{"id":4,"method":"worktree.create","params":{"dir":"/nonexistent-monarch","name":"x"}}"#;
        let failed = ApiConnection::answer(&ctx, outside);
        assert_eq!(code(&failed), Some(RpcError::FAILED));
        assert!(
            failed
                .error
                .is_some_and(|e| e.message.contains("not in a git repository"))
        );
        let garbage = ApiConnection::answer(&ctx, "{");
        assert_eq!((garbage.id, code(&garbage)), (None, Some(RpcError::PARSE)));
    }
}
