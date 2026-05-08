pub mod badge;
pub mod config;
pub mod delete;
pub mod filter;
pub mod input;
pub mod state;
pub mod ui;

use std::io;

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, DisableBracketedPaste, EnableBracketedPaste};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::log::LogEntry;
use crate::repo::{RepoCache, RepoResolver};

use self::filter::{apply_pipeline, build_frequency_map, FilterSpec, PipelineContext};
use self::input::handle_event;

/// Lightweight timing collector enabled when CMDLOG_DEBUG is set.
/// Records nanosecond durations and prints summary stats on exit.
/// When disabled, all methods are no-ops and the caller's `time_*`
/// helpers skip the `Instant::now()` syscall entirely.
struct PerfTimer {
    enabled: bool,
    draws: Vec<u128>,
    pipelines: Vec<u128>,
}

impl PerfTimer {
    fn new() -> Self {
        Self {
            enabled: std::env::var_os("CMDLOG_DEBUG").is_some(),
            draws: Vec::new(),
            pipelines: Vec::new(),
        }
    }

    /// Run `f` and, only if enabled, record its duration as a draw sample.
    /// Skips `Instant::now()` entirely when disabled.
    fn time_draw<R>(&mut self, f: impl FnOnce() -> R) -> R {
        if self.enabled {
            let t = std::time::Instant::now();
            let r = f();
            self.draws.push(t.elapsed().as_nanos());
            r
        } else {
            f()
        }
    }

    fn time_pipeline<R>(&mut self, f: impl FnOnce() -> R) -> R {
        if self.enabled {
            let t = std::time::Instant::now();
            let r = f();
            self.pipelines.push(t.elapsed().as_nanos());
            r
        } else {
            f()
        }
    }

    fn report(&self, entry_count: usize, display_count: usize) {
        if !self.enabled { return; }
        eprintln!("[cmdlog perf] {} log entries, {} displayed", entry_count, display_count);
        Self::print_stats("draw    ", &self.draws);
        Self::print_stats("pipeline", &self.pipelines);
    }

    fn print_stats(label: &str, samples: &[u128]) {
        if samples.is_empty() {
            eprintln!("[cmdlog perf] {}: (no samples)", label);
            return;
        }
        let mut sorted: Vec<u128> = samples.to_vec();
        sorted.sort_unstable();
        let n = sorted.len();
        let sum: u128 = sorted.iter().sum();
        let mean = sum / n as u128;
        let p50 = sorted[n / 2];
        let p99 = sorted[(n * 99 / 100).min(n - 1)];
        let max = sorted[n - 1];
        eprintln!(
            "[cmdlog perf] {}: n={} mean={:.2}ms p50={:.2}ms p99={:.2}ms max={:.2}ms",
            label, n,
            mean as f64 / 1_000_000.0,
            p50 as f64 / 1_000_000.0,
            p99 as f64 / 1_000_000.0,
            max as f64 / 1_000_000.0,
        );
    }
}

/// Restore terminal to normal state. Safe to call multiple times,
/// from panic hooks, and from signal handlers (uses only write(2) to stderr).
fn restore_terminal() {
    let _ = execute!(
        io::stderr(),
        DisableBracketedPaste,
        DisableMouseCapture,
        LeaveAlternateScreen,
    );
    let _ = disable_raw_mode();
    let _ = execute!(io::stderr(), crossterm::cursor::Show);
}

// Raw FFI for signal handling (avoids adding libc crate)
#[cfg(unix)]
mod sig {
    pub const SIGTERM: i32 = 15;
    pub const SIGHUP: i32 = 1;
    pub const SIG_DFL: usize = 0;
    extern "C" {
        pub fn signal(signum: i32, handler: usize) -> usize;
        pub fn raise(sig: i32) -> i32;
    }
}

/// Signal handler for SIGTERM/SIGHUP: restore terminal, then re-raise
/// the signal with the default handler so the process exits with the
/// correct status (important for parent shells checking $?).
#[cfg(unix)]
unsafe extern "C" fn signal_cleanup(signum: i32) {
    restore_terminal();
    sig::signal(signum, sig::SIG_DFL);
    sig::raise(signum);
}

