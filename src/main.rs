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
use std::{
    collections::HashMap,
    fs,
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

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
    TextSelection,
    Typing,
    ShowingReport,
}

#[derive(PartialEq)]
enum ReportView {
    Charts,
    Analysis,
}

#[derive(Debug, Clone)]
struct TextFile {
    name: String,
    #[allow(dead_code)]
    path: PathBuf,
    content: String,
}

#[derive(Debug, Clone)]
struct TextCategory {
    name: String,
    #[allow(dead_code)]
    path: PathBuf,
    files: Vec<TextFile>,
}

struct TextLibrary {
    categories: Vec<TextCategory>,
    selected_category: usize,
    selected_file: usize,
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

    pub fn generate_styled_text(&self) -> Line<'static> {
        let mut spans = Vec::new();
        let target_chars: Vec<char> = self.target_text.chars().collect();
        let user_chars: Vec<char> = self.user_input.chars().collect();
        
        // Display correctly typed characters in green
        for i in 0..self.current_position.min(target_chars.len()) {
            if i == self.current_position - 1 && !self.has_error && !self.is_frozen {
                // Last correctly typed character with cursor - green with underline
                spans.push(Span::styled(
                    target_chars[i].to_string(),
                    Style::default().fg(Color::Green).add_modifier(Modifier::UNDERLINED).add_modifier(Modifier::BOLD)
                ));
            } else {
                // Other correctly typed characters - green
                spans.push(Span::styled(
                    target_chars[i].to_string(),
                    Style::default().fg(Color::Green)
                ));
            }
        }
        
