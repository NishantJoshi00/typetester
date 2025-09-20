# KeySentry
### Finally understand why you type the way you do

Ever wonder why you stumble on certain letter combinations? Why you pause before punctuation? KeySentry captures every millisecond, every hesitation, every muscle memory glitch. It's like having an MRI for your typing brain.

**Discover your typing DNA. See the invisible patterns. Train what actually matters.**

*"I had no idea I was doing that weird thing with the letter 'q'" - Every user after 5 minutes*

## What Makes KeySentry Special

**TYPING X-RAY VISION**
- See which finger combinations slow you down
- Discover which punctuation marks make you hesitate
- Find the digraphs (letter pairs) that trip you up
- Identify when you're fighting your own muscle memory

**PATTERN RECOGNITION INTELLIGENCE**
- Learns your personal weaknesses across sessions
- Maps your mental load: where do you pause to think?
- Tracks rhythm disruptions and when your flow breaks
- Detects fatigue patterns and warm-up curves

**REAL-TIME DISCOVERY**
- **Library Mode**: Practice with curated content that reveals common weaknesses
- **File Mode**: Analyze your real-world documents and code
- **Inception Mode**: Type KeySentry's own source code for the ultimate challenge

**INSIGHT-DRIVEN REPORTS**
- Every keystroke mapped with millisecond precision
- Error classification helps you understand what went wrong
- Visual flow analysis shows your rhythm and hesitation patterns
- JSON exports for deeper analysis

## Installation

### Prerequisites
- Rust 2024 edition or later
- Terminal emulator with color support

### Building from source
```bash
git clone <repository-url>
cd keysentry
cargo build --release
```

## Usage

### Quick Start

```bash
# Discover your typing DNA
cargo run -- --file README.md

# Challenge mode: Type your own code
cargo run -- --file src/main.rs

# INCEPTION MODE: Type KeySentry typing KeySentry (mind-bending)
cargo run -- --inception

# See all options
cargo run -- --help
```

**Try this first:** `cargo run -- --inception` - It's weirdly addictive.

### Discovery Modes

```bash
--size small   # Quick 2-minute insights (800-1600 chars)
--size medium  # Deep 5-minute analysis (1600-3200 chars) - default
--size large   # Epic 10-minute deep dive (3200-4800 chars)
```

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

## How KeySentry Works

### Error Detection & Analysis
KeySentry tracks four types of typing patterns:
- **Substitution**: Wrong character typed
- **Insertion**: Extra character added
- **Omission**: Character skipped
- **Repetition**: Character duplicated

When KeySentry detects an error, it allows up to 10 additional keystrokes before requiring correction. This gives you natural typing flow while ensuring mistakes don't compound indefinitely.

### Analytics Engine
KeySentry captures detailed metrics about your typing behavior:
- Words per minute (WPM) and accuracy percentages
- Keystroke latency and rhythm patterns
- Error distribution and correction response times
- Hesitation patterns including long pauses and punctuation delays
- Weakness analysis identifying slow digraphs and problematic transitions
- Finger load distribution across the QWERTY layout

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

## What You Get

**Instant Insights**
- Real-time visual feedback showing your typing DNA
- Live hesitation detection as you type
- Error classification that explains why you make mistakes

**Deep Analytics Export**
- Complete JSON reports with every keystroke analyzed
- Beautiful HTML visualizer (`stats_viewer.html`) for your data
- Shareable charts showing your unique patterns

**The "Aha!" Moments**
- "I never knew I hesitate before the letter 'q'"
- "My left pinky is way weaker than I thought"
- "I type 20% faster after coffee"
- "I stumble on the same 3 letter combinations every time"

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

## Share Your Discoveries

Found something cool about your typing patterns? KeySentry makes it easy to share:

```bash
# Export your session data
# Press 'e' in the report view to generate JSON

# Open the beautiful HTML visualizer
open stats_viewer.html
```

**Community Challenges:**
- Post your "weirdest typing habit" discovery
- Share your before/after improvement screenshots
- Compare finger usage patterns with friends
- See who can master Inception Mode fastest


## Acknowledgments

Built with the excellent Rust TUI ecosystem:
- [ratatui](https://github.com/ratatui-org/ratatui) - Terminal UI framework
- [crossterm](https://github.com/crossterm-rs/crossterm) - Cross-platform terminal library