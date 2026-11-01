//! GIF export (feature `gif`) for places that do not show SVG: 24 growth frames plus a final
//! frame held 3 s, at 1× (12 px cells), palette ≤ 64 colours, no blossom twinkle.
//! The header and legend use the pixel font, so no font engine is needed.

use super::svg::{
    growth_distance, is_pot, CELL, GRID_X, GRID_Y, HEIGHT, LEGEND_COL_GAP, LEGEND_ROW, LEGEND_Y,
    PAD, STEP, WIDTH,
};
use super::{font, LegendItem, Phase, Scene};
use crate::grid::{Role, GRID_W};
use crate::palette::{cell_color, fixed, Rgb, Theme};

/// Growth frames before the held final frame.
pub const FRAMES: usize = 24;
const FRAME_DELAY_CS: u16 = 9;
const HOLD_CS: u16 = 300;

/// GIF encoding failed.
#[derive(Debug, thiserror::Error)]
pub enum GifError {
    #[error("gif encoding: {0}")]
    Encode(#[from] ::gif::EncodingError),
    #[error("more than 256 colours")]
    Palette,
}

struct Raster {
    px: Vec<u8>,
    palette: Vec<Rgb>,
}

impl Raster {
    fn new(bg: Rgb) -> Raster {
        Raster {
            px: vec![0; (WIDTH * HEIGHT) as usize],
            palette: vec![bg],
        }
    }

    fn index(&mut self, c: Rgb) -> u8 {
        match self.palette.iter().position(|p| *p == c) {
            Some(i) => i as u8,
            None => {
                self.palette.push(c);
                (self.palette.len() - 1).min(255) as u8
            }
        }
    }

    /// A filled rectangle; `round` cuts the three corner pixels of each corner (≈ rx 3).
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb, round: bool) {
        let i = self.index(c);
        for yy in 0..h {
            for xx in 0..w {
                if round {
                    let dx = xx.min(w - 1 - xx);
                    let dy = yy.min(h - 1 - yy);
                    if dx + dy < 2 {
                        continue;
                    }
                }
                let (px, py) = (x + xx, y + yy);
                if px >= 0 && py >= 0 && px < WIDTH as i32 && py < HEIGHT as i32 {
                    self.px[(py as u32 * WIDTH + px as u32) as usize] = i;
                }
            }
        }
    }

    fn text(&mut self, text: &str, x: i32, y: i32, scale: i32, c: Rgb) {
        let dot = if scale >= 3 { scale - 1 } else { scale };
        for (_, row, col, len) in font::runs(text) {
            for k in col..col + len {
                self.rect(
                    x + k as i32 * scale,
                    y + row as i32 * scale,
                    dot,
                    dot,
                    c,
                    false,
                );
            }
        }
    }
}

/// What a frame shows, as fractions of the intro (0 = nothing yet, 1 = all).
fn frame(scene: &Scene, theme: Theme, progress: f64, dist: &[Option<u32>], max_d: u32) -> Raster {
    let t = theme.colors();
    let mut r = Raster::new(t.bg);
    let h = &scene.header;

    // Header (always shown, like the typed title in the SVG).
    let badge = h.phase.badge();
    let badge_w = font::width(&badge) as i32 * 2 + 20;
    let badge_x = (WIDTH - PAD) as i32 - badge_w;
    let room = badge_x - PAD as i32 - 16;
    let scale = (room / font::width(&h.login).max(1) as i32).clamp(1, 3);
    r.text(&h.login, PAD as i32, 40 + (3 - scale) * 7, scale, t.fg);
    r.text(
        &format!("{} commits · {}y", h.commits, h.age_years),
        PAD as i32,
        70,
        2,
        t.muted,
    );

    let last = progress >= 1.0;
    let pot_rows: Vec<usize> = (0..crate::grid::GRID_H)
        .filter(|y| (0..GRID_W).any(|x| scene.grid.get(x, *y).is_some_and(is_pot)))
        .collect();
    let pot_n = pot_rows.len().max(1) as f64;
    for (x, y, role) in scene.grid.iter() {
        let shown = match role {
            Role::Empty => ((x + y) as f64 / 68.0) <= progress * 3.0,
            Role::Fallen => last,
            Role::Blossom => last,
            rl if is_pot(rl) => {
                let k = pot_rows.iter().rev().position(|py| *py == y).unwrap_or(0) as f64;
                (k + 1.0) / pot_n <= ((progress - 0.12) / 0.2).max(0.0)
            }
            _ => {
                let d = dist[y * GRID_W + x].unwrap_or(max_d) as f64;
                d <= (progress - 0.3) / 0.65 * f64::from(max_d.max(1))
            }
        };
        if !shown {
            continue;
        }
        let role = if role == Role::Blossom && !last {
            Role::Empty
        } else {
            role
        };
        r.rect(
            (GRID_X + x as u32 * STEP) as i32,
            (GRID_Y + y as u32 * STEP) as i32,
            CELL as i32,
            CELL as i32,
            cell_color(role, &t, &scene.langs),
            true,
        );
    }

    if last {
        let (bg, fg) = match h.phase {
            Phase::Bloom { .. } => (fixed::BLOSSOM, fixed::BADGE_FG),
            Phase::Growing | Phase::Sapling => (fixed::MOSS, fixed::BADGE_FG),
            Phase::Dormant { .. } => (t.badge_idle_bg, t.badge_idle_fg),
        };
        r.rect(badge_x, 58, badge_w, 26, bg, true);
        r.text(&badge, badge_x + 10, 64, 2, fg);
        let col_w = (WIDTH - 2 * PAD - LEGEND_COL_GAP) / 2;
        for (i, item) in scene.legend.iter().take(4).enumerate() {
            let x0 = (PAD + (i as u32 % 2) * (col_w + LEGEND_COL_GAP)) as i32;
            let y0 = (LEGEND_Y + (i as u32 / 2) * LEGEND_ROW) as i32;
            let (sw, label, muted): (Vec<Rgb>, String, bool) = match item {
                LegendItem::Branch {
                    repo,
                    language,
                    commits,
                    lang,
                } => {
                    let p = scene.langs.get(usize::from(*lang));
                    (
                        [1u8, 3, 4]
                            .iter()
                            .map(|l| {
                                p.map_or(t.empty, |p| crate::palette::legible(p.level(*l), t.empty))
                            })
                            .collect(),
                        format!("{repo} · {language} · {commits}"),
                        false,
                    )
                }
                LegendItem::Pruned { repo } => {
                    (vec![fixed::SCAR], format!("pruned · {repo}"), true)
                }
            };
            let mut x = x0;
            for c in sw {
                r.rect(x, y0 + 1, 10, 10, c, false);
                x += 16;
            }
            r.text(&label, x, y0 + 2, 1, if muted { t.muted } else { t.fg });
        }
    }
    r
}

