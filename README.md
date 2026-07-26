# betterreview

[![CI](https://github.com/juniorsantos/betterreview/actions/workflows/ci.yml/badge.svg)](https://github.com/juniorsantos/betterreview/actions/workflows/ci.yml)

🇧🇷 [Leia em Português](README_PT_BR.md)

Terminal code review for GitHub pull requests and GitLab merge requests. Navigate the diff with vim/lazygit-style keys, comment inline just like on GitHub, mark files as reviewed and submit your review — without leaving the terminal.

![Review screen](assets/review.svg)

When launched inside a repository, the picker lists the open reviews and prefetches the highlighted PR:

![PR picker](assets/picker.svg)

## Features

- Diff with GitHub visual parity: edge-to-edge green/red backgrounds, a single line-number column, file name on top and expandable hidden context (`z`)
- Inline comments as cards: create (`c`), edit (`e`), delete (`x`), reply (`r`) and code suggestions (`s`)
- Line or block selection (`v`) to comment exactly like on GitHub
- File tree with reviewed checkboxes (`m`), collapsible folders and de-emphasized generated files
- Quick jumps: next hunk (`]h`), next comment (`]c`), next file (`]f`), next unreviewed (`]u`)
- Hunk-level progress (`M`): each hunk carries its own reviewed mark, surfaced in the Files panel as `2/5`
- In-diff search (`/`, `n`/`N`), mouse support (scroll and click)
- Persistent sessions: quit and resume the review where you left off (`betterreview resume`)
- Full review submission (`R`): approve, request changes or comment

## Dependencies

| Tool | Purpose | Install |
|---|---|---|
| [git](https://git-scm.com) | repository context | ships with macOS/Linux |
| [gh](https://cli.github.com) | GitHub PRs (`gh auth login`) | `brew install gh` |
| [glab](https://gitlab.com/gitlab-org/cli) | GitLab MRs (`glab auth login`) | `brew install glab` |
| [delta](https://github.com/dandavison/delta) | diff rendering | `brew install git-delta` |
| [gitui](https://github.com/extrawurst/gitui) | optional: staging before review | `brew install gitui` |

Run `betterreview doctor` to check that everything is ready.

## Installation

### Homebrew (macOS and Linux)

```sh
brew tap juniorsantos/tap
brew install betterreview
```

Or in a single command: `brew install juniorsantos/tap/betterreview`.

Installs `gh`, `glab` and `delta` automatically as dependencies.

### Release binary

Download the binary for your platform from the [releases page](https://github.com/juniorsantos/betterreview/releases):

```sh
# macOS Apple Silicon
curl -sSL https://github.com/juniorsantos/betterreview/releases/latest/download/betterreview-v0.1.0-aarch64-apple-darwin.tar.gz | tar xz
sudo mv betterreview /usr/local/bin/
```

Available targets: `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`.

### Via cargo

```sh
cargo install --git https://github.com/juniorsantos/betterreview
```

## Usage

```sh
# inside a repository: opens the picker with the open PRs/MRs
betterreview

# directly by URL
betterreview https://github.com/owner/repo/pull/42

# resume the last review session
betterreview resume

# list saved sessions
betterreview sessions

# check dependencies and authentication
betterreview doctor
```

### Main shortcuts

| Key | Action |
|---|---|
| `j`/`k` | move cursor |
| `Tab`, `2`/`3` | switch focus between Files and Diff |
| `v` | start/end line selection |
| `c` | comment on the line or selection |
| `s` | suggest code on the selection |
| `e` / `x` / `r` | edit / delete / reply to the comment under the cursor |
| `m` | mark file as reviewed |
| `M` | mark the hunk under the cursor as reviewed |
| `z` | expand hidden diff context (or collapse folder in the Files panel) |
| `]h` `[h` / `]c` `[c` | next/previous hunk / comment |
| `]f` `[f` / `]u` `[u` | next/previous file / unreviewed file |
| `/`, `n`/`N` | search in the diff |
| `R` | submit review (approve / request changes / comment) |
| `?` | help with all shortcuts |
| `q` | quit |

## Development

```sh
cargo test          # full suite
cargo test --test app_reducer   # reducer only, the fastest loop
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Releases are automated: `feat:`/`fix:` commits on `main` produce a semantic version bump, tag and release with binaries via GitHub Actions.

## License

Distributed under the MIT License. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
