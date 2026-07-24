mod editor_state;
mod keymap;
pub mod picker;
mod render;
mod splash;
mod terminal;
pub mod theme;
mod viewport;
mod widgets;

pub use editor_state::EditorState;
pub use keymap::{KeyMap, key_to_action};
pub use render::render;
pub use splash::splash;
pub use terminal::{ExitReason, TuiError, handle_key, run};
