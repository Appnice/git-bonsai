//! SVG output: a hand-written string of rects and paths, colours grouped in CSS classes, motion in
//! CSS (no script), so it animates inside an `<img>` on github.com.
//!
//! Geometry follows board 07: 600 px wide, 40 px padding, cells 12 px with a 2 px gap and `rx=3`,
//! header above the grid, a two-column legend below. The header is set in the 5×7 pixel font; the
//! legend in the system monospace stack (web fonts do not load inside an `<img>`).
//!
//! Timeline of the animated image (plays once, then only the loops run):
//! 1. the empty days fade in as a diagonal wave, like the contribution graph loading;
//! 2. the pot pops in from the bottom row up;
//! 3. the tree grows out of the soil: every cell pops in by its distance from the trunk's base,
//!    so the trunk rises, branches reach out and pads unfold from their tips;
//! 4. the badge and the legend arrive; then the loops: cells grown today pulse, blossoms twinkle,
//!    a few leaves glint, autumn leaves fall step by step, a dormant tree snores.
//!
//! `prefers-reduced-motion: reduce` switches every animation off; the final frame is the
//! un-animated image, so nothing depends on the motion.

use super::{font, LegendItem, Phase, Scene};
use crate::grid::{Role, GRID_H, GRID_W};
use crate::palette::{cell_color, fixed, Rgb, Theme, ThemeColors};
use std::collections::{BTreeSet, VecDeque};
use std::fmt::Write as _;

/// Image width in px.
pub const WIDTH: u32 = 600;
/// Image height in px.
pub const HEIGHT: u32 = 672;

pub(crate) const PAD: u32 = 40;
pub(crate) const CELL: u32 = 12;
pub(crate) const STEP: u32 = 14; // cell + gap
const GRID_PX_W: u32 = GRID_W as u32 * STEP - 2;
pub(crate) const GRID_X: u32 = (WIDTH - GRID_PX_W) / 2;
pub(crate) const GRID_Y: u32 = 104;
pub(crate) const LEGEND_Y: u32 = GRID_Y + GRID_H as u32 * STEP - 2 + 20;
pub(crate) const LEGEND_COL_GAP: u32 = 24;
pub(crate) const LEGEND_ROW: u32 = 22;
const FONT: &str = "ui-monospace,SFMono-Regular,Menlo,Consolas,'Liberation Mono',monospace";

// Layout invariants, checked at compile time.
const _: () = assert!(GRID_PX_W == 502 && GRID_X == 49);
const _: () = assert!(LEGEND_Y + LEGEND_ROW + 12 <= HEIGHT - PAD);

// Timeline (ms).
const WAVE_STEP: u32 = 10;
const POT_START: u32 = 300;
const POT_STEP: u32 = 90;
const TREE_START: u32 = 900;
const TREE_SPAN: u32 = 2600;
const DELAY_QUANTUM: u32 = 20;

/// Render options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SvgOptions {
    pub theme: Theme,
    /// CSS animation (growth intro and loops). Off: a still image.
    pub animate: bool,
}

