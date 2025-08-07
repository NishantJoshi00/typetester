Here’s the tight, implementation-focused product document for the Typing Analyzer TUI in Rust using ratatui and crossterm. This includes everything needed to start building with clear scope and deliverables.

⸻

Typing Analyzer – Product & Technical Specification (Rust TUI)

1. Objective

A terminal-based typing analyzer that tracks user keystrokes in real time, detects and enforces correction of typing errors, analyzes behavior, and outputs detailed performance insights. Designed for training, not passive tracking.

⸻

2. Tech Stack
	•	Language: Rust
	•	UI: ratatui
	•	Input: crossterm
	•	Timing: std::time::Instant
	•	Serialization: serde, serde_json

⸻

3. Functional Scope

3.1 Input Handling
	•	Raw key capture with timestamps
	•	Support for standard keys, backspace, and control characters
	•	Character-by-character comparison with target string (if present)

3.2 Error Detection
	•	Detects and classifies:
	•	Substitution (wrong char)
	•	Insertion (extra char)
	•	Omission (missing char)
	•	Repeat (duplicate char)

3.3 Enforcement Logic
	•	After an error:
	•	Allow up to 15 more characters
	•	Freeze input if not corrected
	•	Resume only when corrected via backspace

3.4 Metrics Tracked
	•	Words per minute (WPM)
	•	Accuracy (% correct)
	•	Average latency between keystrokes
	•	Error counts and types
	•	Correction latency
	•	Pre-error hesitation
	•	Post-correction recovery time
	•	Finger load and error distribution (if modeled)

3.5 Output
	•	Full SessionReport struct exported as JSON
	•	Includes summary metrics, error events, key stats, correction behavior, and time-based trends

⸻

4. Terminal UI Design

+---------------------------------------------------------+
| Typing Analyzer                                         |
|---------------------------------------------------------|
| Target:   The quick brown fox jumps over the lazy dog  |
| Input:    The quuck brwon                              |
|                                                       |
| Status: ERROR (Substitution at position 6)             |
| Correction required: 12/15 characters typed            |
|                                                       |
| WPM: 68   Accuracy: 93.1%   Errors: 4   Latency: 109ms |
+---------------------------------------------------------+


⸻

5. Data Structures

SessionReport

Full metrics summary with all insights (see previous answer). Serialized via serde.

ErrorEvent

Tracks type, location, timestamps, and correction behavior per error.

KeyStat

Latency, error count, and frequency per key.

Optional: MinuteStats

Rolling time-window stats to track fatigue and warm-up.

⸻

6. Session Flow
	1.	Load reference text (or free input mode)
	2.	Begin input capture and analysis
	3.	Display real-time feedback in TUI
	4.	Enforce correction discipline
	5.	End on text match or user action
	6.	Export full SessionReport as JSON

⸻

7. Deliverables
	•	TUI app with live typing view
	•	Real-time enforcement and stats
	•	Full session report in JSON
	•	Optional: basic command-line flags for mode, target file, output path

⸻

This document is minimal but complete — ready for immediate implementation. Let me know if you want a Cargo.toml scaffold or project layout next.
