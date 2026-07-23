mod editor_state;
mod keymap;
mod render;
mod terminal;
pub mod theme;
mod viewport;
mod widgets;

pub use editor_state::EditorState;
pub use keymap::{KeyMap, key_to_action};
pub use render::render;
pub use terminal::{ExitReason, TuiError, handle_key, run};
