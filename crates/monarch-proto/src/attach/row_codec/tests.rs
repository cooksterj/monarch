//! Unit and property tests for [`RowCodec`].

use proptest::prelude::*;

use super::*;
use crate::{CellAttrs, TermColor};

fn styled(ch: char, fg: u8) -> Cell {
    Cell {
        style: CellStyle {
            fg: TermColor::Indexed(fg),
            ..CellStyle::default()
        },
        ..Cell::new(ch, 1)
    }
}

fn round_trip(cells: &[Cell]) -> (Vec<Run>, Vec<Cell>) {
    let mut table = StyleTable::default();
    let mut defs = Vec::new();
    let runs = RowCodec::encode(cells, &mut table, &mut defs);
    let mut cache = StyleCache::new();
    defs.into_iter().for_each(|d| cache.define(d));
    let cols = u16::try_from(cells.len()).expect("small row");
    (runs.clone(), RowCodec::decode(&runs, &cache, cols))
}

#[test]
fn same_style_cells_share_a_run_and_trailing_blanks_are_dropped() {
    let mut row: Vec<Cell> = "hi there".chars().map(|c| Cell::new(c, 1)).collect();
    row.extend(std::iter::repeat_n(Cell::default(), 10));
    let (runs, decoded) = round_trip(&row);
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].text, "hi there");
    assert_eq!(decoded, row);
}

#[test]
fn style_changes_split_runs() {
    let row = vec![
        styled('a', 1),
        styled('b', 1),
        styled('c', 2),
        Cell::new('d', 1),
    ];
    let (runs, decoded) = round_trip(&row);
    let texts: Vec<_> = runs.iter().map(|r| r.text.as_str()).collect();
    assert_eq!(texts, ["ab", "c", "d"]);
    assert_eq!(decoded, row);
}

#[test]
fn wide_cells_imply_their_spacer() {
    let row = vec![
        Cell::new('中', 2),
        Cell::spacer(),
        Cell::new('文', 2),
        Cell::spacer(),
        Cell::new('x', 1),
    ];
    let (runs, decoded) = round_trip(&row);
    assert_eq!(runs[0].text, "中文");
    assert_eq!(runs[0].cells, 2);
    assert_eq!(decoded, row);
}

#[test]
fn combining_marks_travel_as_clusters() {
    let mut accented = Cell::new('e', 1);
    accented.combining = vec!['\u{301}'];
    let row = vec![Cell::new('a', 1), accented, Cell::new('b', 1)];
    let (runs, decoded) = round_trip(&row);
    assert_eq!(runs.len(), 3);
    assert!(runs[1].is_cluster());
    assert_eq!(decoded, row);
}

#[test]
fn orphan_spacers_survive() {
    let row = vec![Cell::new('a', 1), Cell::spacer(), Cell::new('b', 1)];
    assert_eq!(round_trip(&row).1, row);
}

#[test]
fn decode_clamps_to_the_row_width() {
    let runs = vec![Run {
        style: crate::attach::StyleId::DEFAULT,
        width: 1,
        cells: 5,
        text: "abcde".into(),
    }];
    let row = RowCodec::decode(&runs, &StyleCache::new(), 3);
    assert_eq!(row.len(), 3);
    assert_eq!(row[2].ch, 'c');
}

fn arb_style() -> impl Strategy<Value = CellStyle> {
    (0u8..4, any::<bool>()).prop_map(|(fg, bold)| CellStyle {
        fg: if fg == 0 {
            TermColor::Default
        } else {
            TermColor::Indexed(fg)
        },
        attrs: if bold {
            CellAttrs::BOLD
        } else {
            CellAttrs::empty()
        },
        ..CellStyle::default()
    })
}

/// Rows the engine can produce: narrow cells (some with marks) and wide
/// cells followed by a spacer in the same style.
fn arb_row() -> impl Strategy<Value = Vec<Cell>> {
    let narrow = (
        prop::sample::select(vec!['a', 'b', ' ', '$']),
        prop::option::of(Just('\u{301}')),
        arb_style(),
    )
        .prop_map(|(ch, mark, style)| {
            vec![Cell {
                ch,
                combining: mark.into_iter().collect(),
                width: 1,
                style,
            }]
        });
    let wide = (prop::sample::select(vec!['中', '🦋']), arb_style()).prop_map(|(ch, style)| {
        vec![
            Cell {
                style,
                ..Cell::new(ch, 2)
            },
            Cell {
                style,
                ..Cell::spacer()
            },
        ]
    });
    prop::collection::vec(prop_oneof![3 => narrow, 1 => wide], 0..40)
        .prop_map(|cells| cells.into_iter().flatten().collect())
}

proptest! {
    #[test]
    fn encode_then_decode_is_lossless(row in arb_row()) {
        prop_assume!(!row.is_empty());
        let (_, decoded) = round_trip(&row);
        prop_assert_eq!(decoded, row);
    }
}
