use git_bonsai::layout::build_scene;
use git_bonsai::render::{ascii, svg};
use git_bonsai::state::TreeState;
use git_bonsai::Theme;

fn reference() -> TreeState {
    serde_json::from_str(include_str!("fixtures/reference_state.json")).unwrap()
}

#[test]
fn reference_state_ascii() {
    insta::assert_snapshot!(
        "reference_state_ascii",
        ascii::print(&build_scene(&reference()).grid)
    );
}

#[test]
fn reference_state_svg_dark() {
    let svg = svg::render(
        &build_scene(&reference()),
        &svg::SvgOptions {
            theme: Theme::Dark,
            animate: true,
        },
    );
    assert!(svg.len() < 120 * 1024);
    insta::assert_snapshot!("reference_state_dark", svg);
}
