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
    FocusFiles,
    FocusDiff,
    NextFile,
    PreviousFile,
    NextUnreviewed,
    PreviousUnreviewed,
    NextHunk,
    PreviousHunk,
    NextComment,
    PreviousComment,
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
    EditComment(crate::domain::DraftId),
    DeleteComment(crate::domain::DraftId),
    ConfirmDeleteChoice(bool),
    ReplyComment(crate::domain::ThreadId),
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
    ToggleFold,
    ToggleComments,
    ConfirmSearch,
    SearchNext,
    SearchPrevious,
    CancelSearch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitChoice {
    KeepSession,
    DiscardEditor,
    Cancel,
}
