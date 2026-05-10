# cmdlog

[![license](https://img.shields.io/github/license/mel0us/cmdlog)](LICENSE)
[![Tests](https://github.com/mel0us/cmdlog/actions/workflows/build.yml/badge.svg?branch=main)](https://github.com/mel0us/cmdlog/actions/workflows/build.yml)

Cross-shell command logger with an interactive TUI for browsing and reusing
your command history across bash, zsh, and tcsh.

cmdlog records every interactive command to a shared TSV log file, then lets
you search, filter, and re-use commands from any shell. The TUI renders
on stderr so the selected command flows to stdout, where a shell wrapper
injects it into your prompt for editing before execution.

## Quick Start

```bash
# Install latest release (Linux x86_64/aarch64, macOS arm64)
curl -fsSL https://raw.githubusercontent.com/mel0us/cmdlog/main/install.sh | bash

# Wire up your shell
cmdlog install bash      # or: zsh, tcsh

# Open a new shell, then:
cmdlog list              # launches interactive TUI
```

## Installation

`install.sh` (run in Quick Start above) downloads the prebuilt tarball for
your platform and lays it out as:

```
~/.local/share/cmdlog/
├── cmdlog              # binary (hooks for all three shells embedded inside)
└── default.conf        # seed config
~/.local/bin/cmdlog     # symlink → ~/.local/share/cmdlog/cmdlog
```

To pin a specific release: `./install.sh v2.4.0`.

### Build from source

```bash
make                # debug build
make release        # optimized build (LTO, stripped, ~2 MB)
make install        # release build + copy to ./bin/
```

Requires Rust toolchain (cargo). To use a from-source build, copy the binary
and `default.conf` into `~/.local/share/cmdlog/` and symlink the binary into
`~/.local/bin/cmdlog`. Shell hooks are embedded in the binary — no separate
hook files to copy.

### Hook install details

`cmdlog install <shell>` writes a guarded block (`# >>> cmdlog >>>` ...
`# <<< cmdlog <<<`) into the first existing rc candidate:

| Shell | rc file | rc content |
|-------|---------|------------|
| bash  | `~/.bashrc` | `eval "$(~/.local/bin/cmdlog hook bash)"` |
| zsh   | `~/.zshrc`  | `eval "$(~/.local/bin/cmdlog hook zsh)"`  |
| tcsh  | `~/.tcshrc` (or `~/.cshrc`) | `source ~/.local/share/cmdlog/cmdlog.tcsh` |

bash and zsh evaluate the hook source printed by `cmdlog hook <shell>` at
shell startup, so the hook always matches the installed binary. tcsh's
backtick substitution collapses newlines and breaks eval, so `cmdlog install
tcsh` extracts the embedded hook to `~/.local/share/cmdlog/cmdlog.tcsh` and
sources that file.

Re-runs are detected and refused (both eval and legacy source forms). To
remove: `cmdlog uninstall <shell>`.

## Usage

### Interactive TUI

When stderr is a TTY, `cmdlog list` launches an interactive browser:

```bash
cmdlog list                # full TUI
```

The TUI has five zones, navigable with **Tab**:

```
Show Columns:   [time] [shell] [path] [repo] [count]     column visibility
Quick Filter:   [this shell] [this dir] [this repo] ...  narrow results
Context Group:  [abspath] [repo] [relpath]                group current context on top
Sort Order:     [recency: new first] [frequency: most]    sort controls
─────────────────────────────────────────────
> search regex here█
─────────────────────────────────────────────
> git push origin main                             selected command
  make -j8
  cargo test -- --test-threads=1
  ssh server "bash -l -c 'cd $PWD && make test'"
─────────────────────────────────────────────
↑↓ navigate  Space toggle  Tab focus  Enter inject  dd delete  u undo  Esc quit       1/4
```

### Keybindings

#### List zone

| Key | Action |
|-----|--------|
| **j** / **Down** | Move selection down |
| **k** / **Up** | Move selection up (Up at top moves to search bar) |
| **gg** | Jump to top |
| **G** | Jump to bottom |
| **Space** | Toggle detail view for selected entry |
| **Enter** | Inject command into shell for editing |
| **dd** | Soft-delete command (dedup-aware, applied on exit) |
| **u** | Undo last delete |
| **PageUp/PageDown** | Scroll by 20 entries |

#### Search zone

| Key | Action |
|-----|--------|
| Type | Regex search (filters in real-time) |
| **Backspace** | Delete last character |
| **Enter** / **Down** | Move focus to list |

#### Show / Filter / Group / Order zones

| Key | Action |
|-----|--------|
| **Left/Right** | Reorder badges |
| **Space** / **Enter** | Toggle on/off or cycle options |
| **Up/Down** | Navigate between badges |

#### Global

| Key | Action |
|-----|--------|
| **Tab** | Cycle focus: Show > Filter > Group > Order > Search > List |
| **/** | Jump to search bar (from any non-search zone) |
| **Esc** / **Ctrl+C** | Quit without selecting |

### Detail View

Press **Space** on a selected command to expand its metadata:

```
> git push origin main
    tcsh (5): git push origin main --force --verbose
    repo: /home/user/project | owner/cmdlog
    dir: /home/user/project/src
    date: 2026-04-06T10:30:15 | 2h ago
```

Long lines wrap automatically on narrow terminals.

### Deleting Commands

Press **dd** on a selected command to soft-delete it immediately (no confirm
prompt). The behavior is dedup-aware:

- **Dedup on** — deletes all collapsed entries with the same command text
- **Dedup off** — deletes only the single selected entry

Press **u** to undo the last delete. Deletions are applied when you exit the
TUI via atomic log rewrite with verification.

### Path Display

The path column shows `$PWD` for entries matching the current working directory,
regardless of whether abspath or relpath display mode is active. The footer
displays a `current/total` position indicator.

### Compact

Remove waived and malformed entries from the log file:

```bash
cmdlog compact              # summary only: count of removable entries
cmdlog compact -n           # dry-run: list each entry with reason
cmdlog compact -f           # force: remove entries from the log
```

`-n` annotates reasons (`[waived]` in yellow, `[malformed]` in red, with tab
characters in reverse-video). `-f` rewrites the log atomically (temp-file +
verify + rename).

### Linear Output

For scripting or when stderr is not a TTY:

```bash
cmdlog list --no-tui                    # linear output to terminal
cmdlog list --no-color                  # plain text (for piping)
cmdlog list -a -t bash -s "git"         # all entries, bash only, matching "git"
cmdlog list --today --here              # today's commands in current dir
cmdlog list -d 2026-04 -p /tmp -n 50   # April, /tmp prefix, last 50
```

### CLI Flags

| Flag | Description |
|------|-------------|
| `-n`, `--last N` | Show last N entries (default: 20) |
| `-a`, `--all` | Show all entries |
| `-s`, `--search PAT` | Filter by command substring (case-insensitive) |
| `-d`, `--date PREFIX` | Filter by date prefix (e.g., `2026-04-06`) |
| `-t`, `--shell-type S` | Filter by shell (`bash`, `zsh`, `tcsh`) |
| `-p`, `--path PREFIX` | Filter by working directory prefix |
| `--today` | Today's entries only |
| `--here` | Current directory only |
| `--no-color` | Disable colored output |
| `--no-tui` | Disable interactive mode |
| `--version`, `-v`, `-V` | Show version and exit |

## Configuration

Settings are stored in `~/.cmdlog.conf` (TOML), auto-created on first run
from `~/.local/share/cmdlog/default.conf`.

TUI settings (show columns, filters, group, order) are saved automatically
when you exit the TUI, so your preferences persist between sessions.

### Waive List

Commands listed under `[waive]` are hidden from `cmdlog list` output (but
still recorded to the log). Commands containing shell operators
(`` ; & | ( ) { } $ ` ! < > ``) are always shown regardless of the waive list.

```toml
[waive]
commands = [
    "ls", "cd", "cat", "head", "tail",
    "grep", "find", "ps", "top",
    # add your own...
]
min_cmd_len = 5   # hide single-word commands of 5 chars or fewer (e.g., "make", "vim")
```

Edit `~/.cmdlog.conf` directly. Changes take effect on the next `cmdlog list`.
The `min_cmd_len` filter only applies to single-word commands without shell operators.

### Show Columns

```toml
[show]
order = ["repo", "path", "count", "time", "shell"]
time = "age"        # "off", "date", or "age"
dir = "off"         # "off", "abspath", or "relpath"
shell = false
repo = true
count = true
```

When the terminal is narrow, repo names automatically shorten from
`owner/repo` to just `repo`, and long commands truncate with `...` (full
command visible in detail view).

### Filter, Order, Group

```toml
[filter]
this_shell = true   # only current shell type
this_dir = false    # only current directory
this_repo = false   # only current git repo
today = false       # only today's commands
dedup = true        # collapse duplicates
piped = false       # only piped commands
chained = false     # only chained commands (contains ; & |)

[order]
sequence = ["recency", "frequency"]
recency = "asc"     # "asc" = newest first
frequency = "asc"   # "asc" = most frequent first

[group]
sequence = ["repo", "abspath", "relpath"]
abspath = true      # group by current directory
repo = true         # group by current git repo
relpath = true      # group by relative path in repo
```

### Command Injection

The `[inject]` section controls how selected commands are placed into your
shell prompt for editing. Each shell has a native method (default) plus
shared alternatives:

```toml
[inject]
bash = "readline"   # "readline", "tiocsti", or "history"
zsh = "print-z"     # "print-z", "tiocsti", or "history"
tcsh = "tiocsti"    # "tiocsti" or "history"
```

| Method | Shells | How it works |
|--------|--------|-------------|
| `readline` | bash | Sets `READLINE_LINE` via `bind -x` callback, triggered by a DSR terminal query. Command appears on the prompt line ready to edit. |
| `print-z` | zsh | Uses `print -z` builtin to push onto the edit buffer stack. Command appears on the next prompt line ready to edit. |
| `tiocsti` | all | Uses `TIOCSTI` ioctl to push each byte into the kernel terminal input queue, simulating keystrokes. Requires Linux < 6.2 or `sysctl -w dev.tty.legacy_tiocsti=1`. |
| `history` | all | Loads the command into shell history only. Nothing appears on the prompt; press Up to recall and edit. Works on any kernel. |

Missing `[inject]` keys are auto-populated with defaults on first use.

## How It Works

### Recording

Shell hooks fire after each command completes (via `PROMPT_COMMAND` in bash,
`precmd_functions` in zsh, `alias precmd` in tcsh). The hook extracts the last
command from shell history and passes it to `cmdlog record <shell> <pwd> <exit_code> <cmd>`.
The exit code (`$?` / `$status`) is captured as the first statement in the hook
function, before any other command can clobber it.

Bash 5.1+ uses `PROMPT_COMMAND` as an array for robustness against other tools
overriding it. Older bash falls back to string concatenation. Zsh uses the
`precmd_functions` array, which is inherently safe from overrides.

The binary applies filtering:
1. Skip empty commands
2. If the command contains shell operators (`` ; & | ( ) { } $ ` ! < > ``), always log
3. Skip shell builtins (`cd`, `echo`, `export`, etc.)

Accepted commands are appended as a TSV line to `~/.cmdlog.tsv`:

```
2026-04-06T10:30:15	bash	/home/user/project	0	git push origin main
```

### Querying

`cmdlog list` reads the log, applies waive filtering and dedup at display
time, then either launches the TUI (rendered on stderr) or prints linear
output. Config validation runs inline before the TUI starts — run
`cmdlog doctor` to repair broken configs.

### Shell Wrapper

Each hook defines a `cmdlog()` wrapper function (or alias in tcsh) that
intercepts `cmdlog list` calls. The wrapper:

1. Captures the binary's stdout (the selected command, printed when you
   press Enter in the TUI).
2. Reads the injection method from `~/.cmdlog.conf` `[inject]`.
3. Injects the command into the shell's edit buffer for editing,
   preserving access to aliases, functions, and shell-specific syntax.

The tcsh hook chains into any existing `precmd` alias (e.g., gitprompt.pl)
rather than replacing it.

## Log File

Commands are stored in TSV (Tab-Separated Values) format — a plain-text table
where each field is separated by a tab character (`\t`), one entry per line.

Default location: `~/.cmdlog.tsv`

Override with the `CMDLOG_FILE` environment variable:

```bash
export CMDLOG_FILE=/path/to/custom.log
```

Format: `DATE\tSHELL\tPWD\tEXIT_CODE\tCMD` (TSV, one line per entry).

## Performance

| Operation | Time |
|-----------|------|
| Record (skip builtin) | ~2 ms |
| Record (log) | ~2 ms |
| List (TUI startup) | ~40 ms |
| List (linear) | ~9 ms |
| Binary size | ~2 MB |

Hooks fire after command completion, before the next prompt. No impact on
command execution latency.