impl Default for SvgOptions {
    fn default() -> Self {
        SvgOptions {
            theme: Theme::Dark,
            animate: true,
        }
    }
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

/// Animation delays in use, each one a class `d<n>` = `n × 20 ms`.
#[derive(Default)]
struct Delays(BTreeSet<u32>);

impl Delays {
    /// The class for a delay, rounded to the quantum.
    fn class(&mut self, ms: u32) -> String {
        let n = (ms + DELAY_QUANTUM / 2) / DELAY_QUANTUM;
        self.0.insert(n);
        format!("d{n}")
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

/// A number with at most two decimals and no trailing zeros.
fn num(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

pub(crate) fn is_tree(r: Role) -> bool {
    matches!(
        r,
        Role::Trunk(_)
            | Role::Branch
            | Role::Leaf { .. }
            | Role::Blossom
            | Role::Scar
            | Role::Ghost
    )
}

pub(crate) fn is_pot(r: Role) -> bool {
    matches!(r, Role::Pot(_) | Role::Soil | Role::Moss)
}

/// Distance of every tree cell from the trunk's base (8-neighbour BFS over tree cells).
/// Cells the walk does not reach get `None`.
pub(crate) fn growth_distance(scene: &Scene) -> Vec<Option<u32>> {
    let g = &scene.grid;
    let mut dist = vec![None; GRID_W * GRID_H];
    let mut q = VecDeque::new();
    // Seeds: tree cells in the lowest row that has any.
    if let Some(base) = (0..GRID_H)
        .rev()
        .find(|y| (0..GRID_W).any(|x| g.get(x, *y).is_some_and(is_tree)))
    {
        for x in 0..GRID_W {
            if g.get(x, base).is_some_and(is_tree) {
                dist[base * GRID_W + x] = Some(0);
                q.push_back((x, base));
            }
        }
    }
    while let Some((x, y)) = q.pop_front() {
        let d = dist[y * GRID_W + x].unwrap_or(0);
        for (dx, dy) in [
            (0i32, -1i32),
            (-1, 0),
            (1, 0),
            (0, 1),
            (-1, -1),
            (1, -1),
            (-1, 1),
            (1, 1),
        ] {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if nx < 0 || ny < 0 || nx >= GRID_W as i32 || ny >= GRID_H as i32 {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            if dist[ny * GRID_W + nx].is_none() && g.get(nx, ny).is_some_and(is_tree) {
                dist[ny * GRID_W + nx] = Some(d + 1);
                q.push_back((nx, ny));
            }
        }
    }
    dist
}

/// Cheap stable hash of a cell, for picking glinting leaves and their phases.
fn cell_hash(x: usize, y: usize) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9e37_79b1) ^ (y as u32).wrapping_mul(0x85eb_ca77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^ (h >> 12)
}

/// Pixel-font text as one `<path>` per glyph; each lit pixel a square slightly smaller than its
/// pitch so the letters echo the cell grid.
fn pixel_text(
    text: &str,
    x: f64,
    y: f64,
    px: f64,
    attrs: &mut dyn FnMut(usize) -> String,
) -> String {
    let size = num(px * 0.86);
    let mut glyphs: Vec<String> = vec![String::new(); text.chars().count()];
    for (i, row, col, len) in font::runs(text) {
        for c in col..col + len {
            let _ = write!(
                glyphs[i],
                "M{} {}h{size}v{size}h-{size}z",
                num(x + c as f64 * px),
                num(y + row as f64 * px),
            );
        }
    }
    let mut out = String::new();
    for (i, d) in glyphs.iter().enumerate() {
        if !d.is_empty() {
            let _ = write!(out, r#"<path d="{d}"{}/>"#, attrs(i));
        }
    }
    out
}

/// Renders the scene to a complete SVG document. Same scene and options give the same bytes.
pub fn render(scene: &Scene, opts: &SvgOptions) -> String {
    let t: ThemeColors = opts.theme.colors();
    let anim = opts.animate;
    let mut cls = Classes::default();
    let mut delays = Delays::default();
    let mut body = String::with_capacity(112 * 1024);
    let mut extra_css = String::new();

    // Timeline from the tree's size.
    let dist = growth_distance(scene);
    let max_d = dist.iter().flatten().copied().max().unwrap_or(0);
    let tree_step = (TREE_SPAN / (max_d + 1)).clamp(12, 45);
    let tree_end = TREE_START + (max_d + 2) * tree_step;
    let intro_end = tree_end + 450;
    let pot_bottom = (0..GRID_H)
        .rev()
        .find(|y| (0..GRID_W).any(|x| scene.grid.get(x, *y).is_some_and(is_pot)))
        .unwrap_or(GRID_H - 1);

    // --- Header: login in the pixel font, stats line, phase badge ---
    let h = &scene.header;
    let badge = h.phase.badge();
    let badge_px = 2.0;
    let badge_w = font::width(&badge) as f64 * badge_px + 20.0;
    let badge_x = f64::from(WIDTH - PAD) - badge_w;
    let is_org = h.kind == crate::snapshot::AccountKind::Org;
    // Organizations carry an outlined ORG tag after the name.
    let tag_w = font::width("org") as f64 * 2.0 + 12.0;
    let title_room = badge_x - f64::from(PAD) - 16.0 - if is_org { tag_w + 10.0 } else { 0.0 };
    let title_w = font::width(&h.login).max(1) as f64;
    let title_px = ((title_room / title_w).clamp(1.0, 3.0) * 4.0).floor() / 4.0;
    let title_y = 40.0 + (3.0 - title_px) * 7.0;
    body.push_str(&pixel_text(
        &h.login,
        f64::from(PAD),
        title_y,
        title_px,
        &mut |i| {
            if anim {
                format!(r#" class="fg ty {}""#, delays.class(100 + i as u32 * 45))
            } else {
                r#" class="fg""#.to_string()
            }
        },
    ));
    if is_org {
        let tx = f64::from(PAD) + title_w * title_px + 10.0;
        let ty = title_y + 7.0 * title_px - 20.0;
        let anim = if anim {
            format!(
                r#" class="fi {}""#,
                delays.class(100 + font::width(&h.login) as u32 / 6 * 45)
            )
        } else {
            String::new()
        };
        let _ = write!(
            body,
            r#"<g{anim}><rect x="{}" y="{}" width="{}" height="20" rx="3" fill="none" stroke="{}" stroke-width="1.5"/>"#,
            num(tx + 0.75),
            num(ty + 0.75),
            num(tag_w - 1.5),
            t.muted
        );
        let tag_fill = format!(r#" class="mu""#);
        body.push_str(&pixel_text("org", tx + 6.0, ty + 3.0, 2.0, &mut |_| {
            tag_fill.clone()
        }));
        body.push_str("</g>");
    }
    let stats = h.stats();
    let stats_class = if anim {
        format!(r#" class="mu fi {}""#, delays.class(600))
    } else {
        r#" class="mu""#.into()
    };
    // The stats line shrinks to stay clear of the badge.
    let stats_room = badge_x - f64::from(PAD) - 12.0;
    let stats_px =
        ((stats_room / font::width(&stats).max(1) as f64).clamp(1.0, 2.0) * 4.0).floor() / 4.0;
    body.push_str(&pixel_text(
        &stats,
        f64::from(PAD),
        70.0 + (2.0 - stats_px) * 7.0,
        stats_px,
        &mut |_| stats_class.clone(),
    ));
    let (badge_bg, badge_fg) = match h.phase {
        Phase::Bloom { .. } => (fixed::BLOSSOM, fixed::BADGE_FG),
        Phase::Growing | Phase::Sapling => (fixed::MOSS, fixed::BADGE_FG),
        Phase::Dormant { .. } => (t.badge_idle_bg, t.badge_idle_fg),
    };
    let badge_anim = if anim {
        format!(
            r#" class="pop {}""#,
            delays.class(tree_end.saturating_sub(200))
        )
    } else {
        String::new()
    };
    let _ = write!(
        body,
        r#"<g{badge_anim}><rect x="{}" y="58" width="{}" height="26" rx="3" fill="{badge_fg_bg}"/>"#,
        num(badge_x),
        num(badge_w),
        badge_fg_bg = badge_bg
    );
    let fg_attr = format!(r#" fill="{badge_fg}""#);
    body.push_str(&pixel_text(
        &badge,
        badge_x + 10.0,
        64.0,
        badge_px,
        &mut |_| fg_attr.clone(),
    ));
    body.push_str("</g>");

    // --- Grid: every cell is a rect, empty days too, so the empty days are the background ---
    let _ = write!(body, r#"<g transform="translate({GRID_X} {GRID_Y})">"#);
    // Empty days grouped by wave diagonal (one class per group), the rest one `<use>` each.
    let mut empties: std::collections::BTreeMap<usize, String> = Default::default();
    let mut cells = String::with_capacity(48 * 1024);
    let mut blossoms = 0u32;
    let mut fallen = 0u32;
    for (x, y, role) in scene.grid.iter() {
        let c = cls.of(cell_color(role, &t, &scene.langs, scene.glaze));
        let (px, py) = (x as u32 * STEP, y as u32 * STEP);
        let mut class = format!("c{c}");
        let mut extra = String::new();
        if anim {
            let i = y * GRID_W + x;
            match role {
                Role::Empty => {}
                r if is_pot(r) => {
                    let d = POT_START + (pot_bottom.saturating_sub(y)) as u32 * POT_STEP;
                    let _ = write!(class, " k {}", delays.class(d));
                }
                Role::Fallen => {
                    // Falls from just under its pad to the soil, one cell per step, forever.
                    let below = (y + 1..GRID_H)
                        .take_while(|yy| {
                            scene
                                .grid
                                .get(x, *yy)
                                .is_some_and(|r| r == Role::Empty || r == Role::Fallen)
                        })
                        .count() as u32;
                    let down = below.clamp(1, 12);
                    let steps = down + 3;
                    let n = fallen;
                    fallen += 1;
                    let _ = write!(
                        extra_css,
                        "@keyframes f{n}{{0%{{transform:translateY(-{up}px);opacity:0}}12%{{opacity:1}}88%{{opacity:1}}100%{{transform:translateY({dn}px);opacity:0}}}}.lf{n}{{opacity:0;animation:f{n} {dur}ms steps({steps}) {intro_end}ms infinite}}",
                        up = 3 * STEP,
                        dn = down * STEP,
                        dur = steps * 520,
                    );
                    let _ = write!(class, " lf lf{n}");
                }
                Role::Blossom => {
                    let d = dist[i].map_or(tree_end, |d| TREE_START + d * tree_step);
                    let tw = intro_end + blossoms * 370;
                    blossoms += 1;
                    class.push_str(" bl");
                    extra = format!(r#" style="animation-delay:{d}ms,{tw}ms""#);
                }
                _ => {
                    let d = dist[i].map_or(tree_end, |d| TREE_START + d * tree_step);
                    let _ = write!(class, " k {}", delays.class(d));
                }
            }
        }
        if role == Role::Empty {
            let key = if anim { (x + y) / 2 } else { 0 };
            let group = empties.entry(key).or_insert_with(|| {
                if anim {
                    format!(
                        r#"<g class="{class} e {}">"#,
                        delays.class((key * 2) as u32 * WAVE_STEP)
                    )
                } else {
                    format!(r#"<g class="{class}">"#)
                }
            });
            let _ = write!(group, r##"<use href="#q" x="{px}" y="{py}"/>"##);
            continue;
        }
        let _ = write!(
            cells,
            // A real rect: WebKit scales a `<use>` with `transform-box: fill-box` around the wrong
            // origin, which would fling popping cells across the grid in Safari.
            r#"<rect x="{px}" y="{py}" width="{CELL}" height="{CELL}" rx="3" class="{class}"{extra}/>"#
        );
    }
    for g in empties.values() {
        body.push_str(g);
        body.push_str("</g>");
    }
    body.push_str(&cells);

    if anim {
        // Glints over some leaves and pulses over today's growth: white overlays, transparent
        // until their loop runs.
        for (x, y, role) in scene.grid.iter() {
            if matches!(role, Role::Leaf { .. }) && cell_hash(x, y) % 11 == 0 {
                let phase = intro_end + cell_hash(y, x) % 9000;
                let _ = write!(
                    body,
                    r##"<use href="#q" x="{}" y="{}" class="sh" style="animation-delay:{phase}ms"/>"##,
                    x as u32 * STEP,
                    y as u32 * STEP
                );
            }
        }
        for (x, y) in &scene.fresh {
            let _ = write!(
                body,
                r##"<use href="#q" x="{}" y="{}" class="fr {}"/>"##,
                *x as u32 * STEP,
                *y as u32 * STEP,
                delays.class(intro_end)
            );
        }
        // A dormant tree snores: three Zs drift up from its crown.
        if matches!(h.phase, Phase::Dormant { .. }) {
            // From just right of the crown's top row.
            let crown = scene
                .grid
                .iter()
                .find(|(_, _, r)| matches!(r, Role::Ghost | Role::Leaf { .. }))
                .map(|(_, y, _)| y);
            if let Some((zx, zy)) = crown.and_then(|cy| {
                scene
                    .grid
                    .iter()
                    .filter(|(_, y, r)| *y == cy && matches!(r, Role::Ghost | Role::Leaf { .. }))
                    .map(|(x, _, _)| x)
                    .max()
                    .map(|x| (x as f64 * f64::from(STEP), cy as f64 * f64::from(STEP)))
            }) {
                for k in 0..3u32 {
                    let px = 1.5 + f64::from(k) * 0.5;
                    let attrs = format!(
                        r#" class="zz mu" style="animation-delay:{}ms""#,
                        intro_end + k * 1500
                    );
                    body.push_str(&pixel_text(
                        "z",
                        zx + 16.0 + f64::from(k) * 10.0,
                        zy + 4.0 - f64::from(k) * 8.0,
                        px,
                        &mut |_| attrs.clone(),
                    ));
                }
            }
        }
    }
    body.push_str("</g>");

    // --- Legend: two columns, two rows ---
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
                    .map(|l| p.map_or(t.empty, |p| crate::palette::legible(p.level(*l), t.empty)))
                    .collect();
                (sw, format!("{repo} · {language} · {commits}"), false)
            }
            LegendItem::Pruned { repo } => (vec![fixed::SCAR], format!("pruned · {repo}"), true),
        };
        if anim {
            let _ = write!(
                body,
                r#"<g class="fi {}">"#,
                delays.class(tree_end.saturating_sub(300) + i as u32 * 80)
            );
        } else {
            body.push_str("<g>");
        }
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
            r#"<text x="{}" y="{}" class="{}" font-size="12">{}</text></g>"#,
            x,
            y0 + 10,
            if muted { "mu" } else { "fg" },
            esc(&label)
        );
    }

    // --- Document ---
    let mut out = String::with_capacity(body.len() + 4096);
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
    if anim {
        out.push_str(concat!(
            "@keyframes fi{from{opacity:0}}",
            "@keyframes pop{0%{transform:scale(0)}70%{transform:scale(1.18)}100%{transform:scale(1)}}",
            "@keyframes tw{0%,100%{opacity:1}50%{opacity:.5}}",
            "@keyframes pu{0%,100%{opacity:0}50%{opacity:.42}}",
            "@keyframes sh{0%,90%,100%{opacity:0}95%{opacity:.28}}",
            "@keyframes zz{0%{transform:translate(0,0);opacity:0}20%{opacity:1}100%{transform:translate(10px,-30px);opacity:0}}",
            ".e,.fi{animation:fi .45s ease-out both}",
            ".k,.pop,.bl{transform-box:fill-box;transform-origin:center}",
            ".k,.pop{animation:pop .42s cubic-bezier(.3,1.5,.5,1) both}",
            ".bl{animation:pop .42s cubic-bezier(.3,1.5,.5,1) both,tw 3.1s ease-in-out infinite}",
            ".ty{animation:fi .2s steps(2) both}",
            ".sh{fill:#fff;opacity:0;animation:sh 9s linear infinite}",
            ".fr{fill:#fff;opacity:0;animation:pu 2.4s ease-in-out infinite}",
            ".zz{opacity:0;animation:zz 4.5s ease-out infinite}",
        ));
        for n in &delays.0 {
            let _ = write!(out, ".d{n}{{animation-delay:{}ms}}", n * DELAY_QUANTUM);
        }
        out.push_str(&extra_css);
        out.push_str("@media (prefers-reduced-motion:reduce){*{animation:none!important}.sh,.fr,.zz{display:none}.lf{opacity:1}}");
    }
    out.push_str("</style>");
    let _ = write!(
        out,
        r#"<defs><rect id="q" width="{CELL}" height="{CELL}" rx="3"/></defs>"#
    );
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

    #[test]
    fn numbers_are_short() {
        assert_eq!(num(3.0), "3");
        assert_eq!(num(2.58), "2.58");
        assert_eq!(num(2.5), "2.5");
        assert_eq!(num(-0.001), "0");
    }
}
