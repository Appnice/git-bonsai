//! The hand-designed tree of board "07 Commit pixels": the first golden test and the visual target
//! every procedural tree is compared against.

use crate::grid::{Grid, Role};
use crate::palette::BOARD_LANGS;
use crate::render::{ascii, Header, LegendItem, Phase, Scene};

/// Board 07's character map, in the board's own notation (trunk shadow written `s`).
pub const BOARD_MAP: &str = include_str!("board07.txt");

/// Board 07 as a grid.
pub fn board_grid() -> Grid {
    // The map is a compile-time constant covered by tests; a parse failure is a build bug.
    ascii::parse(BOARD_MAP).unwrap_or_default()
}

/// Board 07's scene in the given phase. `Growing` turns blossoms into level-2 leaves of the top
/// pad's language, `Dormant` turns leaves and blossoms into ghosts, as the board's variants do.
pub fn board_scene(phase: Phase) -> Scene {
    let mut grid = board_grid();
    match phase {
        Phase::Dormant { .. } => grid.make_dormant(),
        Phase::Growing | Phase::Sapling => {
            let blossoms: Vec<(usize, usize)> = grid
                .iter()
                .filter(|(_, _, r)| *r == Role::Blossom)
                .map(|(x, y, _)| (x, y))
                .collect();
            for (x, y) in blossoms {
                grid.set(x, y, Role::Leaf { lang: 0, level: 2 });
            }
        }
        Phase::Bloom { .. } => {}
    }
    Scene {
        grid,
        langs: BOARD_LANGS.to_vec(),
        header: Header {
            login: "mira-codes".into(),
            commits: 4812,
            age_years: 10,
            phase,
        },
        legend: vec![
            LegendItem::Branch {
                repo: "lumen".into(),
                language: "kotlin".into(),
                commits: 2108,
                lang: 0,
            },
            LegendItem::Branch {
                repo: "ferrite".into(),
                language: "rust".into(),
                commits: 1204,
                lang: 1,
            },
            LegendItem::Branch {
                repo: "glyphs".into(),
                language: "python".into(),
                commits: 640,
                lang: 2,
            },
            LegendItem::Pruned {
                repo: "old-android-lib".into(),
            },
        ],
    }
}
