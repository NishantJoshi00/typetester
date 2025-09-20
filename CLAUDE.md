# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

### Build and Run
```bash
cargo build       # Build the project
cargo run          # Run in default library browser mode
cargo run --release # Run optimized build

# CLI Usage Options
cargo run -- --file path/to/file.txt    # Load specific file for typing
cargo run -- --inception                # Use project source code (meta!)
cargo run -- --help                     # Show all CLI options
```

### Testing and Quality
```bash
cargo test         # Run tests
cargo check        # Quick compile check
cargo clippy       # Linting
cargo fmt          # Format code
```

### Project Structure
The codebase is a single Rust binary (`src/main.rs`) implementing a terminal-based typing analyzer using the ratatui TUI framework.

## Architecture Overview

This is a Rust-based terminal typing test application with comprehensive performance analysis capabilities and flexible CLI-based text selection.

### Usage Modes

The application supports three distinct usage modes:

1. **Library Browser Mode** (default): Interactive file browser for organized practice texts
2. **Direct File Mode** (`--file`): Load any text file directly for immediate typing practice
3. **Inception Mode** (`--inception`): Type the application's own source code (meta-programming practice!)

### Core Components

**TypingSession**: The main engine that tracks typing performance, error detection, and real-time metrics. Handles character-by-character comparison, error classification (Substitution, Insertion, Omission, Repeat), and enforcement logic that freezes input after consecutive errors.

**App & UI States**: Three-state application flow:
- `TextSelection`: Browse and select text files from `texts/` directory structure
- `Typing`: Active typing session with real-time feedback and visual indicators
- `ShowingReport`: Post-session analysis with two views (Charts/Analysis)

**Text Library System**: Hierarchical text organization supporting categorized practice texts loaded from filesystem (`texts/category/file.txt`).

**Analytics Engine**: Comprehensive performance tracking including:
- Real-time WPM calculation and accuracy metrics
- Keystroke latency measurement and rhythm analysis
- Error event logging with timestamps and correction tracking
- Hesitation pattern detection (pause types: LongPause, Punctuation, CaseChange, etc.)
- Weakness analysis (slowest digraphs, finger positioning errors, rhythm breaks)

### Data Structures

**SessionReport**: Complete session analytics exported as JSON containing all metrics, error events, key statistics, typing rhythm data, and personalized weakness analysis.

**Error Classification**: Structured error tracking with position, expected/actual characters, timestamps, and correction latency measurements.

**Performance Analytics**: Advanced metrics including digraph analysis, finger-error mapping (QWERTY layout), rhythm disruption detection, and problematic character transition identification.

### UI Framework

Built on ratatui with crossterm for cross-platform terminal input handling. Features real-time visual feedback with color-coded text display (green for correct, red for errors, gray for remaining), comprehensive charts and analysis views, and educational sidebar guidance.

### Export Capabilities

Sessions generate detailed JSON reports compatible with the included HTML stats viewer (`stats_viewer.html`) for web-based visualization of typing performance data.

## Development Notes

- The application expects a `texts/` directory structure for practice materials
- All timing uses `std::time::Instant` for high-precision measurement
- Error enforcement freezes input after 10 consecutive errors, requiring backspace correction
- Session data is comprehensively tracked for detailed performance analysis
- HTML viewer provides additional visualization capabilities beyond the terminal interface