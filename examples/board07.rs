//! Writes board 07 in its three variants to `target/board07-*.svg` for a look in a browser.
use git_bonsai::reference::board_scene;
use git_bonsai::render::svg;
use git_bonsai::{Phase, Theme};

fn main() -> std::io::Result<()> {
    for (name, phase, theme) in [
        ("dark-bloom", Phase::Bloom { streak_days: 23 }, Theme::Dark),
        ("light-growing", Phase::Growing, Theme::Light),
        (
            "dark-dormant",
            Phase::Dormant { idle_days: 64 },
            Theme::Dark,
        ),
    ] {
        let path = format!("target/board07-{name}.svg");
        std::fs::write(
            &path,
            svg::render(
                &board_scene(phase),
                &svg::SvgOptions {
                    theme,
                    animate: true,
                },
            ),
        )?;
        println!("{path}");
    }
    Ok(())
}
