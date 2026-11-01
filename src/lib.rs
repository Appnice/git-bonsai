//! Git Bonsai: a pixel bonsai grown from a GitHub account's public activity, one day at a time.
//!
//! Pipeline: `fetch → snapshot → grow → layout → render`. Everything except `fetch` is a pure
//! function; the image is always a function of the tree state, never of the raw data.

pub mod config;
#[cfg(feature = "network")]
pub mod github;
pub mod grid;
pub mod grow;
pub mod layout;
pub mod palette;
pub mod reference;
pub mod render;
pub mod rules;
pub mod snapshot;
pub mod state;

pub use config::Config;
pub use grid::{Grid, LangId, PotPart, Role, Shade};
pub use grow::{grow, plant, GrowError};
pub use palette::{LangPalette, Rgb, Theme};
pub use render::{Header, LegendItem, Phase, Scene};
