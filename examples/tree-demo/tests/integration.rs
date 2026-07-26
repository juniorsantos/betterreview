#[test]
fn the_cursor_never_goes_below_zero() {
    let mut state = tree_demo::app::State::new();
    state.move_cursor(-1);
    assert_eq!(state.cursor, 0);
}