/// Encodes the growth animation. Same scene and theme give the same bytes.
pub fn render(scene: &Scene, theme: Theme) -> Result<Vec<u8>, GifError> {
    let dist = growth_distance(scene);
    let max_d = dist.iter().flatten().copied().max().unwrap_or(0);
    let frames: Vec<Raster> = (0..=FRAMES)
        .map(|k| {
            let p = if k == FRAMES {
                1.0
            } else {
                k as f64 / FRAMES as f64
            };
            frame(scene, theme, p, &dist, max_d)
        })
        .collect();

    // One global palette: every colour of every frame (the last frame holds them all).
    let mut palette: Vec<Rgb> = Vec::new();
    for f in &frames {
        for c in &f.palette {
            if !palette.contains(c) {
                palette.push(*c);
            }
        }
    }
    if palette.len() > 256 {
        return Err(GifError::Palette);
    }
    let flat: Vec<u8> = palette.iter().flat_map(|c| [c.0, c.1, c.2]).collect();
    let mut out = Vec::new();
    {
        let mut enc = ::gif::Encoder::new(&mut out, WIDTH as u16, HEIGHT as u16, &flat)?;
        enc.set_repeat(::gif::Repeat::Infinite)?;
        let mut prev: Option<Vec<u8>> = None;
        for (k, f) in frames.iter().enumerate() {
            // Map the frame's local palette to the global one.
            let map: Vec<u8> = f
                .palette
                .iter()
                .map(|c| palette.iter().position(|p| p == c).unwrap_or(0) as u8)
                .collect();
            let px: Vec<u8> = f.px.iter().map(|i| map[usize::from(*i)]).collect();
            // Only the rectangle that changed since the previous frame.
            let (mut x0, mut y0, mut x1, mut y1) = (WIDTH, HEIGHT, 0u32, 0u32);
            match &prev {
                Some(p) => {
                    for (i, (a, b)) in px.iter().zip(p).enumerate() {
                        if a != b {
                            let (x, y) = (i as u32 % WIDTH, i as u32 / WIDTH);
                            x0 = x0.min(x);
                            y0 = y0.min(y);
                            x1 = x1.max(x);
                            y1 = y1.max(y);
                        }
                    }
                }
                None => (x0, y0, x1, y1) = (0, 0, WIDTH - 1, HEIGHT - 1),
            }
            if x1 < x0 {
                (x0, y0, x1, y1) = (0, 0, 0, 0);
            }
            let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
            let mut buf = Vec::with_capacity((w * h) as usize);
            for y in y0..=y1 {
                let row = (y * WIDTH) as usize;
                buf.extend_from_slice(&px[row + x0 as usize..=row + x1 as usize]);
            }
            let mut fr = ::gif::Frame {
                left: x0 as u16,
                top: y0 as u16,
                width: w as u16,
                height: h as u16,
                buffer: std::borrow::Cow::Owned(buf),
                delay: if k == FRAMES { HOLD_CS } else { FRAME_DELAY_CS },
                ..Default::default()
            };
            fr.dispose = ::gif::DisposalMethod::Keep;
            enc.write_frame(&fr)?;
            prev = Some(px);
        }
    }
    Ok(out)
}
