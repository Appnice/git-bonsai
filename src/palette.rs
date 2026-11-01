//! Colours: the fixed palette of board "07 Commit pixels", the two themes, and language shades in OKLCH.

use crate::grid::{PotPart, Role, Shade};
use std::fmt;

/// An sRGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// Parses `#rrggbb` or `rrggbb` (the form GitHub's linguist colours come in).
    pub fn from_hex(s: &str) -> Option<Rgb> {
        let s = s.strip_prefix('#').unwrap_or(s);
        if s.len() != 6 || !s.is_ascii() {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
        Some(Rgb(byte(0)?, byte(2)?, byte(4)?))
    }

    /// Const constructor from `0xRRGGBB`.
    pub const fn hex(v: u32) -> Rgb {
        Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

/// The four shades of one language's leaf pads, level 1 (lightest) to 4 (darkest).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LangPalette {
    pub levels: [Rgb; 4],
}

impl LangPalette {
    /// Shades derived from a linguist colour (RFC: the linguist colour is level 2; levels 1/3/4 are
    /// lightness +0.12 / -0.10 / -0.20 in OKLCH at the same hue, chroma reduced until in gamut).
    ///
    /// Near-grey colours (C `#555555`, …) would make a dead-looking tree, so below
    /// [`MIN_LEAF_CHROMA`] the hue turns to sage green at that chroma, lightness kept.
    pub fn from_linguist(base: Rgb) -> LangPalette {
        let (l, c, h) = to_oklch(base);
        let (c, h, base) = if c < MIN_LEAF_CHROMA {
            let tinted = from_oklch(l, SAGE_CHROMA, SAGE_HUE);
            (SAGE_CHROMA, SAGE_HUE, tinted)
        } else {
            (c, h, base)
        };
        let shade = |dl: f64| from_oklch((l + dl).clamp(0.0, 1.0), c, h);
        LangPalette {
            levels: [shade(0.12), base, shade(-0.10), shade(-0.20)],
        }
    }

    /// The colour of one level (1..=4; out-of-range levels clamp).
    pub fn level(&self, level: u8) -> Rgb {
        self.levels[usize::from(level.clamp(1, 4) - 1)]
    }
}

/// Hand-tuned pads of board 07 (purple, rust-brown, blue), used by the reference map.
pub const BOARD_LANGS: [LangPalette; 3] = [
    LangPalette {
        levels: [
            Rgb::hex(0xd2b8ff),
            Rgb::hex(0xb48cff),
            Rgb::hex(0x9466f0),
            Rgb::hex(0x6e40c9),
        ],
    },
    LangPalette {
        levels: [
            Rgb::hex(0xf6d2b8),
            Rgb::hex(0xe8b48f),
            Rgb::hex(0xd0905f),
            Rgb::hex(0xa8683c),
        ],
    },
    LangPalette {
        levels: [
            Rgb::hex(0x8cc4f0),
            Rgb::hex(0x5a9ad4),
            Rgb::hex(0x3572a5),
            Rgb::hex(0x24527a),
        ],
    },
];

/// Light or dark README variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Theme {
    Dark,
    Light,
}

impl Theme {
    pub fn name(self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
        }
    }

    pub fn colors(self) -> ThemeColors {
        match self {
            Theme::Dark => ThemeColors {
                bg: Rgb::hex(0x0d1117),
                fg: Rgb::hex(0xe6edf3),
                muted: Rgb::hex(0x8b949e),
                empty: Rgb::hex(0x161b22),
                ghost: Rgb::hex(0x262c36),
                trunk_shadow: Rgb::hex(0x4e331d),
                badge_idle_bg: Rgb::hex(0x30363d),
                badge_idle_fg: Rgb::hex(0xe6edf3),
            },
            Theme::Light => ThemeColors {
                bg: Rgb::hex(0xffffff),
                fg: Rgb::hex(0x1f2328),
                muted: Rgb::hex(0x59636e),
                empty: Rgb::hex(0xebedf0),
                ghost: Rgb::hex(0xd0d7de),
                trunk_shadow: Rgb::hex(0x5c3b22),
                badge_idle_bg: Rgb::hex(0xd0d7de),
                badge_idle_fg: Rgb::hex(0x1f2328),
            },
        }
    }
}

