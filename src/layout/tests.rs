use super::*;
use crate::state::{Trunk, Vitals};
use proptest::prelude::*;

fn state_with(branches: Vec<Branch>, height: u32, girth: u32, bends: Vec<[i32; 2]>) -> TreeState {
    TreeState {
        kind: Default::default(),
        schema: 1,
        login: "x".into(),
        seed: "0123456789abcdef".into(),
        planted: time::macros::date!(2020 - 01 - 01),
        last_grown: Some(time::macros::date!(2026 - 06 - 01)),
        trunk: Trunk {
            height,
            girth,
            bends,
        },
        branches,
        year_cache: Default::default(),
        history: vec![],
        vitals: Vitals::default(),
    }
}

fn branch(i: usize, anchor: u32, side: Side, length: u32, pad: u32, pruned: bool) -> Branch {
    Branch {
        repo_id: format!("R_{i}"),
        name: format!("repo{i}"),
        lang: Some(["Rust", "Kotlin", "Python", "Go"][i % 4].into()),
        color: Some(["#dea584", "#a97bff", "#3572a5", "#00add8"][i % 4].into()),
        anchor,
        side,
        length,
        pad,
        commits_total: 100 * (i as u64 + 1),
        pruned: pruned.then_some(time::macros::date!(2024 - 01 - 01)),
    }
}

#[test]
fn pot_and_trunk_are_present() {
    let s = state_with(vec![], 12, 3, vec![]);
    let g = build_scene(&s).grid;
    assert_eq!(
        g.get(17, SOIL_Y as usize + 1),
        Some(Role::Pot(PotPart::Rim))
    );
    assert!(matches!(
        g.get(17, SOIL_Y as usize - 1),
        Some(Role::Trunk(_))
    ));
}

#[test]
fn deterministic() {
    let s: TreeState =
        serde_json::from_str(include_str!("../../tests/fixtures/reference_state.json")).unwrap();
    assert_eq!(build_scene(&s), build_scene(&s));
}

prop_compose! {
    fn arb_state()(
        height in 6u32..=24, girth in 2u32..=5,
        bends in proptest::collection::vec((0i32..24, -4i32..=4), 0..5),
        specs in proptest::collection::vec((1u32..24, any::<bool>(), 2u32..16, 0u32..=7, proptest::bool::weighted(0.15)), 0..8),
    ) -> TreeState {
        let branches = specs.into_iter().enumerate().map(|(i, (a, left, len, pad, pruned))| {
            branch(i, a, if left { Side::Left } else { Side::Right }, len, pad, pruned)
        }).collect();
        state_with(branches, height, girth, bends.into_iter().map(|(r, x)| [r, x]).collect())
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Pads never overlap, everything stays on the grid, the pot is intact.
    #[test]
    fn random_states_lay_out_cleanly(s in arb_state()) {
        let t = TrunkShape::new(&s);
        let apex = s.branches.iter().enumerate()
            .filter(|(_, b)| b.pruned.is_none() && b.pad > 0)
            .max_by_key(|(i, b)| (b.commits_total, std::cmp::Reverse(*i))).map(|(i, _)| i);
        let placed = place_branches(&s, &t, apex);
        let pads: Vec<Pad> = placed.iter().filter_map(|p| p.pad).collect();
        for (i, a) in pads.iter().enumerate() {
            prop_assert!(a.fits(), "{a:?}");
            for b in &pads[i + 1..] {
                prop_assert!(!a.overlaps(b), "{a:?} overlaps {b:?}");
            }
        }
        let scene = build_scene(&s);
        prop_assert_eq!(scene.grid.get(17, SOIL_Y as usize + 1), Some(Role::Pot(PotPart::Rim)));
        // Trunk is continuous from the soil to its top.
        for r in 0..t.height() {
            let y = TrunkShape::y(r) as usize;
            prop_assert!(scene.grid.rows().nth(y).unwrap().iter().any(|c| !matches!(c, Role::Empty)));
        }
    }
}
