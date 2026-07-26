# Changelog

All notable changes to this project are documented in this file, generated
from conventional commits by [release-please](https://github.com/googleapis/release-please).

## [0.3.0](https://github.com/juniorsantos/betterreview/compare/v0.2.0...v0.3.0) (2026-07-26)


### Features

* migrate the whole interface to English (closes [#15](https://github.com/juniorsantos/betterreview/issues/15)) ([6bb33d2](https://github.com/juniorsantos/betterreview/commit/6bb33d2880d60ba8645045964ff61be1f641be57))
* Q returns to the review picker instead of quitting (closes [#14](https://github.com/juniorsantos/betterreview/issues/14)) ([37342cc](https://github.com/juniorsantos/betterreview/commit/37342cc307b576b24d3e94d605e30def88b034e4))


### Bug Fixes

* give the submit modal a visible focus and move the verdict to a shortcut (closes [#9](https://github.com/juniorsantos/betterreview/issues/9)) ([9845fca](https://github.com/juniorsantos/betterreview/commit/9845fca6d353d4b1d5efd82fe277b0a88dd09bcd))
* submit reviews with no draft and replace the option-key verdict shortcut with Tab (closes [#11](https://github.com/juniorsantos/betterreview/issues/11), closes [#12](https://github.com/juniorsantos/betterreview/issues/12)) ([6b64171](https://github.com/juniorsantos/betterreview/commit/6b641711efd5ec22d2cc129fa99d83ac0398598d))
* translate the submit modal assertions left in Portuguese ([389d42e](https://github.com/juniorsantos/betterreview/commit/389d42efdc7934cd1d13a516d7c035f96d27e663))

## [0.2.0] - 2026-07-26

### Features

- Review hunk by hunk with M, and brighter diff colors (closes #4)

### Chores

- Ignore the local codegraph index
## [0.1.4] - 2026-07-24

### CI

- Push the tap formula with a deploy key
## [0.1.3] - 2026-07-24

### CI

- Make the release publish step idempotent
## [0.1.2] - 2026-07-24

### Bug fixes

- Redact the crate version in layout snapshots

### CI

- Move to the node 24 action releases
## [0.1.1] - 2026-07-24

### Documentation

- Document the homebrew tap install
- Show the tap then install flow for homebrew

### CI

- Update the homebrew tap formula on release
- Bump patch for small changes and add glab to the formula
## [0.1.0] - 2026-07-24

### Features

- Bootstrap betterreview rust cli
- Add provider neutral review domain
- Add safe asynchronous process runner
- Resolve github and gitlab review context
- Add betterreview dependency doctor
- Parse canonical review diff positions
- Validate review line selections
- Render review patches with git delta
- Define review provider contract
- Load github review snapshots
- Support github review operations
- Load gitlab review snapshots
- Support gitlab review operations
- Persist resumable review sessions
- Reconcile resumed review sessions
- Add review application state machine
- Add terminal review navigation
- Add terminal review editors and modal
- Connect persistent terminal review workflow
- Restyle the tui with github dark high contrast
- List open pull requests for the picker
- List open merge requests for the picker
- Add picker state machine
- Render the review picker
- Drive the picker with prefetch tasks
- Open reviews from the repo picker
- Build display rows for inline comments
- Navigate the diff through inline comments
- Render inline comments in the diff
- Edit delete and reply to comments inline
- Show saving feedback in the status bar
- Show the repository in the picker header
- Render comments as cards with action hints
- De-emphasize generated files
- Jump between hunks and comments
- Search inside the diff
- Unify dialogs behind one component
- Translate the interface copy to portuguese
- Box comment cards with borders and key hints
- Bring back file checkboxes and confirm reviews from anywhere
- Restyle the picker after gwm
- Flatten the status bar hints
- Colorize the help keys
- Add the opening splash and align comment cards to the code body
- Add breathing room inside every bordered panel
- Number the review panels and focus them directly
- List descriptions for the picker
- Show fold chevrons on directory headers
- Show fold chevrons on directory headers
- Flow into the neighboring file at the diff edges
- Hide diff file headers from the display
- Read file contents from the providers
- Disable approving your own pull request and polish spacing
- Expand hidden context inside the diff
- Surface submit and quit in the status hints
- Bootstrap trailing context and keep folded folders visible
- Scroll every panel with the mouse wheel
- Hide raw hunk markers like modern review uis
- Paint diff line backgrounds edge to edge
- Share the screen layout math
- Click to focus and select with the mouse
- Pad comment cards and give them a distinct border color
- Give comment cards more inner padding

### Bug fixes

- Complete betterreview cli bootstrap
- Bound and terminate process execution
- Clean process groups on cancellation
- Harden delta rendering and diagnostics
- Load github review thread positions
- Navigate the focused terminal panel
- Keep terminal navigation in view
- Save comments and reviews with enter
- Resolve pending reviews through the reviews connection
- Stop stale drafts from hijacking new comments
- Cancel pending open when the highlight moves
- Keep the display cursor in sync with the session
- Protect parked drafts from edit and reply
- Clear stale errors when the list reloads
- Keep the highlight on the same review after reload
- Report an empty list after reload
- Surface notices and harden editor mode transitions
- Fall back to rest patches when the raw diff is too large
- Delete drafts with the right field and show the editor cursor
- Keep the draft anchor when an update response omits it
- Isolate delta from the user gitconfig
- Reserve room for picker row badges so they don't clip
- Polish the picker spacing and show the file name in the diff
- Restore the picker layout and scroll the review list
- Apply the review findings and colorize dialog hints
- Trim selection edges that land on hunk boundaries
- Reach folded folders scope z and streamline quitting
- Polish picker search and copy details
- Refetch the thread after replying

### Performance

- Load snapshots with concurrent provider calls
- Overlap startup fetches and fetch 100 items per page

### Documentation

- Define terminal viewport navigation
- Define review picker with prefetch
- Record round-2 fixes and lessons
- Plan the review picker implementation
- Record picker delivery
- Define inline diff comments
- Plan the inline comments implementation
- List comment keys in the help overlay
- Plan the hunk inspired improvements
- Specify the gwm restyle
- Add README with screenshots, install and usage
- Make English README the default and link README_PT_BR

### Tests

- Route list fixtures by operation name
- Cover the created draft appearing inline

### CI

- Add quality gate and automated release pipeline
- Generate the changelog on release and cross-compile macos x64

### Chores

- Stop tracking the local tasks folder
- Stop tracking planning docs
- Drop an unused import
## [0.0.0] - 2026-07-22

### Chores

- Initialize betterreview