/// Colours that change with the theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeColors {
    pub bg: Rgb,
    pub fg: Rgb,
    pub muted: Rgb,
    pub empty: Rgb,
    pub ghost: Rgb,
    pub trunk_shadow: Rgb,
    pub badge_idle_bg: Rgb,
    pub badge_idle_fg: Rgb,
}

/// Fixed colours from board 07, the same in both themes.
pub mod fixed {
    use super::Rgb;
    pub const TRUNK: Rgb = Rgb::hex(0x7a5230);
    pub const TRUNK_HIGHLIGHT: Rgb = Rgb::hex(0xa0703f);
    pub const SCAR: Rgb = Rgb::hex(0xe3c08f);
    pub const SOIL: Rgb = Rgb::hex(0x3a2a1a);
    pub const MOSS: Rgb = Rgb::hex(0x3fb950);
    pub const BLOSSOM: Rgb = Rgb::hex(0xff7eb6);
    /// An autumn leaf on its way down.
    pub const FALLEN: Rgb = Rgb::hex(0xd29922);
    pub const POT_RIM: Rgb = Rgb::hex(0xa86f4c);
    pub const POT_BODY: Rgb = Rgb::hex(0x8d5636);
    pub const POT_DARK: Rgb = Rgb::hex(0x5e3520);
    pub const BADGE_FG: Rgb = Rgb::hex(0x0d1117);
}

/// The colour of a cell. Leaves of a language with no palette fall back to the empty colour.
pub fn cell_color(role: Role, theme: &ThemeColors, langs: &[LangPalette]) -> Rgb {
    match role {
        Role::Empty => theme.empty,
        Role::Trunk(Shade::Base) | Role::Branch => fixed::TRUNK,
        Role::Trunk(Shade::Highlight) => fixed::TRUNK_HIGHLIGHT,
        Role::Trunk(Shade::Shadow) => theme.trunk_shadow,
        Role::Leaf { lang, level } => langs
            .get(usize::from(lang))
            .map_or(theme.empty, |p| legible(p.level(level), theme.empty)),
        Role::Blossom => fixed::BLOSSOM,
        Role::Scar => fixed::SCAR,
        Role::Fallen => fixed::FALLEN,
        Role::Soil => fixed::SOIL,
        Role::Moss => fixed::MOSS,
        Role::Pot(PotPart::Rim) => fixed::POT_RIM,
        Role::Pot(PotPart::Body) => fixed::POT_BODY,
        Role::Pot(PotPart::Dark) => fixed::POT_DARK,
        Role::Ghost => theme.ghost,
    }
}

/// Chroma below which a language colour counts as grey and gets the sage tint.
const MIN_LEAF_CHROMA: f64 = 0.06;
/// OKLCH chroma of the sage tint.
const SAGE_CHROMA: f64 = 0.085;
/// OKLCH hue of the sage tint (radians, ≈ 140°).
const SAGE_HUE: f64 = 2.44;

/// Minimum OKLCH lightness gap between a leaf and an empty day.
const MIN_LIGHTNESS_GAP: f64 = 0.16;

/// Moves a colour's lightness away from the empty-day colour until the gap is at least
/// [`MIN_LIGHTNESS_GAP`], keeping hue and chroma (dark languages on the dark theme, pale ones on
/// the light theme).
pub fn legible(c: Rgb, empty: Rgb) -> Rgb {
    let (l, ch, h) = to_oklch(c);
    let (le, _, _) = to_oklch(empty);
    if (l - le).abs() >= MIN_LIGHTNESS_GAP {
        return c;
    }
    let target = if le < 0.5 {
        le + MIN_LIGHTNESS_GAP
    } else {
        le - MIN_LIGHTNESS_GAP
    };
    from_oklch(target, ch, h)
}

// --- OKLCH (Björn Ottosson's OKLab) ---

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f64) -> f64 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn to_oklch(c: Rgb) -> (f64, f64, f64) {
    let r = srgb_to_linear(f64::from(c.0) / 255.0);
    let g = srgb_to_linear(f64::from(c.1) / 255.0);
    let b = srgb_to_linear(f64::from(c.2) / 255.0);
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    let ll = 0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s;
    let aa = 1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s;
    let bb = 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s;
    (ll, aa.hypot(bb), bb.atan2(aa))
}

