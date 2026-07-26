mod app;
mod providers;
mod ui;

fn main() {
    let state = app::State::new();
    ui::draw(&state);
}
