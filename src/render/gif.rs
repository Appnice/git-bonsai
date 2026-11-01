//! GIF export (feature `gif`). Stub on the `backend` branch; the real encoder lives on main.

use crate::palette::Theme;
use crate::render::Scene;

#[derive(Debug, thiserror::Error)]
pub enum GifError {
    #[error("GIF export is not built on this branch yet")]
    NotBuilt,
}

/// Renders the scene as an animated GIF (stub: always `NotBuilt`).
pub fn render(_scene: &Scene, _theme: Theme) -> Result<Vec<u8>, GifError> {
    Err(GifError::NotBuilt)
}
