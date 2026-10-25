//! Spike 6: a client that reattaches sees exactly what a client that stayed
//! attached sees: the snapshot a new client gets and the deltas an attached
//! one applied produce the same screen, cell for cell, and the same frame.

use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use monarch_core::PaneId;
use monarch_proto::{ClientMsg, ScreenSize, ServerMsg};
use monarch_server::{PaneSpec, Server, ServerConfig};
use monarch_tui::{Applied, AttachState, AttachView, ClientEvent, ServerLink, StatusLine};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;

const PANE: PaneId = PaneId(1);
const HOST: ScreenSize = ScreenSize { cols: 60, rows: 16 };

/// Colours, attributes, wide and combining characters, cursor addressing,
/// a title and scrolling, then output that keeps going while the second
/// client attaches.
const SCRIPT: &str = r#"
printf '\033[1;31mred\033[0m \033[4;38;5;33munder\033[0m \346\274\242\345\255\227 e\314\201 \033[7mrev\033[0m\r\n'
seq 1 30
printf '\033[5;10H\033[42;30m XY \033[0m\033]2;reattach\007'
sleep 1
i=0; while [ $i -lt 40 ]; do printf "tick $i \033[3%dm*\033[0m\r\n" $((i % 8)); i=$((i+1)); sleep 0.02; done
printf 'DONE'
sleep 30
"#;

/// One attached client: its model and the messages feeding it.
struct Client {
    link: ServerLink,
    events: Receiver<ClientEvent>,
    state: AttachState,
}

impl Client {
    fn attach(socket: &Path) -> Self {
        let deadline = Instant::now() + Duration::from_secs(10);
        let (link, reader) = loop {
            match ServerLink::connect(socket) {
                Ok(pair) => break pair,
                Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
                Err(e) => panic!("connect: {e}"),
            }
        };
        let (tx, events) = mpsc::channel();
        ServerLink::spawn_reader(reader, tx);
        link.send(&ClientMsg::Resize(AttachView::pane_area(HOST, 0)))
            .expect("resize");
        Self {
            link,
            events,
            state: AttachState::new(),
        }
    }

    /// Applies messages for `period`.
    fn pump(&mut self, period: Duration) {
        let end = Instant::now() + period;
        while let Some(left) = end.checked_duration_since(Instant::now()) {
            match self.events.recv_timeout(left) {
                Ok(ClientEvent::Server(msg)) => self.apply(msg),
                Ok(_) => {}
                Err(_) => return,
            }
        }
    }

    fn apply(&mut self, msg: ServerMsg) {
        let mut applied = Applied::default();
        self.state.apply(msg, &mut applied);
        for pane in applied.resync {
            self.link.send(&ClientMsg::Resync(pane)).expect("resync");
        }
    }

    fn shows(&self, text: &str) -> bool {
        self.state.screen(PANE).is_some_and(|screen| {
            screen.rows.iter().any(|row| {
                row.iter()
                    .map(|c| c.text())
                    .collect::<String>()
                    .contains(text)
            })
        })
    }

    fn wait_for(&mut self, text: &str) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while !self.shows(text) {
            assert!(Instant::now() < deadline, "never showed {text:?}");
            self.pump(Duration::from_millis(20));
        }
    }

    /// The frame this client would draw.
    fn frame(&self) -> Buffer {
        let mut term = Terminal::new(TestBackend::new(HOST.cols, HOST.rows)).expect("terminal");
        let (tabs, active_tab) = self.state.tab_labels();
        let view = AttachView {
            state: &self.state,
            status: StatusLine {
                session: "reattach",
                tabs: &tabs,
                active_tab,
                prefix: "ctrl+b",
                detach: None,
                pending: false,
                message: None,
                mouse: monarch_tui::MouseOwner::Monarch,
                chrome: monarch_tui::Chrome::default(),
            },
            sidebar: None,
            selection: None,
            matches: &[],
            copy_cursor: None,
            which_key: None,
            picker: None,
            review: None,
            form: None,
            help: None,
            menu: None,
        };
        term.draw(|frame| view.draw(frame)).expect("draw");
        term.backend().buffer().clone()
    }
}

#[test]
fn a_reattached_client_sees_exactly_what_an_attached_one_sees() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = dir.path().join("r.sock");
    let config = ServerConfig {
        session: "reattach".into(),
        socket: Some(socket.clone()),
        panes: vec![PaneSpec {
            argv: vec!["sh".into(), "-c".into(), SCRIPT.into()],
            ..PaneSpec::default()
        }],
        scrollback: 100,
        ..ServerConfig::default()
    };
    thread::spawn(move || Server::run(config));

    let mut stayed = Client::attach(&socket);
    stayed.wait_for("XY");
    let mut first = Client::attach(&socket);
    first.wait_for("XY");
    first.link.send(&ClientMsg::Detach).expect("detach");
    drop(first);

    stayed.wait_for("tick 5");
    let mut back = Client::attach(&socket);
    back.wait_for("tick");
    for client in [&mut stayed, &mut back] {
        client.wait_for("DONE");
    }
    // Let both drain whatever frames were still in flight.
    stayed.pump(Duration::from_millis(300));
    back.pump(Duration::from_millis(300));

    let (a, b) = (stayed.state.screen(PANE), back.state.screen(PANE));
    assert!(a.is_some(), "attached client has the pane");
    assert_eq!(a, b, "cells, styles, cursor and modes match");
    assert_eq!(stayed.state.focused_title(), Some("reattach"));
    assert_eq!(stayed.state.focused_title(), back.state.focused_title());
    assert_eq!(stayed.frame(), back.frame(), "the drawn frames match");
}
