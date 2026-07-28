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
    FocusLeft,
    FocusRight,
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
    JumpToStart,
    JumpToEnd,
    MoveCursor(i32),
    /// Selects the file at this index in `provider.files`, mirroring
    /// `NextFile`/`PreviousFile`'s `activate_file` landing — used by a mouse
    /// click on a file row. A folded, non-representative index is a no-op
    /// (defensive: the click handler should never produce one, since folded
    /// files don't appear in `visible_rows`).
    ActivateFile(usize),
    /// Toggles the fold state of an arbitrary directory, not necessarily the
    /// active file's — used by a mouse click on a directory header (`z`/
    /// `Enter` only ever fold the active file's directory via `ToggleFold`).
    ToggleFoldDir(String),
    /// Clamps `index` into `display_rows` and lands on it, snapping a
    /// non-stop row (comment body/footer, file/orphan header) back to the
    /// nearest preceding stop — used by a mouse click on the diff panel,
    /// which can land on any drawn line, not just a navigation stop.
    JumpToDisplayRow(usize),
    ToggleReviewed,
    ToggleHunkReviewed,
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
    BackToPicker,
    ConfirmQuit(QuitChoice),
    ToggleHelp,
    ToggleFilesPanel,
    ToggleFilesVisible,
    ToggleFold,
    ToggleComments,
    ToggleDiffLayout,
    CycleSplitSide,
    ToggleWrap,
    ToggleBlame,
    DismissBlocked,
    ExpandGap,
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