/// Linear sRGB of an OKLCH colour, possibly outside [0, 1].
fn oklch_to_linear(l: f64, c: f64, h: f64) -> [f64; 3] {
    let (a, b) = (c * h.cos(), c * h.sin());
    let l_ = (l + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m_ = (l - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s_ = (l - 0.089_484_177_5 * a - 1.291_485_548 * b).powi(3);
    [
        4.076_741_662_1 * l_ - 3.307_711_591_3 * m_ + 0.230_969_929_2 * s_,
        -1.268_438_004_6 * l_ + 2.609_757_401_1 * m_ - 0.341_319_396_5 * s_,
        -0.004_196_086_3 * l_ - 0.703_418_614_7 * m_ + 1.707_614_701 * s_,
    ]
}

fn from_oklch(l: f64, c: f64, h: f64) -> Rgb {
    let mut chroma = c;
    let mut lin = oklch_to_linear(l, chroma, h);
    // Reduce chroma until the colour fits sRGB; 40 halvings-worth of steps are plenty.
    for _ in 0..40 {
        if lin.iter().all(|v| (-1e-9..=1.0 + 1e-9).contains(v)) {
            break;
        }
        chroma *= 0.9;
        lin = oklch_to_linear(l, chroma, h);
    }
    let to8 = |v: f64| (linear_to_srgb(v.clamp(0.0, 1.0)) * 255.0).round() as u8;
    Rgb(to8(lin[0]), to8(lin[1]), to8(lin[2]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        assert_eq!(Rgb::from_hex("#3572A5"), Some(Rgb(0x35, 0x72, 0xa5)));
        assert_eq!(
            Rgb::from_hex("dea584").map(|c| c.to_string()),
            Some("#dea584".into())
        );
        assert_eq!(Rgb::from_hex("#12345"), None);
        assert_eq!(Rgb::from_hex("#zz0000"), None);
    }

    #[test]
    fn oklch_round_trip_is_lossless_enough() {
        for c in [
            Rgb::hex(0x3572a5),
            Rgb::hex(0xdea584),
            Rgb::hex(0xa97bff),
            Rgb(0, 0, 0),
        ] {
            let (l, ch, h) = to_oklch(c);
            let back = from_oklch(l, ch, h);
            for (a, b) in [(c.0, back.0), (c.1, back.1), (c.2, back.2)] {
                assert!(a.abs_diff(b) <= 1, "{c} -> {back}");
            }
        }
    }

    #[test]
    fn grey_languages_turn_sage() {
        let p = LangPalette::from_linguist(Rgb::hex(0x555555));
        let (_, c, h) = to_oklch(p.level(2));
        assert!(c > 0.04 && (h - SAGE_HUE).abs() < 0.2, "{c} {h}");
        let rust = Rgb::hex(0xdea584);
        assert_eq!(LangPalette::from_linguist(rust).level(2), rust);
    }

    #[test]
    fn dark_languages_are_lifted_on_the_dark_theme() {
        let empty = Theme::Dark.colors().empty;
        let c = legible(Rgb::hex(0x24292e), empty);
        assert!(to_oklch(c).0 - to_oklch(empty).0 >= MIN_LIGHTNESS_GAP - 0.01);
        let bright = Rgb::hex(0xdea584);
        assert_eq!(legible(bright, empty), bright);
        let pale = legible(Rgb::hex(0xf1e05a), Theme::Light.colors().empty);
        assert!(
            to_oklch(Theme::Light.colors().empty).0 - to_oklch(pale).0 >= MIN_LIGHTNESS_GAP - 0.01
        );
    }

    #[test]
    fn linguist_shades_get_darker_with_level() {
        let p = LangPalette::from_linguist(Rgb::hex(0x3572a5));
        assert_eq!(p.level(2), Rgb::hex(0x3572a5));
        let lightness: Vec<f64> = p.levels.iter().map(|c| to_oklch(*c).0).collect();
        assert!(lightness.windows(2).all(|w| w[0] > w[1]), "{lightness:?}");
    }
}
