//! Turns shadow-grid frames into attach messages for one client.

use monarch_proto::{Delta, RowCodec, RowDelta, ServerMsg, Snapshot, StyleDef, StyleTable};
use monarch_term::ShadowGrid;

use crate::FrameHeader;

/// Per-connection encoder: owns the connection's style table, so style ids
/// mean the same thing to the encoder and to the client's cache.
#[derive(Debug, Clone, Default)]
pub struct FrameEncoder {
    styles: StyleTable,
    reset_pending: bool,
}

impl FrameEncoder {
    /// An encoder for a fresh connection (the client's cache is empty).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets all styles; the next message tells the client to do the
    /// same. Used when the client must resync.
    pub fn reset(&mut self) {
        self.styles.clear();
        self.reset_pending = true;
    }

    /// The whole pane: style definitions, then a `Snapshot`.
    pub fn snapshot(&mut self, header: FrameHeader, shadow: &ShadowGrid) -> Vec<ServerMsg> {
        let mut out = self.begin();
        let mut defs = Vec::new();
        let rows = shadow
            .rows()
            .iter()
            .map(|row| RowCodec::encode(row, &mut self.styles, &mut defs))
            .collect();
        finish(&mut out, defs);
        out.push(ServerMsg::Snapshot(Snapshot {
            pane: header.pane,
            generation: header.generation,
            seq: header.seq,
            size: shadow.size(),
            rows,
            state: shadow.state(),
        }));
        out
    }

    /// The given rows: style definitions, then a `Delta`.
    pub fn delta(
        &mut self,
        header: FrameHeader,
        shadow: &ShadowGrid,
        lines: &[u16],
    ) -> Vec<ServerMsg> {
        let mut out = self.begin();
        let mut defs = Vec::new();
        let rows = lines
            .iter()
            .filter_map(|&line| {
                let cells = shadow.row(line)?;
                let runs = RowCodec::encode(cells, &mut self.styles, &mut defs);
                Some(RowDelta { line, runs })
            })
            .collect();
        finish(&mut out, defs);
        out.push(ServerMsg::Delta(Delta {
            pane: header.pane,
            generation: header.generation,
            seq: header.seq,
            rows,
            state: shadow.state(),
        }));
        out
    }

    fn begin(&mut self) -> Vec<ServerMsg> {
        let restarted = self.styles.begin_message();
        if restarted || std::mem::take(&mut self.reset_pending) {
            vec![ServerMsg::ResetStyles]
        } else {
            Vec::new()
        }
    }
}

fn finish(out: &mut Vec<ServerMsg>, defs: Vec<StyleDef>) {
    out.extend(defs.into_iter().map(ServerMsg::DefineStyle));
}

#[cfg(test)]
mod tests;
