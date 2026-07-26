use crate::app::State;

pub fn render(state: &State) {
    for index in 0..5 {
        let marker = if index == state.cursor { '>' } else { ' ' };
        println!("{marker} item {index}");
    }
}
