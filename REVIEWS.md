# Cross-AI Code Review — portman v0.1.0

**Reviewed at:** 2026-04-12
**Reviewers:** Gemini CLI, Internal audit
**Files reviewed:** All source files (14 files, 2559 lines)

---

## Gemini Review

**Summary:**
portman is a well-structured and functionally complete TUI application for monitoring TCP ports on macOS. The separation of concerns between state management (app.rs), data acquisition (process.rs), and UI rendering (ui/) is clean and idiomatic. While the project is highly usable, there are several performance bottlenecks in the rendering loop and potential robustness issues in the lsof parsing logic that should be addressed.

**Strengths:**
- Architectural Clarity: Elm-like state machine in App makes the TUI predictable
- Effective Resource Joining: Correctly joins lsof output with sysinfo using HashMap lookup
- UX Features: Multi-select, filtering, and sorting by every column
- System Integration: pbcopy integration, Homebrew formula
- UI Design: Detail panel, color-coded resource usage, help overlay

**Concerns:**

| Severity | Concern | Description |
|---|---|---|
| HIGH | Rendering Performance | `filtered_processes()` performs full filter+sort on every call. Called multiple times per frame (table, detail, footer) |
| MEDIUM | Brittle lsof Parsing | Relies on whitespace splitting and hardcoded indices. Can fail if command names contain spaces |
| MEDIUM | Clipboard Portability | `clipboard_copy` uses pbcopy directly with silent failure |
| LOW | State Sync | Stale marks pruned only on refresh(). Kill attempt on dead PID possible between refreshes |
| LOW | Error Visibility | Errors truncated heavily in footer |

**Suggestions:**
1. Cache filtered results in App instead of recomputing every call
2. Use lsof `-F` field mode for robust parsing
3. Return anyhow::Result from clipboard_copy for better error feedback
4. Mix Length/Min constraints in table for small terminals
5. Consider clap for argument parsing

**Risk Assessment: LOW** — Production-ready for v0.1.0. Performance concern is easily refactored.

---

## Internal Audit Findings

Additional concerns found during synthesis:

| Severity | Concern | File | Description |
|---|---|---|---|
| HIGH | filtered_processes() called 3-4x per frame | app.rs:136 | Each render calls it from table, detail, footer, and selection logic. N*log(N) sort each time |
| MEDIUM | lsof command names with spaces | process.rs:215 | `split_whitespace()` breaks "Google Chrome Helper" into multiple fields, corrupting PID/TYPE indices |
| MEDIUM | No .gitignore for Cargo.lock discussion | Cargo.lock | Binary projects SHOULD commit Cargo.lock (correct), but should document this choice |
| LOW | Homebrew formula needs arch-aware binary | Formula/portman.rb | Current formula builds from source (correct for Homebrew), but no pre-built bottles |
| LOW | sysinfo refresh_processes_specifics overhead | process.rs:68 | Refreshes ALL processes even though we only need ~50-60 listening ones |

---

## Consensus Action Items

### Must Fix (HIGH)

1. **Cache filtered_processes() result** — compute once per frame, store in App
2. **Fix lsof parsing for spaced command names** — use `-F` field mode or fall back to fixed-width parsing

### Should Fix (MEDIUM)

3. **Clipboard error feedback** — surface pbcopy failures to user
4. **Responsive table columns** — use Min constraints for small terminals

### Nice to Have (LOW)

5. **clap for CLI args** — better --help and error messages
6. **Targeted sysinfo refresh** — only refresh PIDs found by lsof
