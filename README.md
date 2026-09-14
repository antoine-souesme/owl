# owl

<p align="center">
  <img src="docs/media/demo.gif" alt="owl demo" width="800">
</p>

A command-line tool that lists your GitHub pull requests in the terminal and merges
them according to each repository's rules.

## Why

Keeping track of your pull requests means opening a browser or running `gh` command
after command, repository by repository. `owl` puts everything on one screen: the
list of PRs, their check status, their review status, and merging without leaving
the terminal.

## Requirements

- Rust (2021 edition)
- `gh` installed and logged in (`gh auth login`)

## Install

```bash
cargo install --path .
```

Or run it locally, without installing:

```bash
cargo run
```

## Usage

Run `owl` with no arguments. The list of pull requests appears: repository, number,
age of the last update, target branch, title, CI status and review status. Drafts
and conflicts are flagged.

### Keys

| Key | List view | Detail view |
|---|---|---|
| Up arrow, `k` | previous selection | scroll up |
| Down arrow, `j` | next selection | scroll down |
| Right arrow, `Enter` | open the detail | — |
| Left arrow, `Esc` | — | back to the list |
| `m` | open the merge dialog | open the merge dialog |
| `r` | refresh | refresh |
| `o` | open the PR in the browser | same |
| `q`, `Ctrl+C` | quit | quit |

The merge dialog only offers the methods the repository actually allows. If there is
only one, `owl` just asks for confirmation.

The list refreshes on its own every minute. When a PR becomes mergeable, a system
notification is sent — once per PR, and only at the moment it changes.

## Configuration

Optional file, at `~/.config/owl/config.toml`:

```toml
filters = ["author:@me", "is:open"]
refresh_interval = 60
preferred_merge_method = "squash"
page_size = 50
```

## Authentication

The token is looked up in this order: `OWL_TOKEN`, `GITHUB_TOKEN`, then the output of
`gh auth token`. It is never written to a file, logged, or displayed.

## What owl does not do

Create a pull request, push code, write comments, manage issues, show a line-by-line
diff, or merge several PRs at once.

## Development

```bash
cargo build
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

The project summary is in `DESCRIPTION.md`, the specifications in `docs/specs/`
(both in French).