/// Run the interactive TUI. Returns the selected command or None if quit.
pub fn run(
    entries: Vec<LogEntry>,
    cmdlog_dir: &std::path::Path,
    config_path: &std::path::Path,
    mut state: state::AppState,
    current_shell: &str,
    current_dir: &str,
) -> Option<String> {
    let mut repo_cache = RepoCache::load(cmdlog_dir);
    let freq_map = build_frequency_map(&entries);
    let current_repo = repo_cache.repo_name(current_dir);
    let ctx = PipelineContext {
        current_shell,
        current_dir,
        current_repo: &current_repo,
    };

    let mut perf = PerfTimer::new();

    // Initial pipeline
    let mut display_entries = perf.time_pipeline(|| apply_pipeline(
        &entries, &FilterSpec::from(&state), &mut repo_cache, &freq_map, &ctx,
    ));

    // Install panic hook to restore terminal on crash
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        prev_hook(info);
    }));

    // Install signal handlers for SIGTERM/SIGHUP (SIGINT is handled as
    // a key event in raw mode via Ctrl+C; SIGKILL cannot be caught)
    #[cfg(unix)]
    unsafe {
        sig::signal(sig::SIGTERM, signal_cleanup as *const () as usize);
        sig::signal(sig::SIGHUP, signal_cleanup as *const () as usize);
    }

    // Setup terminal (render to stderr so stdout is free for the result)
    enable_raw_mode().expect("failed to enable raw mode");
    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture, EnableBracketedPaste)
        .expect("failed to setup terminal");
    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend).expect("failed to create terminal");

    // Event loop
    loop {
        // Adjust scroll and selection for visible height
        let visible_height = terminal.size().map(|s| s.height as usize).unwrap_or(24);
        let list_height = visible_height.saturating_sub(8); // header(4) + search(3) + footer(1)
        // Scroll viewport to keep selected_index visible
        if list_height > 0 && state.nav.selected_index >= state.nav.scroll_offset + list_height {
            state.nav.scroll_offset = state.nav.selected_index - list_height + 1;
        }
        if state.nav.selected_index < state.nav.scroll_offset {
            state.nav.selected_index = state.nav.scroll_offset;
        }
        // Clamp to valid entry range
        let entry_count = display_entries.len();
        if entry_count > 0 && state.nav.selected_index >= entry_count {
            state.nav.selected_index = entry_count - 1;
        }
        // Prevent blank lines at bottom: scroll_offset can't push past last entry
        if list_height > 0 && entry_count > 0 && state.nav.scroll_offset + list_height > entry_count {
            state.nav.scroll_offset = entry_count.saturating_sub(list_height);
        }

        // Draw
        perf.time_draw(|| {
            terminal
                .draw(|frame| ui::draw(frame, &state, &display_entries))
                .expect("failed to draw");
        });

        // Expire timed messages
        state.delete_log.tick();

        // Handle events (use poll with timeout when a timed message is active)
        let poll_timeout = if state.delete_log.has_timed_message() {
            std::time::Duration::from_millis(100)
        } else {
            std::time::Duration::from_secs(60)
        };
        if !event::poll(poll_timeout).unwrap_or(false) {
            continue; // timeout — redraw to update/expire message
        }
        if let Ok(ev) = event::read() {
            // Clear timed message on any user input
            if state.delete_log.message.is_some() {
                state.delete_log.clear_message();
            }
            let needs_refilter = handle_event(ev, &mut state, display_entries.len());

            // Clear undo highlight on any user interaction
            if !state.undo_highlight.is_empty() {
                state.undo_highlight.clear();
            }

            if state.quit {
                break;
            }

            // Check if Enter was pressed on a list item
            if let Some(ref cmd) = state.exec_cmd {
                if cmd.is_empty() {
                    if state.nav.selected_index < display_entries.len() {
                        let selected_cmd =
                            display_entries[state.nav.selected_index].entry.cmd.clone();
                        state.exec_cmd = Some(selected_cmd);
                    } else {
                        state.exec_cmd = None;
                        continue;
                    }
                }
                break;
            }

            // Handle dd delete (soft-delete in memory)
            if state.delete_requested {
                state.delete_requested = false;

                // Determine which display rows to delete
                let range = if let Some((lo, hi)) = state.nav.visual_range() {
                    lo..=hi
                } else {
                    let i = state.nav.selected_index;
                    i..=i
                };

                let mut batch = Vec::new();
                for idx in range {
                    if idx >= display_entries.len() {
                        break;
                    }
                    let de = &display_entries[idx];
                    if state.filter.dedup {
                        let target_cmd = &de.entry.cmd;
                        for e in &entries {
                            if e.cmd == *target_cmd {
                                batch.push(e.date.clone());
                            }
                        }
                    } else {
                        batch.push(de.entry.date.clone());
                    }
                }

                state.delete_log.delete_batch(batch);

                state.nav.exit_visual_mode();
                display_entries = perf.time_pipeline(|| apply_pipeline(
                    &entries, &FilterSpec::from(&state), &mut repo_cache, &freq_map, &ctx,
                ));
                if !display_entries.is_empty() && state.nav.selected_index >= display_entries.len() {
                    state.nav.selected_index = display_entries.len() - 1;
                }
                continue;
            }

            // Handle undo
            if state.undo_requested {
                state.undo_requested = false;
                let restored = state.delete_log.undo();
                if !restored.is_empty() {
                    state.undo_highlight = restored.into_iter().collect();
                    display_entries = perf.time_pipeline(|| apply_pipeline(
                        &entries, &FilterSpec::from(&state), &mut repo_cache, &freq_map, &ctx,
                    ));
                    if !display_entries.is_empty() && state.nav.selected_index >= display_entries.len() {
                        state.nav.selected_index = display_entries.len() - 1;
                    }
                }
                continue;
            }

            if needs_refilter {
                state.nav.exit_visual_mode();
                display_entries = perf.time_pipeline(|| apply_pipeline(
                    &entries, &FilterSpec::from(&state), &mut repo_cache, &freq_map, &ctx,
                ));
                // Clamp selection
                if !display_entries.is_empty() && state.nav.selected_index >= display_entries.len() {
                    state.nav.selected_index = display_entries.len() - 1;
                }
            }
        }
    }

    // Drain pending input to prevent partial escape sequences leaking to shell
    while event::poll(std::time::Duration::from_millis(20)).unwrap_or(false) {
        let _ = event::read();
    }

    // Restore terminal
    restore_terminal();
    // Remove our panic hook now that terminal is cleaned up
    let _ = std::panic::take_hook();

    // Apply pending soft-deletes to log file
    if !state.delete_log.is_empty() {
        if let Err(e) = crate::log::rewrite_excluding(state.delete_log.deleted_set()) {
            eprintln!("[cmdlog] delete failed: {}", e);
        }
    }

    // Save settings and repo cache
    let _ = config::save_config(&config_path, &state);
    repo_cache.save();

    perf.report(entries.len(), display_entries.len());

    state.exec_cmd
}
