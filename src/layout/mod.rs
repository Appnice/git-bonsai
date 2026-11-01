//! Layout: `TreeState` → `Scene`. Stub on the `backend` branch; the real generator lives on main.

use crate::reference;
use crate::render::{Phase, Scene};
use crate::state::TreeState;

/// Builds the scene for a tree (stub: board 07 in the growing phase).
pub fn build_scene(_state: &TreeState) -> Scene {
    reference::board_scene(Phase::Growing)
}
