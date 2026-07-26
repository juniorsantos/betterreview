mod widgets;

use crate::app::State;

pub fn draw(state: &State) {
    widgets::list::render(state);
    widgets::button::render("submit");
}
