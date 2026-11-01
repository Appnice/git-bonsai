use git_bonsai::reference::{board_grid, board_scene, BOARD_MAP};
use git_bonsai::render::{ascii, svg};
use git_bonsai::{Phase, Theme};

fn render(phase: Phase, theme: Theme) -> String {
    svg::render(
        &board_scene(phase),
        &svg::SvgOptions {
            theme,
            animate: false,
        },
    )
}

#[test]
fn board_map_parses_and_prints_canonically() {
    let printed = ascii::print(&board_grid());
    // Canonical form: full-width rows, trunk shadow written `k` instead of the board's `s`.
    insta::assert_snapshot!("board07_canonical", printed);
    assert_eq!(ascii::parse(&printed).unwrap(), board_grid());
    let original: Vec<String> = BOARD_MAP.lines().map(|l| format!("{l:.<36}")).collect();
    let canon: Vec<&str> = printed.lines().collect();
    assert_eq!(original.len(), canon.len());
    for (o, c) in original.iter().zip(&canon) {
        for (a, b) in o.chars().zip(c.chars()) {
            assert!(a == b || (a == 's' && b == 'k'), "{o}\n{c}");
        }
    }
}

#[test]
fn board_svg_dark_bloom() {
    insta::assert_snapshot!(
        "board07_dark_bloom",
        render(Phase::Bloom { streak_days: 23 }, Theme::Dark)
    );
}

#[test]
fn board_svg_light_growing() {
    insta::assert_snapshot!(
        "board07_light_growing",
        render(Phase::Growing, Theme::Light)
    );
}

#[test]
fn board_svg_dark_dormant() {
    insta::assert_snapshot!(
        "board07_dark_dormant",
        render(Phase::Dormant { idle_days: 64 }, Theme::Dark)
    );
}

#[test]
fn svg_is_small_and_stable() {
    let a = render(Phase::Bloom { streak_days: 23 }, Theme::Dark);
    assert!(a.len() < 120 * 1024, "{} bytes", a.len());
    assert_eq!(a, render(Phase::Bloom { streak_days: 23 }, Theme::Dark));
    assert_eq!(a.matches("<rect").count(), 1 + 1 + 36 * 34 + 3 * 3 + 1);
}
