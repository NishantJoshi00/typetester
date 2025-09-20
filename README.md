# TypeTester

A terminal-based typing analyzer that tracks keystrokes in real time, detects and enforces correction of typing errors, and provides detailed performance insights. Designed for training, not passive tracking.

## Features

- **Real-time typing analysis** with character-by-character tracking
- **Error detection and enforcement** - freezes input until errors are corrected
- **Comprehensive analytics** including WPM, accuracy, error patterns, and rhythm analysis
- **Three usage modes**: library browser, direct file input, and "inception mode" (type the source code)
- **Detailed session reports** exported as JSON with optional HTML visualization
- **Text library system** with categorized practice materials

## Installation

### Prerequisites
- Rust 2024 edition or later
- Terminal emulator with color support

### Building from source
```bash
git clone <repository-url>
cd typetester
cargo build --release
```

## Usage

### Basic Usage
```bash
# Default mode - browse text library
cargo run

# Load specific file for typing practice
cargo run -- --file path/to/your/text.txt

# Inception mode - type the application's own source code
cargo run -- --inception

# Show all options
cargo run -- --help
```

### Chunk Sizes
Control practice text length with the `--size` flag:
- `small`: ~800-1600 characters (20-40 lines)
- `medium`: ~1600-3200 characters (40-80 lines) - default
- `large`: ~3200-4800 characters (80-120 lines)

### Text Library Structure
Organize practice texts in the `texts/` directory:
```
texts/
├── beginner/
├── intermediate/
├── advanced/
├── programming/
├── quotes/
└── samples/
```

## How It Works

### Error Detection & Enforcement
The application detects four types of errors:
- **Substitution**: Wrong character typed
- **Insertion**: Extra character typed
- **Omission**: Character skipped
- **Repeat**: Character duplicated

After detecting an error, the system allows up to 10 additional characters before freezing input. Typing resumes only after backspacing to correct the error.

### Analytics
TypeTester tracks comprehensive metrics:
- Words per minute (WPM) and accuracy
- Keystroke latency and rhythm patterns
- Error distribution and correction times
- Hesitation patterns (long pauses, punctuation delays, etc.)
- Weakness analysis (slow digraphs, problematic transitions)
- Finger load distribution (QWERTY layout analysis)

### Session Reports
Each session generates a detailed JSON report containing:
- Summary statistics (WPM, accuracy, total errors)
- Individual error events with timestamps
- Per-key statistics and latencies
- Typing rhythm data
- Hesitation and weakness analysis
- Time-series WPM tracking

## UI Navigation

### Text Selection Mode
- **↑/↓**: Navigate categories and files
- **Enter**: Select text for typing practice
- **q**: Quit application

### Typing Mode
- **Type naturally**: Real-time feedback with color coding
- **Backspace**: Correct errors (required when frozen)
- **Esc**: Return to text selection
- **Ctrl+C**: Quit application

### Report View
- **Tab**: Switch between Charts and Analysis views
- **Esc**: Return to text selection
- **q**: Quit application

## File Outputs

- **Session reports**: JSON files with complete analytics
- **HTML viewer**: Open `stats_viewer.html` in a browser to visualize JSON reports

## Development

### Commands
```bash
cargo build       # Build the project
cargo run          # Run in library browser mode
cargo test         # Run tests
cargo check        # Quick compile check
cargo clippy       # Linting
cargo fmt          # Format code
```

### Architecture
The application is built as a single Rust binary using:
- **ratatui**: Terminal UI framework
- **crossterm**: Cross-platform terminal input handling
- **serde**: JSON serialization for reports
- **clap**: Command-line argument parsing

Key components:
- `TypingSession`: Core engine for tracking performance and errors
- `App`: State management for UI modes (TextSelection → Typing → ShowingReport)
- Analytics engine: Real-time calculation of metrics and patterns

## Contributing

Contributions are welcome! Please:
1. Fork the repository
2. Create a feature branch
3. Follow the existing code style (run `cargo fmt`)
4. Add tests for new functionality
5. Submit a pull request

## License

[Add your preferred license here]

## Acknowledgments

Built with the excellent Rust TUI ecosystem:
- [ratatui](https://github.com/ratatui-org/ratatui) - Terminal UI framework
- [crossterm](https://github.com/crossterm-rs/crossterm) - Cross-platform terminal library