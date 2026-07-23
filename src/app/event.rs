#[derive(Debug)]
pub enum AppEvent {
    Action(AppAction),
    Terminal(crossterm::event::Event),
    Tick,
    EffectFinished(Box<super::EffectResult>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    CreateDraft(crate::providers::NewDraftComment),
    UpdateDraft {
        id: crate::domain::DraftId,
        body: crate::providers::DraftBody,
    },
    DeleteDraft(crate::domain::DraftId),
    Reply {
        thread: crate::domain::ThreadId,
        body: crate::providers::DraftBody,
    },
    ResolveThread {
        thread: crate::domain::ThreadId,
        resolved: bool,
    },
    SubmitReview {
        summary: String,
        outcome: crate::domain::ReviewOutcome,
    },
    DiscardReview,
    CancelSubmit,
    Refresh,
    Quit,
    ConfirmQuit(QuitChoice),
    ToggleHelp,
    ToggleFilesPanel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitChoice {
    KeepSession,
    DiscardEditor,
    Cancel,
}
