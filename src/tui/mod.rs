mod editor_state;
mod keymap;
mod layout;
pub mod picker;
mod render;
mod splash;
mod terminal;
pub mod text;
pub mod theme;
mod viewport;
mod widgets;

pub use editor_state::EditorState;
pub use keymap::{KeyMap, key_to_action};
pub use layout::{DiffColumns, diff_columns};
pub use render::render;
pub use splash::splash;
pub use terminal::{ExitReason, TuiError, handle_key, run};
pub use text::{abbreviate_path, display_width, truncate_to_width};
pub use viewport::{start_wrapped, wrapped_height};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitSide {
    Old,
    New,
}
