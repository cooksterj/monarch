use monarch_detect::Manifest;

use super::*;
use crate::PaneSpec;
use crate::test_support::{FakePane, context};

/// A one-agent registry: `bot` is working while it shows `esc to stop`,
/// blocked on `allow?`, and keeps its state while `viewer` is open.
fn registry() -> ManifestRegistry {
    let manifest = Manifest::parse(
        r#"id = "bot"
[[rules]]
id = "working"
state = "working"
contains = ["esc to stop"]
[[rules]]
id = "ask"
state = "blocked"
priority = 5
contains = ["allow?"]
[[rules]]
id = "viewer"
state = "unknown"
skip_state_update = true
priority = 9
contains = ["viewer"]"#,
    )
    .expect("valid");
    ManifestRegistry::from(vec![manifest])
}

fn show(ctx: &ServerContext, pane: PaneId, text: &str) {
    let lock = ctx.lock();
    let shared = lock.pane(pane).expect("pane").shared();
    shared
        .lock()
        .feed(format!("\x1b[2J\x1b[H{text}").as_bytes(), &mut Vec::new());
    shared.mark_dirty();
}

fn state(ctx: &ServerContext, pane: PaneId) -> AgentState {
    ctx.lock().agent_status(pane).state
}

#[test]
fn screens_drive_the_state_and_idle_settles() {
    let ctx = context();
    let pane = ctx.insert_for_test(PaneSpec::shell_command("bot"), FakePane::shell());
    ctx.lock().set_command(pane, Some("bot".into()));
    let mut watcher = ScreenWatcher::new(registry());
    let t0 = Instant::now();
    show(&ctx, pane, "thinking · esc to stop");
    watcher.check(&ctx, t0);
    assert_eq!(state(&ctx, pane), AgentState::Working);
    show(&ctx, pane, "> ");
    watcher.check(&ctx, t0);
    assert_eq!(
        state(&ctx, pane),
        AgentState::Working,
        "idle not settled yet"
    );
    watcher.check(&ctx, t0 + IDLE_SETTLE);
    assert_eq!(state(&ctx, pane), AgentState::Done, "unwatched turn ended");
    show(&ctx, pane, "viewer");
    watcher.check(&ctx, t0 + IDLE_SETTLE * 2);
    assert_eq!(state(&ctx, pane), AgentState::Done, "held");
    show(&ctx, pane, "allow? esc to stop");
    watcher.check(&ctx, t0 + IDLE_SETTLE * 3);
    assert_eq!(state(&ctx, pane), AgentState::Blocked);
}

#[test]
fn leaving_the_agent_ends_its_status() {
    let ctx = context();
    let pane = ctx.insert_for_test(PaneSpec::shell_command("bot"), FakePane::shell());
    ctx.lock().set_command(pane, Some("bot".into()));
    let mut watcher = ScreenWatcher::new(registry());
    show(&ctx, pane, "esc to stop");
    watcher.check(&ctx, Instant::now());
    assert_eq!(state(&ctx, pane), AgentState::Working);
    ctx.lock().set_command(pane, Some("zsh".into()));
    watcher.check(&ctx, Instant::now());
    assert_eq!(state(&ctx, pane), AgentState::Unknown, "a shell again");
}
