#![cfg(feature = "gif")]
use git_bonsai::reference::board_scene;
use git_bonsai::render::gif;
use git_bonsai::{Phase, Theme};

#[test]
fn gif_is_small_deterministic_and_complete() {
    let scene = board_scene(Phase::Bloom { streak_days: 23 });
    let a = gif::render(&scene, Theme::Dark).unwrap();
    assert_eq!(a, gif::render(&scene, Theme::Dark).unwrap());
    assert!(a.len() < 1024 * 1024, "{} bytes", a.len());
    let mut dec = ::gif::DecodeOptions::new();
    dec.set_color_output(::gif::ColorOutput::Indexed);
    let mut d = dec.read_info(a.as_slice()).unwrap();
    let pal = d.global_palette().unwrap().len() / 3;
    assert!(pal <= 64, "{pal} colours");
    let mut n = 0;
    while d.read_next_frame().unwrap().is_some() {
        n += 1;
    }
    assert_eq!(n, gif::FRAMES + 1);
}
