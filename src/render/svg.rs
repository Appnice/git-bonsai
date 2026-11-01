//! SVG output: hand-written string, only rects and text, colours grouped in CSS classes.
//!
//! Geometry follows board 07: 600 px wide, 40 px padding, cells 12 px with a 2 px gap and `rx=3`,
//! header above the grid, a two-column legend below. Text uses the system monospace stack (web
//! fonts do not load inside an `<img>` on github.com).

use super::{LegendItem, Phase, Scene};
use crate::grid::{GRID_H, GRID_W};
use crate::palette::{cell_color, fixed, Rgb, Theme, ThemeColors};
use std::fmt::Write as _;

/// Image width in px.
pub const WIDTH: u32 = 600;
/// Image height in px.
pub const HEIGHT: u32 = 672;

const PAD: u32 = 40;
const CELL: u32 = 12;
const STEP: u32 = 14; // cell + gap
const GRID_PX_W: u32 = GRID_W as u32 * STEP - 2;
const GRID_X: u32 = (WIDTH - GRID_PX_W) / 2;
const GRID_Y: u32 = 104;
const LEGEND_Y: u32 = GRID_Y + GRID_H as u32 * STEP - 2 + 20;
const LEGEND_COL_GAP: u32 = 24;
const LEGEND_ROW: u32 = 22;
const FONT: &str = "ui-monospace,SFMono-Regular,Menlo,Consolas,'Liberation Mono',monospace";

// Layout invariants, checked at compile time.
const _: () = assert!(GRID_PX_W == 502 && GRID_X == 49);
const _: () = assert!(LEGEND_Y + LEGEND_ROW + 12 <= HEIGHT - PAD);

/// Render options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SvgOptions {
    pub theme: Theme,
    /// CSS growth animation (phase 5; ignored for now).
    pub animate: bool,
}

/// Colours in first-use order, each one a CSS class `c<i>`; deterministic, no hashing.
#[derive(Default)]
struct Classes(Vec<Rgb>);

impl Classes {
    fn of(&mut self, c: Rgb) -> usize {
        match self.0.iter().position(|x| *x == c) {
            Some(i) => i,
            None => {
                self.0.push(c);
                self.0.len() - 1
            }
        }
    }
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// Width of monospace text at `size` px, in px (glyphs are 0.6 em wide).
fn text_w(s: &str, size: u32) -> u32 {
    (s.chars().count() as u32 * size * 3).div_ceil(5)
}

/// Renders the scene to a complete SVG document. Same scene and options give the same bytes.
pub fn render(scene: &Scene, opts: &SvgOptions) -> String {
    let t: ThemeColors = opts.theme.colors();
    let mut cls = Classes::default();
    let mut body = String::with_capacity(96 * 1024);

    // Header.
    let h = &scene.header;
    let _ = write!(
        body,
        r#"<text x="{PAD}" y="62" class="fg" font-size="22">{}</text>"#,
        esc(&h.login)
    );
    let _ = write!(
        body,
        r#"<text x="{PAD}" y="82" class="mu" font-size="12">{} commits · age {}y</text>"#,
        h.commits, h.age_years
    );
    let badge = h.phase.badge();
    let (badge_bg, badge_fg) = match h.phase {
        Phase::Bloom { .. } => (fixed::BLOSSOM, fixed::BADGE_FG),
        Phase::Growing | Phase::Sapling => (fixed::MOSS, fixed::BADGE_FG),
        Phase::Dormant { .. } => (t.badge_idle_bg, t.badge_idle_fg),
    };
    let bw = text_w(&badge, 12) + 20;
    let bx = WIDTH - PAD - bw;
    let _ = write!(
        body,
        r#"<rect x="{bx}" y="58" width="{bw}" height="26" rx="4" fill="{badge_bg}"/><text x="{}" y="75" font-size="12" fill="{badge_fg}">{}</text>"#,
        bx + 10,
        esc(&badge)
    );

    // Grid: every cell is a rect, empty days too, so the empty days are the background.
    let _ = write!(body, r#"<g transform="translate({GRID_X} {GRID_Y})">"#);
    for (x, y, role) in scene.grid.iter() {
        let c = cls.of(cell_color(role, &t, &scene.langs));
        let _ = write!(
            body,
            r#"<rect x="{}" y="{}" width="{CELL}" height="{CELL}" rx="3" class="c{c}"/>"#,
            x as u32 * STEP,
            y as u32 * STEP
        );
    }
    body.push_str("</g>");

    // Legend: two columns, two rows.
    let col_w = (WIDTH - 2 * PAD - LEGEND_COL_GAP) / 2;
    for (i, item) in scene.legend.iter().take(4).enumerate() {
        let x0 = PAD + (i as u32 % 2) * (col_w + LEGEND_COL_GAP);
        let y0 = LEGEND_Y + (i as u32 / 2) * LEGEND_ROW;
        let (swatches, label, muted) = match item {
            LegendItem::Branch {
                repo,
                language,
                commits,
                lang,
            } => {
                let p = scene.langs.get(usize::from(*lang));
                let sw: Vec<Rgb> = [1u8, 3, 4]
                    .iter()
                    .map(|l| p.map_or(t.empty, |p| p.level(*l)))
                    .collect();
                (sw, format!("{repo} · {language} · {commits}"), false)
            }
            LegendItem::Pruned { repo } => (vec![fixed::SCAR], format!("pruned · {repo}"), true),
        };
        let mut x = x0;
        for s in swatches {
            let c = cls.of(s);
            let _ = write!(
                body,
                r#"<rect x="{x}" y="{}" width="10" height="10" rx="2" class="c{c}"/>"#,
                y0 + 1
            );
            x += 16;
        }
        let _ = write!(
            body,
            r#"<text x="{}" y="{}" class="{}" font-size="12">{}</text>"#,
            x,
            y0 + 10,
            if muted { "mu" } else { "fg" },
            esc(&label)
        );
    }

    let mut out = String::with_capacity(body.len() + 2048);
    let label = format!(
        "Bonsai of {}: {} commits, age {} years, {}",
        h.login,
        h.commits,
        h.age_years,
        h.phase.badge()
    );
    let _ = write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {WIDTH} {HEIGHT}" width="{WIDTH}" height="{HEIGHT}" role="img" aria-label="{0}"><title>{0}</title>"#,
        esc(&label)
    );
    let _ = write!(
        out,
        "<style>text{{font-family:{FONT}}}.fg{{fill:{}}}.mu{{fill:{}}}",
        t.fg, t.muted
    );
    for (i, c) in cls.0.iter().enumerate() {
        let _ = write!(out, ".c{i}{{fill:{c}}}");
    }
    out.push_str("</style>");
    let _ = write!(
        out,
        r#"<rect width="{WIDTH}" height="{HEIGHT}" fill="{}"/>"#,
        t.bg
    );
    out.push_str(&body);
    out.push_str("</svg>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_markup() {
        assert_eq!(esc(r#"a<b>&"c'"#), "a&lt;b&gt;&amp;&quot;c&apos;");
        assert_eq!(esc("x\u{0}y"), "xy");
    }
}
