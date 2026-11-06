//! `agent.read`: the last thing a pane's agent said.

use monarch_core::PaneId;
use monarch_proto::{AgentReadReply, ReadSource, StatusSource};

use super::ServerContext;

impl ServerContext {
    /// What `pane`'s agent last said: the final message of its last turn
    /// when its hooks report; else what its screen shows, since that is
    /// all there is; nothing for a hooked agent that hasn't answered yet.
    /// `None` when there is no such pane.
    #[must_use]
    pub fn read_agent(&self, pane: PaneId) -> Option<AgentReadReply> {
        let state = self.lock();
        let io = state.pane(pane)?;
        let status = state.agent_status(pane);
        if let Some(text) = state.agent_said(pane) {
            return Some(AgentReadReply {
                status,
                text: text.to_owned(),
                source: ReadSource::Message,
            });
        }
        if state.agent_source(pane) == StatusSource::Hooks {
            return Some(AgentReadReply::nothing(status));
        }
        let text = io.shared().lock().text();
        let text = text.trim_end();
        if text.is_empty() {
            return Some(AgentReadReply::nothing(status));
        }
        Some(AgentReadReply {
            status,
            text: text.to_owned(),
            source: ReadSource::Screen,
        })
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::{AgentKind, AgentState};
    use monarch_proto::AgentReport;

    use super::*;
    use crate::PaneSpec;
    use crate::test_support::{FakePane, context};

    /// A pane showing `> Fixed it.` and two blank rows.
    fn showing_fixed_it(ctx: &ServerContext) -> PaneId {
        let pane = ctx.insert_for_test(PaneSpec::default(), FakePane::shell());
        let state = ctx.lock();
        let mut term = state.pane(pane).expect("pane").shared().lock();
        term.feed(b"> Fixed it.\r\n\r\n", &mut Vec::new());
        pane
    }

    #[test]
    fn agents_without_hooks_give_their_screen() {
        let ctx = context();
        let gemini = showing_fixed_it(&ctx);
        let screen = ctx.read_agent(gemini).expect("pane");
        assert_eq!(
            (screen.text.as_str(), screen.source),
            ("> Fixed it.", ReadSource::Screen),
            "trailing blank rows dropped"
        );
        assert_eq!(ctx.read_agent(PaneId(99)), None);
    }

    #[test]
    fn hooked_agents_give_their_message_not_their_screen() {
        let ctx = context();
        let claude = showing_fixed_it(&ctx);
        let idle = AgentReport {
            state: Some(AgentState::Idle),
            ..AgentReport::new(claude, AgentKind::Claude)
        };
        ctx.lock().report_agent(&idle);
        let nothing = ctx.read_agent(claude).expect("pane");
        assert_eq!(
            (nothing.text.as_str(), nothing.source, nothing.status.state),
            ("", ReadSource::Nothing, AgentState::Idle),
            "hooks report, nothing said: not the screen"
        );
        let done = AgentReport {
            state: Some(AgentState::Done),
            activity: Some("Fixed it.\n\nTwo tests added.".into()),
            ..AgentReport::new(claude, AgentKind::Claude)
        };
        ctx.lock().report_agent(&done);
        let said = ctx.read_agent(claude).expect("pane");
        assert_eq!(
            (said.text.as_str(), said.source, said.status.state),
            (
                "Fixed it.\n\nTwo tests added.",
                ReadSource::Message,
                AgentState::Done
            )
        );
    }

    #[test]
    fn a_blank_screen_is_nothing() {
        let ctx = context();
        let shell = ctx.insert_for_test(PaneSpec::default(), FakePane::shell());
        let blank = ctx.read_agent(shell).expect("pane");
        assert_eq!(blank.source, ReadSource::Nothing);
    }
}
