# Changelog

All notable changes to this project are documented in this file, generated
from conventional commits by [release-please](https://github.com/googleapis/release-please).

## [1.3.0](https://github.com/juniorsantos/betterreview/compare/v1.2.0...v1.3.0) (2026-07-29)


### Features

* add clickable review links ([#85](https://github.com/juniorsantos/betterreview/issues/85)) ([69521ed](https://github.com/juniorsantos/betterreview/commit/69521ed538bea7a8ffd35c8ef21b4f2bfb398781))
* copy patch hunks and comments ([#83](https://github.com/juniorsantos/betterreview/issues/83)) ([339c30d](https://github.com/juniorsantos/betterreview/commit/339c30dbf58345555ae45261cb25d33611e991ad))
* suspend reviews with Ctrl+Z ([#86](https://github.com/juniorsantos/betterreview/issues/86)) ([7064be2](https://github.com/juniorsantos/betterreview/commit/7064be219dfc955bd344d3c2cc838524edf917b8))


### Bug Fixes

* open review links on click ([#88](https://github.com/juniorsantos/betterreview/issues/88)) ([eee1fad](https://github.com/juniorsantos/betterreview/commit/eee1fad5061e823c335a145816953c13588b3ff2))

## [1.2.0](https://github.com/juniorsantos/betterreview/compare/v1.1.0...v1.2.0)


### Features

* jump to the first and last diff row or file ([#30](https://github.com/juniorsantos/betterreview/issues/30))
* copy the current line, selection or hunk as clean code through OSC-52 ([#76](https://github.com/juniorsantos/betterreview/issues/76))
* keep reviewed status tied to the current HEAD ([#75](https://github.com/juniorsantos/betterreview/issues/75))
* add a dedicated status column and expand author and branch metadata ([#80](https://github.com/juniorsantos/betterreview/pull/80))


### Documentation

* refresh both READMEs and all screenshots, including the approval review modal ([#80](https://github.com/juniorsantos/betterreview/pull/80))

## [1.1.0](https://github.com/juniorsantos/betterreview/compare/v1.0.0...v1.1.0)


### Features

* outline modal and comment actions ([86a1ebc](https://github.com/juniorsantos/betterreview/commit/86a1ebc5dce6445950b6979d5f3ebece2b6124fc))
* polish modal and comment surfaces ([b23eaa4](https://github.com/juniorsantos/betterreview/commit/b23eaa4352ea480a06bb97ed5896559880917f44))
* refine modal styling and comment wrapping ([23c40a6](https://github.com/juniorsantos/betterreview/commit/23c40a63b5c40deb2435160b714d09ca107ff79d))


### Bug Fixes

* adjust action button padding ([50a55c6](https://github.com/juniorsantos/betterreview/commit/50a55c690e5957e8ed1b48c9fd5b4d4464a268d9))
* align and fill action buttons ([6ecba39](https://github.com/juniorsantos/betterreview/commit/6ecba39f8e77c4498666585f9227980e76782e48))
* clean compact button borders ([c71d149](https://github.com/juniorsantos/betterreview/commit/c71d1499ec7f9f37172ae14931b861a51ae34935))
* compact themed action buttons ([bb5d446](https://github.com/juniorsantos/betterreview/commit/bb5d4467a1d4dce10162b6e8c5629abebeea89b4))
* reduce action button footprint ([a0eef79](https://github.com/juniorsantos/betterreview/commit/a0eef79ad5e36311e165d4d48369305de3e2a4aa))
* reduce action button height ([e41ee74](https://github.com/juniorsantos/betterreview/commit/e41ee746f5d497034ec0c70347ec7bbd925218a5))
* refine review dialogs and comment actions ([b7d09c3](https://github.com/juniorsantos/betterreview/commit/b7d09c31eab385a3dc2e90190a1c41da3a7b10e8))
* simplify action button surfaces ([ae39d28](https://github.com/juniorsantos/betterreview/commit/ae39d28cc85f5a9dce3ecef65ec9df3a7192a5a8))
* strengthen modal action buttons ([19d59b2](https://github.com/juniorsantos/betterreview/commit/19d59b2d16eda61f0fe8fd30b7e2a1ac41ec8f88))

## [1.0.0](https://github.com/juniorsantos/betterreview/compare/v0.5.0...v1.0.0)


### Features

* a blame gutter names who wrote each line of the old side (closes [#32](https://github.com/juniorsantos/betterreview/issues/32)) ([b39ab7f](https://github.com/juniorsantos/betterreview/commit/b39ab7f96c96f46ac77959bbd60ff52298a0ed6d))
* a failed blame explains itself in a dialog with git's own words ([0623ef0](https://github.com/juniorsantos/betterreview/commit/0623ef0ee355c7fa91dc3815ad6f13ed9bd82350))
* a transparent canvas leaves the terminal background alone (closes [#26](https://github.com/juniorsantos/betterreview/issues/26)) ([a521acf](https://github.com/juniorsantos/betterreview/commit/a521acfcf86525b148f21c32b79cfac189b2e17b))
* DiffLayout::Auto picks the layout from the panel width (closes [#59](https://github.com/juniorsantos/betterreview/issues/59)) ([1bd2430](https://github.com/juniorsantos/betterreview/commit/1bd24301ed9da790bcb9201dcf912f061804a67e))
* mark code that only changed place in its own colour (closes [#31](https://github.com/juniorsantos/betterreview/issues/31)) ([1ea6bd1](https://github.com/juniorsantos/betterreview/commit/1ea6bd17b34cc4d4f46bd0820506721977fb2dcb))
* space toggles reviewed in the files panel where the checkbox is drawn (closes [#70](https://github.com/juniorsantos/betterreview/issues/70)) ([b47e4f1](https://github.com/juniorsantos/betterreview/commit/b47e4f1ea3198f2da0d3f4e3570c85bd72a8b576))
* square comment card with a gutter indicator and its keys on a line of their own (closes [#51](https://github.com/juniorsantos/betterreview/issues/51)) ([f1f4802](https://github.com/juniorsantos/betterreview/commit/f1f48023f994ded4668e5588f42fcd881dff8214))
* the comment bar runs from the reviewed line through the card and the keys stay visible ([9b30e9c](https://github.com/juniorsantos/betterreview/commit/9b30e9cec8af3d8fdfd63faf8ad4b8c0a5bf768a))
* the files cursor marks the folder itself and enter folds it ([93e5c92](https://github.com/juniorsantos/betterreview/commit/93e5c924c453e3f2817eed88e4b5fe920b35fd0e))
* the review regions lose their frames in favour of one separator (closes [#66](https://github.com/juniorsantos/betterreview/issues/66)) ([7db6a41](https://github.com/juniorsantos/betterreview/commit/7db6a41644636412b5a529beda0dcd8dcffb6d7b))
* tint reply cards a lighter blue so an answer reads apart from the comment it answers ([1dabf9c](https://github.com/juniorsantos/betterreview/commit/1dabf9c149fccd73953d5dbc38538158cb24b20d))


### Bug Fixes

* escape bidi and zero-width characters and flag the files carrying them (closes [#56](https://github.com/juniorsantos/betterreview/issues/56)) ([9b120db](https://github.com/juniorsantos/betterreview/commit/9b120dbba051a4ea4ba038f273619006f51d5016))
* fall back to the changes endpoint when a gitlab instance has no working diffs route (closes [#72](https://github.com/juniorsantos/betterreview/issues/72)) ([a4093c4](https://github.com/juniorsantos/betterreview/commit/a4093c48f2b198e4be277324d753edc84e2da428))
* indent the files tree without a guide line and lighten the alignment hatch ([582c6ab](https://github.com/juniorsantos/betterreview/commit/582c6ab90743a22c9a193a84169393e1c25c1324))
* keep the comment marking on the end of a range that still resolves ([13d8d57](https://github.com/juniorsantos/betterreview/commit/13d8d57692e565bcaf5f9dbe39a2b9f677601e03))
* mark the cursor row with a bar the diff background cannot swallow (closes [#65](https://github.com/juniorsantos/betterreview/issues/65)) ([a9e561e](https://github.com/juniorsantos/betterreview/commit/a9e561e5994563e97aea2145d3d3c62708fe2eb6))
* persist the closed editor and the cleared pending submit so they do not come back on relaunch ([8a1d0a7](https://github.com/juniorsantos/betterreview/commit/8a1d0a7949865988139205cba68585deb54fa9bd))
* read the comment line range back from both providers so a reopened review keeps its anchor (closes [#71](https://github.com/juniorsantos/betterreview/issues/71)) ([a9e42f3](https://github.com/juniorsantos/betterreview/commit/a9e42f3cf47cb5ed37d02a1fcefc012753cc2257))
* refetch after a write so a reply or a resolve reaches the screen (closes [#74](https://github.com/juniorsantos/betterreview/issues/74)) ([d376239](https://github.com/juniorsantos/betterreview/commit/d376239c96bf28788a6386527d21547cf25a5bc0))
* the arrows walk between panels and stop at the ends instead of wrapping ([607bb13](https://github.com/juniorsantos/betterreview/commit/607bb13ad958116d8efa912bbc781230b5fc3d7d))


### Reverts

* bring the panel frames back (reopens [#66](https://github.com/juniorsantos/betterreview/issues/66)) ([6beb877](https://github.com/juniorsantos/betterreview/commit/6beb877a1622b31ae2a0dac4a4396177e210a79a))


### Miscellaneous Chores

* accumulate the next release as 1.0.0 ([0780922](https://github.com/juniorsantos/betterreview/commit/07809222ce3859d997a0d87ece2575b088f9f49e))

## [0.5.0](https://github.com/juniorsantos/betterreview/compare/v0.4.0...v0.5.0)


### Features

* a renamed file header points from its old path to the new one (closes [#58](https://github.com/juniorsantos/betterreview/issues/58)) ([7ed39c4](https://github.com/juniorsantos/betterreview/commit/7ed39c477be41ee238a3b9c57638924b44105962))
* expand one side of the split and move the column geometry out of the widget (closes [#45](https://github.com/juniorsantos/betterreview/issues/45), closes [#46](https://github.com/juniorsantos/betterreview/issues/46)) ([f2a2aaf](https://github.com/juniorsantos/betterreview/commit/f2a2aaf3bce9ff61ebdefa6512235f3cc1aae596))
* name the enclosing section in the hunk header and the pinned row (closes [#57](https://github.com/juniorsantos/betterreview/issues/57)) ([25ec0c5](https://github.com/juniorsantos/betterreview/commit/25ec0c5151f57c7dec2305e0980b06c384d00936))
* panel titles carry their list size and the alignment gap is truly hatched (closes [#50](https://github.com/juniorsantos/betterreview/issues/50)) ([311b8d2](https://github.com/juniorsantos/betterreview/commit/311b8d2da02412a2520f9eb50a5719e464584c31))
* pin the file and hunk header while the diff scrolls (closes [#28](https://github.com/juniorsantos/betterreview/issues/28)) ([39daf8c](https://github.com/juniorsantos/betterreview/commit/39daf8c702c8da55612fe06fe4f41cb3336e20ed))
* re-tier the palette so frames and line numbers recede behind the code (closes [#63](https://github.com/juniorsantos/betterreview/issues/63)) ([4063e30](https://github.com/juniorsantos/betterreview/commit/4063e304adbc2565e15ee3724557c8f20bf932fc))
* shell completions for bash, zsh and fish (closes [#40](https://github.com/juniorsantos/betterreview/issues/40)) ([c6cbdfe](https://github.com/juniorsantos/betterreview/commit/c6cbdfe2264d0d6809982056d76edd4f6e3cc45b))
* show hunk progress and diff totals in the status bar (closes [#29](https://github.com/juniorsantos/betterreview/issues/29)) ([71789b2](https://github.com/juniorsantos/betterreview/commit/71789b2bfd71fea676b0e4c7c79e00b02fcf014a))
* size the gutter from the file's line count (closes [#61](https://github.com/juniorsantos/betterreview/issues/61)) ([fd6cd13](https://github.com/juniorsantos/betterreview/commit/fd6cd136ae40b96ec5cf1ecdf0b46a9c3a711ba0))
* two-column line-number gutter in the unified diff (closes [#54](https://github.com/juniorsantos/betterreview/issues/54)) ([66ef690](https://github.com/juniorsantos/betterreview/commit/66ef690e53e7beba91305b7c1308acdcf5128c18))
* wrap long lines with w and mark truncation otherwise (closes [#21](https://github.com/juniorsantos/betterreview/issues/21)) ([8854ecb](https://github.com/juniorsantos/betterreview/commit/8854ecb305955b5c361a81f9bb0b18c389f596ca))


### Bug Fixes

* blank the expanded-gap old number when it computes below line one ([15cd5d3](https://github.com/juniorsantos/betterreview/commit/15cd5d3075e7bbd3df10f27d7ccf8ceb75a0392e))
* expand tabs to their tab stops so they occupy the cells they paint (closes [#55](https://github.com/juniorsantos/betterreview/issues/55)) ([96b3964](https://github.com/juniorsantos/betterreview/commit/96b3964498439f719c75776e4b5132a962ba658c))
* group help shortcuts by intent in balanced columns (closes [#53](https://github.com/juniorsantos/betterreview/issues/53)) ([e38fef5](https://github.com/juniorsantos/betterreview/commit/e38fef5f9059fae7aa13fc6c6864fd4b4419a80a))
* indent the files tree by depth and trace each file to its folder (closes [#62](https://github.com/juniorsantos/betterreview/issues/62)) ([2fe7fa1](https://github.com/juniorsantos/betterreview/commit/2fe7fa131f76df634c145ab55c11b7883f6994d6))
* keep the gutter reserved on wrapped rows instead of spending a row on the number (closes [#60](https://github.com/juniorsantos/betterreview/issues/60)) ([b8ac69c](https://github.com/juniorsantos/betterreview/commit/b8ac69c4182053b3a25477f5e613125ea65ca6eb))
* measure text in terminal cells and abbreviate paths instead of cutting names (closes [#25](https://github.com/juniorsantos/betterreview/issues/25), closes [#44](https://github.com/juniorsantos/betterreview/issues/44)) ([adaf97c](https://github.com/juniorsantos/betterreview/commit/adaf97c9b30e351f4d68f54d94e60b2592df8a4a))

## [0.4.0](https://github.com/juniorsantos/betterreview/compare/v0.3.0...v0.4.0)


### Features

* hide the files panel with f and track the real terminal width (closes [#19](https://github.com/juniorsantos/betterreview/issues/19)) ([1961cf6](https://github.com/juniorsantos/betterreview/commit/1961cf64fa093d786c9f50bea14c6b6a199bb2e4))
* optional side-by-side diff layout toggled with backslash (closes [#5](https://github.com/juniorsantos/betterreview/issues/5)) ([f29169e](https://github.com/juniorsantos/betterreview/commit/f29169e7194460c5d4fc5d7e78641a38ed7b3b19))


### Bug Fixes

* draw a large block banner on the splash screen (closes [#17](https://github.com/juniorsantos/betterreview/issues/17)) ([48f31e3](https://github.com/juniorsantos/betterreview/commit/48f31e377e6b0cc2eff036c17c7117301d802978))
* keep the config out of the session directory and read it from ~/.config (closes [#20](https://github.com/juniorsantos/betterreview/issues/20)) ([9937792](https://github.com/juniorsantos/betterreview/commit/993779209bf08b804199e9fee2da148f67d0c445))

## [0.3.0](https://github.com/juniorsantos/betterreview/compare/v0.2.0...v0.3.0)


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
