use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{BarChart, Block, Borders, Paragraph, Wrap, Padding},
    Frame, Terminal,
};
use serde::{Deserialize, Serialize};
use chrono;
use clap::{Parser, Subcommand};
use std::{
    collections::HashMap,
    fs,
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Parser)]
#[command(name = "typetester")]
#[command(about = "A terminal typing tester with advanced analytics")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Load a specific file for typing practice
    #[arg(short, long, value_name = "FILE")]
    file: Option<PathBuf>,

    /// Use the project's own source code for typing practice (inception mode!)
    #[arg(long)]
    inception: bool,

    /// Size of the text chunk to practice with
    #[arg(short, long, value_enum, default_value = "medium")]
    size: ChunkSize,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum ChunkSize {
    Small,  // ~20-40 lines or 800-1600 characters
    Medium, // ~40-80 lines or 1600-3200 characters
    Large,  // ~80-120 lines or 3200-4800 characters
}

#[derive(Subcommand)]
enum Commands {
    /// Start typing test with file browser (default mode)
    Browse,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ErrorType {
    Substitution,
    Insertion,
    Omission,
    Repeat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEvent {
    pub error_type: ErrorType,
    pub position: usize,
    pub expected_char: Option<char>,
    pub actual_char: Option<char>,
    pub timestamp: Duration,
    pub correction_timestamp: Option<Duration>,
    pub correction_latency: Option<Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyStat {
    pub key: char,
    pub count: u32,
    pub total_latency: Duration,
    pub error_count: u32,
    pub latencies: Vec<u64>, // Individual keystroke latencies in ms
    pub positions: Vec<usize>, // Where this key appeared in text
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypingRhythm {
    pub timestamp: Duration,
    pub latency: Duration,
    pub position: usize,
    pub char_typed: char,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HesitationPattern {
    pub position: usize,
    pub duration: Duration,
    pub preceding_chars: String,
    pub following_chars: String,
    pub pattern_type: HesitationType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HesitationType {
    LongPause,        // >500ms pause
    DoubleDigraph,    // Common letter combinations (th, er, ing)
    Transition,       // Moving between hands/fingers
    Punctuation,      // Hesitation before punctuation
    CaseChange,       // Upper/lowercase transitions
    NumberSymbol,     // Numbers or symbols
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaknessAnalysis {
    pub slowest_digraphs: Vec<(String, f64)>, // Letter pairs and avg latency
    pub error_clusters: Vec<(usize, usize)>,  // Start/end positions of error zones
    pub finger_errors: HashMap<String, u32>,  // Finger assignment errors
    pub rhythm_breaks: Vec<usize>,            // Positions where rhythm broke
    pub problematic_transitions: Vec<(char, char, f64)>, // char1->char2, avg latency
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionReport {
    pub session_duration: Duration,
    pub total_characters: usize,
    pub correct_characters: usize,
    pub wpm: f64,
    pub accuracy: f64,
    pub average_latency: Duration,
    pub errors: Vec<ErrorEvent>,
    pub key_stats: HashMap<char, KeyStat>,
    pub total_corrections: usize,
    pub average_correction_latency: Option<Duration>,
    pub typing_rhythm: Vec<TypingRhythm>,
    pub hesitation_patterns: Vec<HesitationPattern>,
    pub weakness_analysis: WeaknessAnalysis,
    pub wpm_over_time: Vec<(Duration, f64)>, // WPM at different time points
}

#[derive(PartialEq)]
enum AppState {
    Typing,
    ShowingReport,
}

#[derive(PartialEq)]
enum ReportView {
    Charts,
    Analysis,
}


pub struct TypingSession {
    target_text: String,
    user_input: String,
    current_position: usize,
    errors: Vec<ErrorEvent>,
    key_stats: HashMap<char, KeyStat>,
    session_start: Instant,
    session_end: Option<Instant>,
    last_keystroke: Option<Instant>,
    has_error: bool,
    consecutive_errors: usize,
    is_frozen: bool,
    total_corrections: usize,
    typing_rhythm: Vec<TypingRhythm>,
    hesitation_patterns: Vec<HesitationPattern>,
    wpm_samples: Vec<(Instant, f64)>,
}

impl TypingSession {
    pub fn new(target_text: String) -> Self {
        Self {
            target_text,
            user_input: String::new(),
            current_position: 0,
            errors: Vec::new(),
            key_stats: HashMap::new(),
            session_start: Instant::now(),
            session_end: None,
            last_keystroke: None,
            has_error: false,
            consecutive_errors: 0,
            is_frozen: false,
            total_corrections: 0,
            typing_rhythm: Vec::new(),
            hesitation_patterns: Vec::new(),
            wpm_samples: Vec::new(),
        }
    }

    pub fn handle_key(&mut self, key: char) {
        if self.is_frozen {
            return;
        }

        let now = Instant::now();
        let latency = if let Some(last) = self.last_keystroke {
            now.duration_since(last)
        } else {
            Duration::from_millis(0)
        };

        if key == '\x08' {
            self.handle_backspace();
            self.last_keystroke = Some(now);
            return;
        }

        let expected_char = self.target_text.chars().nth(self.current_position);
        
        self.user_input.push(key);
        self.update_key_stats(key, latency);

        if let Some(expected) = expected_char {
            if key == expected {
                // Correct character typed
                if !self.has_error {
                    // No errors, advance normally
                    self.current_position += 1;
                    // Check if we completed the text
                    if self.current_position >= self.target_text.len() {
                        self.session_end = Some(now);
                    }
                } else {
                    // User typed correct character but we're in error state
                    // This means they're correcting by overtyping
                    self.has_error = false;
                    self.consecutive_errors = 0;
                    self.current_position += 1;

                    // Clear the error stack by truncating user_input to match current_position
                    // This removes all the incorrect characters that were in the error buffer
                    let target_chars: Vec<char> = self.target_text.chars().collect();
                    let mut corrected_input = String::new();
                    for i in 0..self.current_position {
                        if let Some(ch) = target_chars.get(i) {
                            corrected_input.push(*ch);
                        }
                    }
                    self.user_input = corrected_input;

                    if self.current_position >= self.target_text.len() {
                        self.session_end = Some(now);
                    }
                }
            } else {
                // Incorrect character typed
                self.handle_error(key, expected, now);
            }
        }

        self.last_keystroke = Some(now);
    }

    fn handle_backspace(&mut self) {
        if self.user_input.pop().is_some() {
            if self.has_error {
                // Reduce consecutive errors when backspacing in error state
                if self.consecutive_errors > 0 {
                    self.consecutive_errors -= 1;
                }
                
                // If no more consecutive errors, clear error state
                if self.consecutive_errors == 0 {
                    self.has_error = false;
                }
                
                self.is_frozen = false;
                self.total_corrections += 1;
            } else if self.current_position > 0 {
                self.current_position -= 1;
            }
        }
    }

    fn handle_error(&mut self, actual: char, expected: char, timestamp: Instant) {
        let error_type = if actual == expected {
            ErrorType::Repeat
        } else {
            ErrorType::Substitution
        };

        let error = ErrorEvent {
            error_type,
            position: self.current_position,
            expected_char: Some(expected),
            actual_char: Some(actual),
            timestamp: timestamp.duration_since(self.session_start),
            correction_timestamp: None,
            correction_latency: None,
        };

        self.errors.push(error);
        self.has_error = true;
        self.consecutive_errors += 1;
        
        // Freeze after 10 consecutive errors
        if self.consecutive_errors >= 10 {
            self.is_frozen = true;
        }
    }

    fn update_key_stats(&mut self, key: char, latency: Duration) {
        let now = Instant::now();
        let latency_ms = latency.as_millis() as u64;
        
        // Update key statistics
        let stat = self.key_stats.entry(key).or_insert(KeyStat {
            key,
            count: 0,
            total_latency: Duration::from_millis(0),
            error_count: 0,
            latencies: Vec::new(),
            positions: Vec::new(),
        });
        
        stat.count += 1;
        stat.total_latency += latency;
        stat.latencies.push(latency_ms);
        stat.positions.push(self.current_position);
        
        if self.has_error {
            stat.error_count += 1;
        }
        
        // Record typing rhythm
        self.typing_rhythm.push(TypingRhythm {
            timestamp: now.duration_since(self.session_start),
            latency,
            position: self.current_position,
            char_typed: key,
        });
        
        // Detect hesitation patterns
        if latency_ms > 500 {
            let preceding = if self.current_position >= 3 {
                self.target_text.chars()
                    .skip(self.current_position.saturating_sub(3))
                    .take(3)
                    .collect()
            } else {
                String::new()
            };
            
            let following: String = self.target_text.chars()
                .skip(self.current_position + 1)
                .take(3)
                .collect();
            
            let pattern_type = self.detect_hesitation_type(key, latency_ms, &preceding, &following);
            
            self.hesitation_patterns.push(HesitationPattern {
                position: self.current_position,
                duration: latency,
                preceding_chars: preceding,
                following_chars: following,
                pattern_type,
            });
        }
        
        // Sample WPM every 10 characters
        if self.current_position % 10 == 0 && self.current_position > 0 {
            let wpm = self.calculate_wpm();
            self.wpm_samples.push((now, wpm));
        }
    }
    
    fn detect_hesitation_type(&self, key: char, latency_ms: u64, preceding: &str, _following: &str) -> HesitationType {
        if latency_ms > 1000 {
            return HesitationType::LongPause;
        }
        
        if key.is_ascii_punctuation() {
            return HesitationType::Punctuation;
        }
        
        if key.is_ascii_digit() || "!@#$%^&*()_+{}|:<>?".contains(key) {
            return HesitationType::NumberSymbol;
        }
        
        if key.is_uppercase() != preceding.chars().last().map_or(false, |c| c.is_uppercase()) {
            return HesitationType::CaseChange;
        }
        
        // Check for common digraphs
        if let Some(prev_char) = preceding.chars().last() {
            let digraph = format!("{}{}", prev_char, key);
            if ["th", "er", "on", "an", "re", "he", "in", "ed", "nd", "ha"].contains(&digraph.as_str()) {
                return HesitationType::DoubleDigraph;
            }
        }
        
        HesitationType::Transition
    }

    pub fn calculate_wpm(&self) -> f64 {
        let elapsed = self.session_start.elapsed().as_secs_f64() / 60.0;
        if elapsed == 0.0 {
            0.0
        } else {
            (self.current_position as f64 / 5.0) / elapsed
        }
    }

    pub fn calculate_accuracy(&self) -> f64 {
        if self.user_input.is_empty() {
            100.0
        } else {
            (self.current_position as f64 / self.user_input.len() as f64) * 100.0
        }
    }

    pub fn is_complete(&self) -> bool {
        self.current_position >= self.target_text.len() && !self.has_error
    }

    pub fn get_status(&self) -> String {
        if self.is_frozen {
            "FROZEN: 10 consecutive errors! Use backspace to correct.".to_string()
        } else if self.has_error {
            format!("ERROR BUFFER: {} of 10 errors - use backspace to correct", self.consecutive_errors)
        } else {
            "Ready".to_string()
        }
    }

    pub fn generate_styled_text(&self) -> Vec<Line<'static>> {
        let target_chars: Vec<char> = self.target_text.chars().collect();
        let user_chars: Vec<char> = self.user_input.chars().collect();

        let mut lines = Vec::new();
        let mut current_line_spans = Vec::new();

        // Display correctly typed characters in green
        for i in 0..self.current_position.min(target_chars.len()) {
            let ch = target_chars[i];

            if ch == '\n' {
                // End current line and start a new one
                lines.push(Line::from(current_line_spans.clone()));
                current_line_spans.clear();
            } else if ch == '\t' {
                // Convert tab to 4 spaces
                let display_text = "    "; // 4 spaces
                if i == self.current_position - 1 && !self.has_error && !self.is_frozen {
                    // Last correctly typed character with cursor - green with underline
                    current_line_spans.push(Span::styled(
                        display_text.to_string(),
                        Style::default().fg(Color::Green).add_modifier(Modifier::UNDERLINED).add_modifier(Modifier::BOLD)
                    ));
                } else {
                    // Other correctly typed characters - green
                    current_line_spans.push(Span::styled(
                        display_text.to_string(),
                        Style::default().fg(Color::Green)
                    ));
                }
            } else {
                if i == self.current_position - 1 && !self.has_error && !self.is_frozen {
                    // Last correctly typed character with cursor - green with underline
                    current_line_spans.push(Span::styled(
                        ch.to_string(),
                        Style::default().fg(Color::Green).add_modifier(Modifier::UNDERLINED).add_modifier(Modifier::BOLD)
                    ));
                } else {
                    // Other correctly typed characters - green
                    current_line_spans.push(Span::styled(
                        ch.to_string(),
                        Style::default().fg(Color::Green)
                    ));
                }
            }
        }

        // Display error buffer (incorrect characters typed beyond correct position)
        if self.has_error && user_chars.len() > self.current_position {
            // Only show the actual incorrect characters that were typed beyond the correct position
            // We should display from current_position to user_chars.len(), but skip if the character
            // at current_position in user_input matches the expected character
            let error_start = self.current_position;
            let mut chars_to_show = Vec::new();

            // Collect only the actual error characters
            for i in error_start..user_chars.len().min(error_start + 10) {
                let user_char = user_chars[i];
                let expected_char_at_pos = self.target_text.chars().nth(i);

                // Only include characters that don't match what's expected at their position
                if Some(user_char) != expected_char_at_pos {
                    chars_to_show.push((i, user_char));
                }
            }

            // Display the error characters
            for (idx, (_i, user_char)) in chars_to_show.iter().enumerate() {
                if *user_char == '\n' {
                    // Handle newlines in error buffer
                    lines.push(Line::from(current_line_spans.clone()));
                    current_line_spans.clear();
                } else if *user_char == '\t' {
                    // Convert tab to 4 spaces in error display
                    let display_text = "    "; // 4 spaces
                    if idx == chars_to_show.len() - 1 {
                        // Last error character gets underline cursor
                        current_line_spans.push(Span::styled(
                            display_text.to_string(),
                            Style::default().bg(Color::Red).fg(Color::White).add_modifier(Modifier::BOLD).add_modifier(Modifier::UNDERLINED)
                        ));
                    } else {
                        current_line_spans.push(Span::styled(
                            display_text.to_string(),
                            Style::default().bg(Color::Red).fg(Color::White).add_modifier(Modifier::BOLD)
                        ));
                    }
                } else {
                    if idx == chars_to_show.len() - 1 {
                        // Last error character gets underline cursor
                        current_line_spans.push(Span::styled(
                            user_char.to_string(),
                            Style::default().bg(Color::Red).fg(Color::White).add_modifier(Modifier::BOLD).add_modifier(Modifier::UNDERLINED)
                        ));
                    } else {
                        current_line_spans.push(Span::styled(
                            user_char.to_string(),
                            Style::default().bg(Color::Red).fg(Color::White).add_modifier(Modifier::BOLD)
                        ));
                    }
                }
            }
        }

        // Display remaining target text in gray
        let start_pos = if self.has_error {
            (self.current_position + self.consecutive_errors).min(target_chars.len())
        } else {
            self.current_position
        };

        for i in start_pos..target_chars.len() {
            let ch = target_chars[i];

            if ch == '\n' {
                lines.push(Line::from(current_line_spans.clone()));
                current_line_spans.clear();
            } else if ch == '\t' {
                // Convert tab to 4 spaces in remaining text
                current_line_spans.push(Span::styled(
                    "    ".to_string(), // 4 spaces
                    Style::default().fg(Color::DarkGray)
                ));
            } else {
                current_line_spans.push(Span::styled(
                    ch.to_string(),
                    Style::default().fg(Color::DarkGray)
                ));
            }
        }

        // Add cursor at the end if we've typed everything without errors
        if self.current_position >= target_chars.len() && !self.has_error {
            current_line_spans.push(Span::styled("|".to_string(),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)));
        }

        // Add the final line if it has content
        if !current_line_spans.is_empty() {
            lines.push(Line::from(current_line_spans));
        }


        lines
    }

    pub fn generate_report(&self) -> SessionReport {
        let session_duration = if let Some(end_time) = self.session_end {
            end_time.duration_since(self.session_start)
        } else {
            self.session_start.elapsed()
        };
        
        let total_latency: Duration = self.key_stats.values()
            .map(|stat| stat.total_latency)
            .sum();
        let total_keys: u32 = self.key_stats.values()
            .map(|stat| stat.count)
            .sum();
        
        let average_latency = if total_keys > 0 {
            total_latency / total_keys
        } else {
            Duration::from_millis(0)
        };

        SessionReport {
            session_duration,
            total_characters: self.user_input.len(),
            correct_characters: self.current_position,
            wpm: self.calculate_wpm_with_duration(session_duration),
            accuracy: self.calculate_accuracy(),
            average_latency,
            errors: self.errors.clone(),
            key_stats: self.key_stats.clone(),
            total_corrections: self.total_corrections,
            average_correction_latency: None,
            typing_rhythm: self.typing_rhythm.clone(),
            hesitation_patterns: self.hesitation_patterns.clone(),
            weakness_analysis: self.analyze_weaknesses(),
            wpm_over_time: self.wpm_samples.iter()
                .map(|(instant, wpm)| (instant.duration_since(self.session_start), *wpm))
                .collect(),
        }
    }

    fn calculate_wpm_with_duration(&self, duration: Duration) -> f64 {
        let elapsed_minutes = duration.as_secs_f64() / 60.0;
        if elapsed_minutes == 0.0 {
            0.0
        } else {
            (self.current_position as f64 / 5.0) / elapsed_minutes
        }
    }
    
    fn analyze_weaknesses(&self) -> WeaknessAnalysis {
        // Analyze slowest digraphs
        let mut digraph_latencies: HashMap<String, Vec<u64>> = HashMap::new();
        for rhythm in &self.typing_rhythm {
            if rhythm.position > 0 {
                if let Some(prev_char) = self.target_text.chars().nth(rhythm.position - 1) {
                    let digraph = format!("{}{}", prev_char, rhythm.char_typed);
                    digraph_latencies.entry(digraph)
                        .or_insert_with(Vec::new)
                        .push(rhythm.latency.as_millis() as u64);
                }
            }
        }
        
        let mut slowest_digraphs: Vec<(String, f64)> = digraph_latencies
            .into_iter()
            .filter(|(_, latencies)| latencies.len() >= 2) // Only consider repeated digraphs
            .map(|(digraph, latencies)| {
                let avg = latencies.iter().sum::<u64>() as f64 / latencies.len() as f64;
                (digraph, avg)
            })
            .collect();
        slowest_digraphs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        slowest_digraphs.truncate(10);
        
        // Identify error clusters (groups of errors within 10 characters)
        let mut error_clusters = Vec::new();
        let mut current_cluster_start = None;
        let mut last_error_pos = None;
        
        for error in &self.errors {
            if let Some(last_pos) = last_error_pos {
                if error.position <= last_pos + 10 {
                    // Continue current cluster
                } else {
                    // End current cluster and start new one
                    if let Some(start) = current_cluster_start {
                        error_clusters.push((start, last_pos));
                    }
                    current_cluster_start = Some(error.position);
                }
            } else {
                current_cluster_start = Some(error.position);
            }
            last_error_pos = Some(error.position);
        }
        
        if let (Some(start), Some(end)) = (current_cluster_start, last_error_pos) {
            error_clusters.push((start, end));
        }
        
        // Analyze finger assignment errors (simplified QWERTY layout)
        let finger_map = self.create_finger_map();
        let mut finger_errors: HashMap<String, u32> = HashMap::new();
        
        for error in &self.errors {
            if let (Some(expected), Some(actual)) = (error.expected_char, error.actual_char) {
                let unknown = "Unknown".to_string();
                let expected_finger = finger_map.get(&expected).unwrap_or(&unknown);
                let actual_finger = finger_map.get(&actual).unwrap_or(&unknown);
                
                if expected_finger != actual_finger {
                    let error_pattern = format!("{} -> {}", expected_finger, actual_finger);
                    *finger_errors.entry(error_pattern).or_insert(0) += 1;
                }
            }
        }
        
        // Detect rhythm breaks (sudden increases in latency)
        let mut rhythm_breaks = Vec::new();
        let latencies: Vec<u64> = self.typing_rhythm.iter()
            .map(|r| r.latency.as_millis() as u64)
            .collect();
        
        if latencies.len() > 5 {
            for i in 5..latencies.len() {
                let moving_avg = latencies[i-5..i].iter().sum::<u64>() / 5;
                if latencies[i] > moving_avg * 2 && latencies[i] > 400 {
                    rhythm_breaks.push(self.typing_rhythm[i].position);
                }
            }
        }
        
        // Analyze problematic transitions
        let mut transition_latencies: HashMap<(char, char), Vec<u64>> = HashMap::new();
        for i in 1..self.typing_rhythm.len() {
            let prev_char = self.typing_rhythm[i-1].char_typed;
            let curr_char = self.typing_rhythm[i].char_typed;
            let latency = self.typing_rhythm[i].latency.as_millis() as u64;
            
            transition_latencies.entry((prev_char, curr_char))
                .or_insert_with(Vec::new)
                .push(latency);
        }
        
        let mut problematic_transitions: Vec<(char, char, f64)> = transition_latencies
            .into_iter()
            .filter(|(_, latencies)| latencies.len() >= 2)
            .map(|((from, to), latencies)| {
                let avg = latencies.iter().sum::<u64>() as f64 / latencies.len() as f64;
                (from, to, avg)
            })
            .filter(|(_, _, avg)| *avg > 300.0) // Only slow transitions
            .collect();
        problematic_transitions.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
        problematic_transitions.truncate(10);
        
        WeaknessAnalysis {
            slowest_digraphs,
            error_clusters,
            finger_errors,
            rhythm_breaks,
            problematic_transitions,
        }
    }
    
    fn create_finger_map(&self) -> HashMap<char, String> {
        let mut map = HashMap::new();
        
        // Left hand
        map.insert('q', "L-Pinky".to_string());
        map.insert('w', "L-Ring".to_string());
        map.insert('e', "L-Middle".to_string());
        map.insert('r', "L-Index".to_string());
        map.insert('t', "L-Index".to_string());
        map.insert('a', "L-Pinky".to_string());
        map.insert('s', "L-Ring".to_string());
        map.insert('d', "L-Middle".to_string());
        map.insert('f', "L-Index".to_string());
        map.insert('g', "L-Index".to_string());
        map.insert('z', "L-Pinky".to_string());
        map.insert('x', "L-Ring".to_string());
        map.insert('c', "L-Middle".to_string());
        map.insert('v', "L-Index".to_string());
        map.insert('b', "L-Index".to_string());
        
        // Right hand
        map.insert('y', "R-Index".to_string());
        map.insert('u', "R-Index".to_string());
        map.insert('i', "R-Middle".to_string());
        map.insert('o', "R-Ring".to_string());
        map.insert('p', "R-Pinky".to_string());
        map.insert('h', "R-Index".to_string());
        map.insert('j', "R-Index".to_string());
        map.insert('k', "R-Middle".to_string());
        map.insert('l', "R-Ring".to_string());
        map.insert('n', "R-Index".to_string());
        map.insert('m', "R-Index".to_string());
        
        // Thumbs
        map.insert(' ', "Thumb".to_string());
        
        map
    }
}

#[derive(Debug, Clone)]
enum TextSource {
    File(String, String), // (filename, content)
    Inception(String),    // source code content
}

struct App {
    session: Option<TypingSession>,
    text_source: TextSource,
    should_quit: bool,
    state: AppState,
    report_view: ReportView,
}

impl ChunkSize {
    fn get_char_range(&self) -> (usize, usize) {
        match self {
            ChunkSize::Small => (800, 1600),
            ChunkSize::Medium => (1600, 3200),
            ChunkSize::Large => (3200, 4800),
        }
    }

    fn get_line_range(&self) -> (usize, usize) {
        match self {
            ChunkSize::Small => (20, 40),
            ChunkSize::Medium => (40, 80),
            ChunkSize::Large => (80, 120),
        }
    }
}

#[derive(Debug)]
struct TextParagraph {
    content: String,
    char_count: usize,
    score: f32,
}

impl TextSource {
    fn load_from_file(path: &Path, size: ChunkSize) -> io::Result<Self> {
        let content = fs::read_to_string(path)?;
        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let processed_content = Self::extract_file_snippet(&content, &filename, size);
        Ok(TextSource::File(filename, processed_content))
    }

    fn extract_file_snippet(content: &str, filename: &str, size: ChunkSize) -> String {
        let (target_min_chars, target_max_chars) = size.get_char_range();

        // Find all meaningful paragraphs/sections
        let mut paragraphs = Self::find_paragraphs(content, filename);

        // Score paragraphs strategically (higher score = better for typing practice)
        for paragraph in &mut paragraphs {
            paragraph.score = Self::calculate_paragraph_score(&paragraph.content, filename);
        }

        // Sort by score (best first) then randomize within score tiers
        paragraphs.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        // Filter paragraphs that fit within the size constraints
        let suitable_paragraphs: Vec<_> = paragraphs.iter()
            .filter(|p| p.char_count >= target_min_chars && p.char_count <= target_max_chars)
            .collect();

        // If we have suitable paragraphs, pick strategically with randomness
        if !suitable_paragraphs.is_empty() {
            let selected = Self::select_strategic_paragraph(&suitable_paragraphs);

            return selected.content.trim().to_string();
        }

        // If no perfect fit, find the best-scoring paragraph that's still meaningful
        let acceptable_paragraphs: Vec<_> = paragraphs.iter()
            .filter(|p| p.char_count >= target_min_chars / 2) // At least half the target
            .collect();

        if !acceptable_paragraphs.is_empty() {
            let selected = Self::select_strategic_paragraph(&acceptable_paragraphs);

            return selected.content.trim().to_string();
        }

        // Fallback: create a chunk of the target size from the middle of the file
        let lines: Vec<&str> = content.lines().collect();
        let (_target_lines_min, target_lines_max) = size.get_line_range();
        let start_idx = lines.len() / 3; // Start from 1/3 into the file
        let end_idx = (start_idx + target_lines_max).min(lines.len());
        let snippet_lines = &lines[start_idx..end_idx];
        let content_str = snippet_lines.join("\n");

        content_str.trim().to_string()
    }

    fn find_paragraphs(content: &str, filename: &str) -> Vec<TextParagraph> {
        let lines: Vec<&str> = content.lines().collect();
        let mut paragraphs = Vec::new();

        // For code files, find function/struct/impl blocks
        if filename.ends_with(".rs") || filename.ends_with(".py") ||
           filename.ends_with(".js") || filename.ends_with(".ts") ||
           filename.ends_with(".cpp") || filename.ends_with(".c") ||
           filename.ends_with(".java") || filename.ends_with(".go") {

            let mut current_start = 0;
            let mut brace_depth = 0;
            let mut in_block = false;

            for (i, line) in lines.iter().enumerate() {
                let trimmed = line.trim();

                // Detect start of code blocks
                if (trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") ||
                    trimmed.starts_with("struct ") || trimmed.starts_with("impl ") ||
                    trimmed.starts_with("enum ") || trimmed.starts_with("class ") ||
                    trimmed.starts_with("def ") || trimmed.starts_with("function ")) &&
                   brace_depth == 0 {
                    current_start = i;
                    in_block = true;
                }

                // Track braces
                brace_depth += line.matches('{').count() as i32;
                brace_depth -= line.matches('}').count() as i32;

                // End of block
                if in_block && brace_depth == 0 && line.contains('}') {
                    let block_lines = &lines[current_start..=i];
                    let content = block_lines.join("\n");
                    if content.len() > 200 { // Only meaningful blocks
                        paragraphs.push(TextParagraph {
                            content,
                            char_count: block_lines.join("\n").len(),
                            score: 0.0, // Will be calculated later
                        });
                    }
                    in_block = false;
                }
            }
        } else {
            // For text files, split by double newlines (paragraphs)
            let content_str = content.to_string();

            for paragraph_text in content_str.split("\n\n") {
                if paragraph_text.trim().len() > 100 { // Only meaningful paragraphs
                    paragraphs.push(TextParagraph {
                        content: paragraph_text.to_string(),
                        char_count: paragraph_text.len(),
                        score: 0.0, // Will be calculated later
                    });
                }
            }
        }

        paragraphs
    }

    fn calculate_paragraph_score(content: &str, filename: &str) -> f32 {
        let mut score = 0.0f32;

        // Base score from content length (sweet spot around 100-200 chars per complexity)
        let len = content.len() as f32;
        score += if len > 50.0 && len < 500.0 { 10.0 } else { 5.0 };

        // Bonus for diverse character usage (good for typing practice)
        let unique_chars = content.chars().collect::<std::collections::HashSet<_>>().len() as f32;
        score += unique_chars * 0.5;

        // Code-specific scoring
        if filename.ends_with(".rs") || filename.ends_with(".py") ||
           filename.ends_with(".js") || filename.ends_with(".ts") ||
           filename.ends_with(".cpp") || filename.ends_with(".java") {

            // Bonus for function implementations (good typing practice)
            if content.contains("fn ") || content.contains("function ") || content.contains("def ") {
                score += 15.0;
            }

            // Bonus for control structures (interesting patterns)
            if content.contains("if ") || content.contains("for ") || content.contains("while ") ||
               content.contains("match ") || content.contains("switch ") {
                score += 10.0;
            }

            // Bonus for data structures
            if content.contains("struct ") || content.contains("class ") || content.contains("enum ") {
                score += 12.0;
            }

            // Bonus for error handling (challenging typing)
            if content.contains("Result") || content.contains("Option") || content.contains("Error") ||
               content.contains("try") || content.contains("catch") || content.contains("except") {
                score += 8.0;
            }

            // Bonus for generics and advanced syntax (very good practice)
            if content.contains('<') && content.contains('>') || content.contains("impl ") {
                score += 12.0;
            }

            // Penalty for mostly comments or too simple
            let comment_ratio = content.lines()
                .filter(|line| line.trim().starts_with("//") || line.trim().starts_with("/*") || line.trim().starts_with("#"))
                .count() as f32 / content.lines().count().max(1) as f32;
            score -= comment_ratio * 10.0;

            // Penalty for too many imports/includes (boring)
            if content.contains("import ") || content.contains("use ") || content.contains("#include") {
                let import_lines = content.lines()
                    .filter(|line| line.contains("import ") || line.contains("use ") || line.contains("#include"))
                    .count();
                if import_lines > 3 {
                    score -= 5.0;
                }
            }

        } else {
            // Text file scoring
            let word_count = content.split_whitespace().count() as f32;

            // Bonus for good paragraph length
            if word_count > 20.0 && word_count < 150.0 {
                score += 10.0;
            }

            // Bonus for punctuation variety (good typing practice)
            let punct_chars = content.chars()
                .filter(|c| ".,;:!?\"'()-[]{}".contains(*c))
                .count() as f32;
            score += punct_chars * 0.3;

            // Bonus for sentences (complete thoughts)
            let sentence_count = content.matches(|c| ".!?".contains(c)).count() as f32;
            score += sentence_count * 2.0;
        }

        // Avoid very short or very long content
        if len < 100.0 {
            score -= 5.0;
        }
        if len > 1000.0 {
            score -= 3.0;
        }

        // Ensure non-negative score
        score.max(0.0)
    }

    fn select_strategic_paragraph<'a>(paragraphs: &'a [&'a TextParagraph]) -> &'a TextParagraph {
        use rand::seq::SliceRandom;
        use rand::thread_rng;

        paragraphs.choose(&mut thread_rng()).unwrap()
    }

    fn load_inception(size: ChunkSize) -> io::Result<Self> {
        // Include the source code directly at compile time
        let full_content = include_str!("main.rs");

        // Split into meaningful code sections and select one based on size
        let snippet = Self::extract_code_section(full_content, size);
        Ok(TextSource::Inception(snippet))
    }

    fn extract_code_section(content: &str, size: ChunkSize) -> String {
        let (target_min_chars, target_max_chars) = size.get_char_range();

        // Use the same strategic paragraph logic for the source code
        let mut paragraphs = Self::find_paragraphs(content, "main.rs");

        // Score paragraphs strategically (higher score = better for typing practice)
        for paragraph in &mut paragraphs {
            paragraph.score = Self::calculate_paragraph_score(&paragraph.content, "main.rs");
        }

        // Sort by score (best first)
        paragraphs.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        // Filter paragraphs that fit the size requirement
        let suitable_paragraphs: Vec<_> = paragraphs.iter()
            .filter(|p| p.char_count >= target_min_chars && p.char_count <= target_max_chars)
            .collect();

        if !suitable_paragraphs.is_empty() {
            let selected = Self::select_strategic_paragraph(&suitable_paragraphs);
            return selected.content.trim().to_string();
        }

        // If no perfect fit, find the best available paragraph
        let acceptable_paragraphs: Vec<_> = paragraphs.iter()
            .filter(|p| p.char_count >= target_min_chars / 2)
            .collect();

        if !acceptable_paragraphs.is_empty() {
            let selected = Self::select_strategic_paragraph(&acceptable_paragraphs);
            return selected.content.trim().to_string();
        }

        // Fallback: use a chunk from the beginning
        let lines: Vec<&str> = content.lines().collect();
        let (_, target_max_lines) = size.get_line_range();
        let end = target_max_lines.min(lines.len());
        let content_str = lines[0..end].join("\n");

        content_str.trim().to_string()
    }

    fn get_content(&self) -> Option<(String, String)> {
        match self {
            TextSource::File(name, content) => {
                Some((name.clone(), content.clone()))
            }
            TextSource::Inception(content) => {
                Some(("main.rs (INCEPTION MODE)".to_string(), content.clone()))
            }
        }
    }

}


impl App {
    fn new(text_source: TextSource) -> io::Result<Self> {
        let mut app = Self {
            session: None,
            text_source,
            should_quit: false,
            state: AppState::Typing,
            report_view: ReportView::Charts,
        };

        // Immediately start typing session
        app.start_typing_session();

        Ok(app)
    }
    
    fn start_typing_session(&mut self) {
        if let Some((_, content)) = self.text_source.get_content() {
            self.session = Some(TypingSession::new(content));
            self.state = AppState::Typing;
        }
    }

    fn handle_event(&mut self, event: Event) -> io::Result<()> {
        if let Event::Key(key) = event {
            match self.state {
                AppState::Typing => {
                    if let Some(session) = &mut self.session {
                        match key.code {
                            KeyCode::Char('q') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                                self.should_quit = true;
                            }
                            KeyCode::Char(c) => {
                                session.handle_key(c);
                                if session.is_complete() {
                                    self.state = AppState::ShowingReport;
                                }
                            }
                            KeyCode::Enter => {
                                session.handle_key('\n');
                                if session.is_complete() {
                                    self.state = AppState::ShowingReport;
                                }
                            }
                            KeyCode::Tab => {
                                // Send 4 individual space characters for tab
                                for _ in 0..4 {
                                    session.handle_key(' ');
                                    if session.is_complete() {
                                        break;
                                    }
                                }
                                if session.is_complete() {
                                    self.state = AppState::ShowingReport;
                                }
                            }
                            KeyCode::Backspace => {
                                session.handle_key('\x08');
                            }
                            _ => {}
                        }
                    }
                }
                AppState::ShowingReport => {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                            self.should_quit = true;
                        }
                        KeyCode::Char('e') => {
                            self.export_report()?;
                        }
                        KeyCode::Char('r') => {
                            self.start_typing_session();
                        }
                        KeyCode::Left | KeyCode::Char('h') => {
                            self.report_view = match self.report_view {
                                ReportView::Charts => ReportView::Analysis,
                                ReportView::Analysis => ReportView::Charts,
                            };
                        }
                        KeyCode::Right | KeyCode::Char('l') => {
                            self.report_view = match self.report_view {
                                ReportView::Charts => ReportView::Analysis,
                                ReportView::Analysis => ReportView::Charts,
                            };
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    fn export_report(&self) -> io::Result<()> {
        if let Some(session) = &self.session {
            let report = session.generate_report();
            let json = serde_json::to_string_pretty(&report)?;
            let filename = format!("typing_report_{}.json", 
                chrono::Utc::now().format("%Y%m%d_%H%M%S"));
            std::fs::write(&filename, json)?;
        }
        Ok(())
    }
}


fn ui_typing(f: &mut Frame, app: &App) {
    if let Some(session) = &app.session {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(2),
                Constraint::Length(3),
            ])
            .split(f.area());

        // Create horizontal layout for centering text in 80% width
        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(10), // Left padding
                Constraint::Percentage(80), // Text area (center 80%)
                Constraint::Percentage(10), // Right padding
            ])
            .split(chunks[0]);

        // Main typing area - centered text with styling
        let text_block = Block::default()
            .borders(Borders::NONE);
        
        let styled_lines = session.generate_styled_text();
        let paragraph = Paragraph::new(styled_lines)
            .block(text_block)
            .wrap(Wrap { trim: false })
            .alignment(Alignment::Left);
        
        f.render_widget(paragraph, horizontal_chunks[1]);

        // Status message
        let status_color = if session.is_frozen {
            Color::Red
        } else if session.has_error {
            Color::Yellow
        } else {
            Color::Green
        };
        
        let status = Paragraph::new(session.get_status())
            .alignment(Alignment::Center)
            .style(Style::default().fg(status_color));
        f.render_widget(status, chunks[1]);

        // Simple help text at bottom
        let help = Paragraph::new("Type the text above. Ctrl+Q: Back to selection")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(help, chunks[2]);
    }
}

fn ui_report(f: &mut Frame, app: &App) {
    if let Some(session) = &app.session {
        let report = session.generate_report();
    
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(f.area());

        // Title with view indicator
        let view_name = match app.report_view {
            ReportView::Charts => "Visual Analysis",
            ReportView::Analysis => "Detailed Insights",
        };
        let title = Paragraph::new(format!("Typing Session Complete! - {}", view_name))
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD));
        f.render_widget(title, chunks[0]);

        // Render different views based on report_view
        match app.report_view {
            ReportView::Charts => render_consolidated_charts_view(f, chunks[1], &report),
            ReportView::Analysis => render_consolidated_analysis_view(f, chunks[1], &report),
        }

        // Help
        let help = Paragraph::new("Left/Right: Switch views  'e': Export  'r': Retry  'q': Back")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(help, chunks[2]);
    }
}

fn render_consolidated_charts_view(f: &mut Frame, area: ratatui::layout::Rect, report: &SessionReport) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(75), Constraint::Percentage(25)])
        .split(area);

    let chart_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(25), // Summary stats
            Constraint::Percentage(35), // Key frequency and errors
            Constraint::Percentage(40), // Error timeline and hesitation
        ])
        .split(main_chunks[0]);

    // Summary stats bar
    let stats_text = format!(
        "WPM: {:.1} | Accuracy: {:.1}% | Errors: {} | Duration: {:.1}s | Avg Latency: {}ms",
        report.wpm,
        report.accuracy,
        report.errors.len(),
        report.session_duration.as_secs_f64(),
        report.average_latency.as_millis()
    );

    let stats = Paragraph::new(stats_text)
        .block(Block::default().title("Session Summary").borders(Borders::ALL))
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD));
    f.render_widget(stats, chart_chunks[0]);

    // Key charts - top row
    let key_charts = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chart_chunks[1]);

    // Most frequent keys chart
    let mut key_data: Vec<_> = report.key_stats.iter()
        .map(|(key, stats)| {
            let display_key = if *key == ' ' { "Space".to_string() } else { key.to_string() };
            (display_key, stats.count as u64)
        })
        .collect();
    key_data.sort_by(|a, b| b.1.cmp(&a.1));
    key_data.truncate(8);

    let key_chart_data: Vec<_> = key_data.iter()
        .map(|(key, count)| (key.as_str(), *count))
        .collect();

    let key_chart = BarChart::default()
        .block(Block::default().title("Most Used Keys").borders(Borders::ALL))
        .data(&key_chart_data)
        .bar_width(3)
        .bar_style(Style::default().fg(Color::Green))
        .value_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
    f.render_widget(key_chart, key_charts[0]);

    // Error-prone keys chart
    let mut error_data: Vec<_> = report.key_stats.iter()
        .filter(|(_, stats)| stats.error_count > 0)
        .map(|(key, stats)| {
            let display_key = if *key == ' ' { "Space".to_string() } else { key.to_string() };
            (display_key, stats.error_count as u64)
        })
        .collect();
    error_data.sort_by(|a, b| b.1.cmp(&a.1));
    error_data.truncate(8);

    if !error_data.is_empty() {
        let error_chart_data: Vec<_> = error_data.iter()
            .map(|(key, count)| (key.as_str(), *count))
            .collect();

        let error_chart = BarChart::default()
            .block(Block::default().title("Error-Prone Keys").borders(Borders::ALL))
            .data(&error_chart_data)
            .bar_width(3)
            .bar_style(Style::default().fg(Color::Red))
            .value_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
        f.render_widget(error_chart, key_charts[1]);
    } else {
        let no_errors = Paragraph::new("No errors! Perfect typing!")
            .block(Block::default().title("Error-Prone Keys").borders(Borders::ALL))
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Green));
        f.render_widget(no_errors, key_charts[1]);
    }

    // Bottom charts row
    let bottom_charts = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chart_chunks[2]);

    // Error timeline
    if !report.errors.is_empty() {
        let timeline_text = report.errors.iter()
            .take(10)
            .enumerate()
            .map(|(i, error)| {
                let timestamp = error.timestamp.as_secs_f64();
                format!("{}. {:.1}s: {} '{}' -> '{}'", 
                    i + 1, 
                    timestamp,
                    match error.error_type {
                        ErrorType::Substitution => "Sub",
                        ErrorType::Insertion => "Ins",
                        ErrorType::Omission => "Omi",
                        ErrorType::Repeat => "Rep",
                    },
                    error.expected_char.unwrap_or('?'),
                    error.actual_char.unwrap_or('?')
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let timeline = Paragraph::new(timeline_text)
            .block(Block::default().title("Error Timeline").borders(Borders::ALL))
            .wrap(Wrap { trim: true });
        f.render_widget(timeline, bottom_charts[0]);
    } else {
        let no_errors = Paragraph::new("No errors recorded!\nPerfect session!")
            .block(Block::default().title("Error Timeline").borders(Borders::ALL))
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Green));
        f.render_widget(no_errors, bottom_charts[0]);
    }

    // Hesitation visualization
    let hesitation_text = if report.hesitation_patterns.is_empty() {
        "No significant hesitations!\nGood rhythm maintained.".to_string()
    } else {
        report.hesitation_patterns.iter()
            .take(8)
            .map(|h| {
                format!("{}ms at pos {} ({})", 
                    h.duration.as_millis(),
                    h.position,
                    match h.pattern_type {
                        HesitationType::LongPause => "Long Pause",
                        HesitationType::DoubleDigraph => "Common Combo",
                        HesitationType::Transition => "Hand Switch",
                        HesitationType::Punctuation => "Punctuation",
                        HesitationType::CaseChange => "Case Change",
                        HesitationType::NumberSymbol => "Number/Symbol",
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let hesitation = Paragraph::new(hesitation_text)
        .block(Block::default().title("Hesitation Patterns").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(hesitation, bottom_charts[1]);

    // Educational sidebar
    let education_text = "VISUAL ANALYSIS GUIDE\n\n\
        📊 WHAT YOU'RE SEEING:\n\
        • Key Usage: Shows which keys you type most\n\
        • Error Patterns: Reveals problem keys\n\
        • Error Timeline: When mistakes occur\n\
        • Hesitation Points: Where you slow down\n\n\
        🎯 HOW THIS HELPS:\n\
        • Identify weak finger positions\n\
        • Spot rhythm disruption patterns\n\
        • Focus practice on problem areas\n\
        • Track improvement over time\n\n\
        💡 ACTION ITEMS:\n\
        • Practice error-prone keys separately\n\
        • Work on smooth transitions\n\
        • Build muscle memory for hesitation points\n\
        • Maintain consistent rhythm\n\n\
        Switch to 'Detailed Insights' →\n\
        for specific recommendations";

    let education = Paragraph::new(education_text)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().padding(Padding::uniform(2)));
    f.render_widget(education, main_chunks[1]);
}

fn render_consolidated_analysis_view(f: &mut Frame, area: ratatui::layout::Rect, report: &SessionReport) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(75), Constraint::Percentage(25)])
        .split(area);

    let analysis_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(20), // Key stats
            Constraint::Percentage(25), // Slowest digraphs and finger errors
            Constraint::Percentage(25), // Weakness summary and rhythm
            Constraint::Percentage(30), // Detailed recommendations
        ])
        .split(main_chunks[0]);

    // Key performance metrics
    let total_latency: Duration = report.key_stats.values()
        .map(|stat| stat.total_latency)
        .sum();
    let total_keys: u32 = report.key_stats.values()
        .map(|stat| stat.count)
        .sum();
    let avg_latency = if total_keys > 0 { 
        total_latency.as_millis() / total_keys as u128 
    } else { 0 };

    let metrics_text = format!(
        "PERFORMANCE METRICS\n\
         • Speed: {:.1} WPM (Target: 40+ WPM)\n\
         • Accuracy: {:.1}% (Target: 95%+)\n\
         • Consistency: {}ms avg latency\n\
         • Error Rate: {:.2}% (Target: <2%)\n\
         • Rhythm Stability: {} breaks detected",
        report.wpm,
        report.accuracy,
        avg_latency,
        (report.errors.len() as f64 / report.total_characters as f64) * 100.0,
        report.weakness_analysis.rhythm_breaks.len()
    );

    let metrics = Paragraph::new(metrics_text)
        .block(Block::default().title("📈 Performance Overview").borders(Borders::ALL))
        .style(Style::default().fg(Color::Green));
    f.render_widget(metrics, analysis_chunks[0]);

    // Weakness analysis - top row
    let weakness_top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(analysis_chunks[1]);

    // Slowest digraphs
    let digraph_text = if report.weakness_analysis.slowest_digraphs.is_empty() {
        "✅ No problematic letter combinations found!\nAll transitions are smooth.".to_string()
    } else {
        let mut text = "⚠️  SLOW LETTER COMBINATIONS:\n".to_string();
        for (digraph, avg_ms) in report.weakness_analysis.slowest_digraphs.iter().take(6) {
            text.push_str(&format!("• '{}': {:.0}ms\n", digraph, avg_ms));
        }
        text.push_str("\nFocus practice on these pairs!");
        text
    };

    let digraphs = Paragraph::new(digraph_text)
        .block(Block::default().title("🔤 Letter Combinations").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(digraphs, weakness_top[0]);

    // Finger positioning errors
    let finger_text = if report.weakness_analysis.finger_errors.is_empty() {
        "✅ Perfect finger positioning!\n\nNo cross-finger errors detected.\n\n\n".to_string()
    } else {
        let mut text = "⚠️  FINGER POSITIONING ERRORS:\n".to_string();
        let errors: Vec<_> = report.weakness_analysis.finger_errors.iter().take(5).collect();
        for (pattern, count) in &errors {
            text.push_str(&format!("• {}: {} times\n", pattern, count));
        }
        // Add padding lines to maintain consistent height
        for _ in errors.len()..5 {
            text.push('\n');
        }
        text.push_str("Practice proper finger placement!");
        text
    };

    let fingers = Paragraph::new(finger_text)
        .block(Block::default().title("👆 Finger Analysis").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(fingers, weakness_top[1]);

    // Analysis middle row
    let weakness_mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(analysis_chunks[2]);

    // Error clustering
    let cluster_text = if report.weakness_analysis.error_clusters.is_empty() {
        "✅ No error clustering detected!\nMistakes are well-distributed.".to_string()
    } else {
        let mut text = "⚠️  ERROR CLUSTERS FOUND:\n".to_string();
        for (start, end) in &report.weakness_analysis.error_clusters {
            text.push_str(&format!("• Positions {}-{} ({} chars)\n", start, end, end - start + 1));
        }
        text.push_str("\nThese sections need extra practice!");
        text
    };

    let clusters = Paragraph::new(cluster_text)
        .block(Block::default().title("🎯 Error Hotspots").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(clusters, weakness_mid[0]);

    // Rhythm analysis
    let rhythm_text = if report.weakness_analysis.rhythm_breaks.is_empty() {
        "✅ Excellent rhythm consistency!\n\nSteady typing pace maintained.\n\n\n".to_string()
    } else {
        let break_count = report.weakness_analysis.rhythm_breaks.len();
        let positions = if break_count <= 8 {
            report.weakness_analysis.rhythm_breaks.iter()
                .map(|pos| pos.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            format!("{} (and {} more)", 
                report.weakness_analysis.rhythm_breaks.iter()
                    .take(6)
                    .map(|pos| pos.to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
                break_count - 6
            )
        };
        
        format!("⚠️  RHYTHM DISRUPTIONS:\n\
                 • {} sudden slowdowns detected\n\
                 • Positions: {}\n\n\
                 Work on maintaining steady pace!",
                break_count, positions
        )
    };

    let rhythm = Paragraph::new(rhythm_text)
        .block(Block::default().title("⏱️  Rhythm Analysis").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(rhythm, weakness_mid[1]);

    // Detailed recommendations
    let mut recommendations = "🎯 PERSONALIZED IMPROVEMENT PLAN:\n\n".to_string();
    
    if report.wpm < 30.0 {
        recommendations.push_str("🐌 SPEED: Focus on accuracy first, then gradually increase pace\n");
    } else if report.wpm < 50.0 {
        recommendations.push_str("🚶 SPEED: Good foundation! Work on consistent 40+ WPM\n");
    } else {
        recommendations.push_str("🏃 SPEED: Excellent! Maintain this pace while improving accuracy\n");
    }

    if report.accuracy < 90.0 {
        recommendations.push_str("❌ ACCURACY: Slow down and focus on correct keystrokes\n");
    } else if report.accuracy < 97.0 {
        recommendations.push_str("✓ ACCURACY: Good! Aim for 97%+ accuracy\n");
    } else {
        recommendations.push_str("🎯 ACCURACY: Excellent precision! Keep it up!\n");
    }

    if !report.weakness_analysis.slowest_digraphs.is_empty() {
        recommendations.push_str("🔤 PRACTICE: Drill slow letter combinations separately\n");
    }

    if !report.weakness_analysis.finger_errors.is_empty() {
        recommendations.push_str("👆 FINGERS: Review proper finger positioning\n");
    }

    if report.weakness_analysis.rhythm_breaks.len() > 3 {
        recommendations.push_str("⏱️  RHYTHM: Practice with metronome for consistency\n");
    }

    recommendations.push_str("\n💡 NEXT STEPS:\n\
                              1. Practice identified weak areas daily\n\
                              2. Use typing games for problem keys\n\
                              3. Maintain proper posture and hand position\n\
                              4. Take breaks to avoid fatigue");

    let rec_widget = Paragraph::new(recommendations)
        .block(Block::default().title("📋 Action Plan").borders(Borders::ALL))
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::Yellow));
    f.render_widget(rec_widget, analysis_chunks[3]);

    // Educational sidebar
    let education_text = "DETAILED INSIGHTS GUIDE\n\n\
        🔍 UNDERSTANDING THE DATA:\n\
        • Performance Metrics: Core stats vs targets\n\
        • Letter Combinations: Transition speed analysis\n\
        • Finger Analysis: Hand positioning accuracy\n\
        • Error Hotspots: Problem text sections\n\
        • Rhythm Analysis: Pace consistency check\n\n\
        📈 SKILL DEVELOPMENT:\n\
        • Focus on one weakness at a time\n\
        • Use targeted practice exercises\n\
        • Track progress over multiple sessions\n\
        • Build muscle memory gradually\n\n\
        🏆 IMPROVEMENT STRATEGY:\n\
        • Accuracy before speed\n\
        • Consistent practice beats marathon sessions\n\
        • Learn from each mistake\n\
        • Celebrate small improvements\n\n\
        ← Switch to 'Visual Analysis'\n\
        for charts and graphs";

    let education = Paragraph::new(education_text)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().padding(Padding::uniform(2)));
    f.render_widget(education, main_chunks[1]);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Determine the text source based on CLI arguments
    let text_source = if cli.inception {
        TextSource::load_inception(cli.size)?
    } else if let Some(file_path) = cli.file {
        TextSource::load_from_file(&file_path, cli.size)?
    } else {
        // Error: user must specify either --file or --inception
        eprintln!("Error: You must specify either --file <path> or --inception");
        eprintln!("Run with --help for usage information");
        std::process::exit(1);
    };

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(text_source)?;

    loop {
        terminal.draw(|f| {
            match app.state {
                AppState::Typing => ui_typing(f, &app),
                AppState::ShowingReport => ui_report(f, &app),
            }
        })?;

        if event::poll(Duration::from_millis(50))? {
            app.handle_event(event::read()?)?;
        }

        if app.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

