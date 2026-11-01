//! Git Bonsai: a pixel bonsai grown from a GitHub account's public activity, one day at a time.
//!
//! Pipeline: `fetch → snapshot → grow → layout → render`. Everything except `fetch` is a pure
//! function; the image is always a function of the tree state, never of the raw data.

pub mod grid;
pub mod layout;
pub mod palette;
pub mod reference;
pub mod render;
pub mod snapshot;
pub mod state;

pub use grid::{Grid, LangId, PotPart, Role, Shade};
pub use palette::{LangPalette, Rgb, Theme};
pub use render::{Header, LegendItem, Phase, Scene};
