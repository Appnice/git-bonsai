//! Renders state fixtures to `target/gallery/<name>-<theme>.svg`.
use git_bonsai::render::svg::{render, SvgOptions};
use git_bonsai::state::TreeState;
use git_bonsai::{layout, Theme};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all("target/gallery")?;
    let mut paths: Vec<_> = std::fs::read_dir("tests/fixtures")?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().is_some_and(|e| e == "json") && p.to_string_lossy().contains("state")
        })
        .collect();
    paths.sort();
    for p in paths {
        let state: TreeState = serde_json::from_str(&std::fs::read_to_string(&p)?)?;
        let scene = layout::build_scene(&state);
        let stem = p
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        for theme in [Theme::Dark, Theme::Light] {
            let out = format!("target/gallery/{stem}-{}.svg", theme.name());
            std::fs::write(
                &out,
                render(
                    &scene,
                    &SvgOptions {
                        theme,
                        animate: true,
                    },
                ),
            )?;
            println!("{out}");
        }
        #[cfg(feature = "gif")]
        {
            let out = format!("target/gallery/{stem}.gif");
            std::fs::write(&out, git_bonsai::render::gif::render(&scene, Theme::Dark)?)?;
            println!("{out}");
        }
    }
    Ok(())
}
