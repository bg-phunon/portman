# portman

TUI tool for monitoring and managing processes listening on TCP ports (macOS).

![License](https://img.shields.io/badge/license-MIT-blue.svg)

## Install

### Homebrew

```sh
brew tap bg-phunon/tap
brew install portman
```

### Cargo

```sh
cargo install --path .
```

### Build from source

```sh
git clone https://github.com/bg-phunon/portman.git
cd portman
cargo build --release
# binary at target/release/portman
```

## Usage

```sh
portman                  # Launch interactive TUI
portman 3000             # Launch with filter on port 3000
portman --json           # One-shot JSON output of all listening ports
portman --json 3000      # JSON output filtered to port 3000
```

## Keybindings

### Navigation

| Key | Action |
|---|---|
| `j` / `Down` | Move down |
| `k` / `Up` | Move up |
| `g` | Go to top |
| `G` | Go to bottom |
| `PgUp` / `PgDn` | Scroll by page |

### Select & Kill

| Key | Action |
|---|---|
| `Space` | Toggle mark on row |
| `m` | Mark all visible rows |
| `M` | Unmark all |
| `K` | Kill marked processes (or single selected) |

### Clipboard

| Key | Action |
|---|---|
| `y` | Copy port number |
| `Y` | Copy `kill -9 <pid>` command (multi-pid if marked) |

### Filter & Sort

| Key | Action |
|---|---|
| `/` | Enter filter mode (search by app, port, user, command) |
| `Esc` | Clear filter |
| `Tab` | Cycle sort column |
| `1`-`8` | Sort by column N (press again to toggle direction) |
| `S` | Reverse sort direction |

### General

| Key | Action |
|---|---|
| `r` | Refresh process list |
| `?` | Toggle help overlay |
| `q` / `Ctrl+C` | Quit |

## How it works

- Uses `lsof -iTCP -sTCP:LISTEN -n -P` to discover listening TCP ports
- Uses `sysinfo` crate for CPU, memory, and process metadata
- Auto-refreshes every 5 seconds
- Falls back to cached data if `lsof` fails

## Platform

macOS only (uses `lsof` for port discovery and `pbcopy` for clipboard).

## License

MIT
