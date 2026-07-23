#[derive(Debug)]
pub enum AppEvent {
    Action(AppAction),
    Terminal(crossterm::event::Event),
    Tick,
    EffectFinished(Box<super::EffectResult>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    FocusNext,
    FocusPrevious,
    NextFile,
    PreviousFile,
    NextUnreviewed,
    PreviousUnreviewed,
    MoveCursor(i32),
    ToggleReviewed,
    ToggleSelection,
    OpenComment,
    OpenSuggestion,
    OpenThreads,
    OpenSubmit,
    Refresh,
    Quit,
    ToggleHelp,
}
