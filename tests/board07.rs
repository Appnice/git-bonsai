use git_bonsai::reference::{board_grid, board_scene, BOARD_MAP};
use git_bonsai::render::{ascii, svg};
use git_bonsai::{Phase, Theme};

fn render(phase: Phase, theme: Theme) -> String {
    svg::render(
        &board_scene(phase),
        &svg::SvgOptions {
            theme,
            animate: true,
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
    // Every grid cell is drawn once: empty days as `<use>`, the rest as 12 px rects.
    let uses = a.matches(r##"<use href="#q" x"##).count()
        - a.matches(r#"class="sh"#).count()
        - a.matches(r#"class="fr"#).count();
    let cells = a
        .matches(r#" width="12" height="12" rx="3" class="#)
        .count();
    assert_eq!(uses + cells, 36 * 34);
}

#[test]
fn svg_is_well_formed_xml_in_every_variant() {
    for phase in [
        Phase::Bloom { streak_days: 23 },
        Phase::Growing,
        Phase::Dormant { idle_days: 64 },
    ] {
        for theme in [Theme::Dark, Theme::Light] {
            for animate in [true, false] {
                let s = svg::render(&board_scene(phase), &svg::SvgOptions { theme, animate });
                roxmltree::Document::parse(&s)
                    .unwrap_or_else(|e| panic!("{phase:?} {theme:?} {animate}: {e}"));
                assert_eq!(s.contains("@keyframes"), animate);
                assert!(s.len() < 120 * 1024);
            }
        }
    }
}