        // Display error buffer (incorrect characters typed beyond correct position)
        if self.has_error && user_chars.len() > self.current_position {
            for i in self.current_position..user_chars.len().min(self.current_position + 10) {
                let user_char = user_chars[i];
                if i == user_chars.len() - 1 {
                    // Last error character gets underline cursor
                    spans.push(Span::styled(
                        user_char.to_string(),
                        Style::default().bg(Color::Red).fg(Color::White).add_modifier(Modifier::BOLD).add_modifier(Modifier::UNDERLINED)
                    ));
                } else {
                    spans.push(Span::styled(
                        user_char.to_string(),
                        Style::default().bg(Color::Red).fg(Color::White).add_modifier(Modifier::BOLD)
                    ));
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
            spans.push(Span::styled(
                target_chars[i].to_string(),
                Style::default().fg(Color::DarkGray)
            ));
        }
        
        // Add cursor at the end if we've typed everything without errors
        if self.current_position >= target_chars.len() && !self.has_error {
            spans.push(Span::styled("|".to_string(), 
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)));
        }
        
        Line::from(spans)
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

struct App {
    session: Option<TypingSession>,
    text_library: TextLibrary,
    should_quit: bool,
    state: AppState,
    report_view: ReportView,
}

impl TextLibrary {
    fn new() -> io::Result<Self> {
        let mut categories = Vec::new();
        let texts_dir = Path::new("texts");
        
        if texts_dir.exists() {
            for entry in fs::read_dir(texts_dir)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.is_dir() {
                    let category_name = path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Unknown")
                        .to_string();
                    
                    let mut files = Vec::new();
                    
                    for file_entry in fs::read_dir(&path)? {
                        let file_entry = file_entry?;
                        let file_path = file_entry.path();
                        
                        if file_path.extension().and_then(|e| e.to_str()) == Some("txt") {
                            let file_name = file_path.file_stem()
                                .and_then(|n| n.to_str())
                                .unwrap_or("Unknown")
                                .replace('_', " ")
                                .split(' ')
                                .map(|word| {
                                    let mut chars = word.chars();
                                    match chars.next() {
                                        None => String::new(),
                                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join(" ");
                            
                            if let Ok(content) = fs::read_to_string(&file_path) {
                                files.push(TextFile {
                                    name: file_name,
                                    path: file_path,
                                    content: content.trim().to_string(),
                                });
                            }
                        }
                    }
                    
                    if !files.is_empty() {
                        categories.push(TextCategory {
                            name: category_name.replace('_', " ").split(' ')
                                .map(|word| {
                                    let mut chars = word.chars();
                                    match chars.next() {
                                        None => String::new(),
                                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join(" "),
                            path,
                            files,
                        });
                    }
                }
            }
        }
        
        Ok(Self {
            categories,
            selected_category: 0,
            selected_file: 0,
        })
    }
    
    fn get_selected_text(&self) -> Option<&TextFile> {
        self.categories
            .get(self.selected_category)
            .and_then(|cat| cat.files.get(self.selected_file))
    }
    
    fn next_category(&mut self) {
        if !self.categories.is_empty() {
            self.selected_category = (self.selected_category + 1) % self.categories.len();
            self.selected_file = 0;
        }
    }
    
    fn prev_category(&mut self) {
        if !self.categories.is_empty() {
            self.selected_category = if self.selected_category == 0 {
                self.categories.len() - 1
            } else {
                self.selected_category - 1
            };
            self.selected_file = 0;
        }
    }
    
    fn next_file(&mut self) {
        if let Some(category) = self.categories.get(self.selected_category) {
            if !category.files.is_empty() {
                self.selected_file = (self.selected_file + 1) % category.files.len();
            }
        }
    }
    
    fn prev_file(&mut self) {
        if let Some(category) = self.categories.get(self.selected_category) {
            if !category.files.is_empty() {
                self.selected_file = if self.selected_file == 0 {
                    category.files.len() - 1
                } else {
                    self.selected_file - 1
                };
            }
        }
    }
}

impl App {
    fn new() -> io::Result<Self> {
        let text_library = TextLibrary::new()?;
        Ok(Self {
            session: None,
            text_library,
            should_quit: false,
            state: AppState::TextSelection,
            report_view: ReportView::Charts,
        })
    }
    
    fn start_typing_session(&mut self) {
        if let Some(text_file) = self.text_library.get_selected_text() {
            self.session = Some(TypingSession::new(text_file.content.clone()));
            self.state = AppState::Typing;
        }
    }

    fn handle_event(&mut self, event: Event) -> io::Result<()> {
        if let Event::Key(key) = event {
            match self.state {
                AppState::TextSelection => {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                            self.should_quit = true;
                        }
                        KeyCode::Up => {
                            self.text_library.prev_category();
                        }
                        KeyCode::Down => {
                            self.text_library.next_category();
                        }
                        KeyCode::Left => {
                            self.text_library.prev_file();
                        }
                        KeyCode::Right => {
                            self.text_library.next_file();
                        }
                        KeyCode::Enter => {
                            self.start_typing_session();
                        }
                        _ => {}
                    }
                }
                AppState::Typing => {
                    if let Some(session) = &mut self.session {
                        match key.code {
                            KeyCode::Char('q') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                                self.state = AppState::TextSelection;
                            }
                            KeyCode::Char(c) => {
                                session.handle_key(c);
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
                            self.state = AppState::TextSelection;
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

fn ui_text_selection(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(f.area());

    // Title
    let title = Paragraph::new("Select Text to Type")
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD));
    f.render_widget(title, chunks[0]);

    // Categories and files
    let mut content = Vec::new();
    
    for (cat_idx, category) in app.text_library.categories.iter().enumerate() {
        let cat_style = if cat_idx == app.text_library.selected_category {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        
        content.push(Line::from(Span::styled(format!("[+] {}", category.name), cat_style)));
        
        if cat_idx == app.text_library.selected_category {
            for (file_idx, file) in category.files.iter().enumerate() {
                let file_style = if file_idx == app.text_library.selected_file {
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                
                let prefix = if file_idx == app.text_library.selected_file { "> " } else { "  " };
                content.push(Line::from(Span::styled(format!("{}{}", prefix, file.name), file_style)));
            }
        }
    }
    
    let selection = Paragraph::new(content)
        .block(Block::default().title("Text Library").borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    f.render_widget(selection, chunks[1]);

    // Help
    let help = Paragraph::new("Up/Down: Categories  Left/Right: Files  Enter: Start  Q: Quit")
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(help, chunks[2]);
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

        // Create horizontal layout for centering text in middle 50%
        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25), // Left padding
                Constraint::Percentage(50), // Text area (center 50%)
                Constraint::Percentage(25), // Right padding
            ])
            .split(chunks[0]);

        // Main typing area - centered text with styling
        let text_block = Block::default()
            .borders(Borders::NONE);
        
        let styled_text = session.generate_styled_text();
        let paragraph = Paragraph::new(styled_text)
            .block(text_block)
            .wrap(Wrap { trim: false })
            .alignment(Alignment::Center);
        
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
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new()?;

    loop {
        terminal.draw(|f| {
            match app.state {
                AppState::TextSelection => ui_text_selection(f, &app),
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

