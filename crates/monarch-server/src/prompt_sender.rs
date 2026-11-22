//! Types in the prompts [`crate::PromptQueue`] says are due: an agent
//! asked while busy gets the prompt once it is back at its prompt.

use std::sync::Arc;
use std::time::Duration;

use monarch_proto::PaneSend;

use crate::ServerContext;

/// How often due prompts are looked for: soon after an agent finishes,
/// and costing one lock of the session when none are.
const POLL: Duration = Duration::from_millis(200);

/// The task that sends queued prompts.
#[derive(Debug, Clone, Copy, Default)]
pub struct PromptSender;

impl PromptSender {
    /// Runs until aborted with the server.
    pub async fn run(ctx: Arc<ServerContext>) {
        let mut tick = tokio::time::interval(POLL);
        loop {
            tick.tick().await;
            let due = ctx.lock().take_due_prompts();
            for (pane, text) in due {
                let ctx = Arc::clone(&ctx);
                // Typing waits between the text and Enter: never on a
                // worker that keystrokes queue behind.
                tokio::task::spawn_blocking(move || {
                    let send = PaneSend {
                        pane,
                        text,
                        enter: true,
                    };
                    if !ctx.type_text(&send) {
                        eprintln!("monarch: no pane {pane} for its queued prompt");
                    }
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::{AgentKind, AgentState};
    use monarch_proto::AgentReport;

    use super::*;
    use crate::PaneSpec;
    use crate::test_support::{FakePane, context};

    #[tokio::test]
    async fn a_queued_prompt_is_typed_once_the_agent_is_idle() {
        let ctx = Arc::new(context());
        let pane = ctx.insert_for_test(PaneSpec::shell_command("sh"), FakePane::shell());
        let working = AgentReport {
            state: Some(AgentState::Working),
            ..AgentReport::new(pane, AgentKind::Claude)
        };
        ctx.lock().report_agent(&working);
        assert_eq!(ctx.lock().queue_prompt(pane, "why?".into()), Some(0));
        assert!(ctx.lock().take_due_prompts().is_empty(), "it is working");
        let idle = AgentReport {
            state: Some(AgentState::Idle),
            ..working
        };
        ctx.lock().report_agent(&idle);
        assert_eq!(ctx.lock().take_due_prompts(), [(pane, "why?".to_owned())]);
        assert_eq!(
            ctx.lock()
                .queue_prompt(monarch_core::PaneId(99), "x".into()),
            None
        );
    }
}
