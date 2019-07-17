#[macro_use]
extern crate stdweb;
extern crate web_sys;

use wasm_bindgen::prelude::*;

// A macro to provide `println!(..)`-style syntax for `console.log` logging.
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

/// A simple event sequence represented by a vector of strings
struct EventSequence {
    events: Vec<String>,
    idx: usize,
}

impl EventSequence {

    /// Create an event sequence from a string.
    pub fn from_string(input_line: String) -> Self {
        let mut seq = Vec::new();
        
        let iter = input_line.split_ascii_whitespace();
        
        for event in iter {
            seq.push(event.to_string());
        }

        EventSequence {
            events: seq,
            idx: 0,
        }
    }

    /// Update an existing sequence from a string.
    pub fn update_sequence(&mut self, input_line: String) {
        self.events.clear();

        let iter = input_line.split_ascii_whitespace();
        
        for event in iter {
            self.events.push(event.to_string());
        }

        if self.idx >= self.events.len() {
            self.idx = self.events.len() - 1;
        }
    }

    /// get the next event in the sequence
    pub fn get_next_event(&mut self) -> &String {
        if self.events.is_empty() {
            "~".to_string();
        }
        
        let cur_idx = self.idx;

        if self.idx + 1 == self.events.len() {
            self.idx = 0;
        } else {
            self.idx += 1;
        }
        
        &self.events[cur_idx]        
    }
}

/// A simple time-recursion event scheduler running at a fixed time interval.
#[wasm_bindgen]
pub struct Scheduler {
    /// time this scheduler was started (AudioContext.currentTime)
    audio_start_time: f64,
    /// time this scheduler was started (performance.now())
    browser_start_time: f64,    
    audio_logical_time: f64,
    browser_logical_time: f64,
    next_schedule_time: f64,
    lookahead: f64, // in seconds
    running: bool,
    tempo: f64, // currently just the duration of a 16th note ... 
    event_sequences: Vec<EventSequence>,
}

#[wasm_bindgen]
impl Scheduler {
    pub fn new() -> Self {
        Scheduler{
            audio_start_time: 0.0,
            browser_start_time: 0.0,
            audio_logical_time: 0.0,
            browser_logical_time: 0.0,
            next_schedule_time: 0.0,
            lookahead: 0.100,
            running: false,
            tempo: 128.0,
            event_sequences: Vec::new(),
        }
    }

    /// Evaluate an input string, turn it into a series of event sequences.
    pub fn evaluate(&mut self, input: Option<String>) {        
        match input {
            Some(all_lines) => {                                               
                let mut seq_idx = 0;

                for line in all_lines.lines() {
                    
                    if !line.trim().is_empty() {
                        if self.event_sequences.len() > seq_idx {
                            self.event_sequences[seq_idx].update_sequence(line.trim().to_string());
                        } else {
                            self.event_sequences.push(EventSequence::from_string(line.trim().to_string()));
                        }
                        seq_idx += 1;                        
                    }
                }
                // check if we need to remove some sequnces because the number of lines got reduced ...
                if seq_idx < self.event_sequences.len() {
                    self.event_sequences.truncate(seq_idx);
                }
            }
            
            None => log!("no input!")
        }
    }    

    /// Fetch all events from the event sequences, post them to main thread
    fn generate_and_send_events(&mut self) {
        if self.event_sequences.is_empty() {
            return
        }

        let trigger_time = self.audio_logical_time + self.lookahead;
        
        for seq in self.event_sequences.iter_mut() {

            let next_event = seq.get_next_event();

            let next_source_type = match next_event.as_ref() {
                "sine" => "SinOsc",
                _ => "Sampler",
            };

            if next_event != "~" {
                // post events that will be dispatched to sampler
                js! {                
                    postMessage( { source_type: @{ next_source_type }, timestamp: @{ trigger_time }, sample_id: @{ next_event} } );
                }
            }
        }
    }

    /// The main scheduler recursion.
    pub fn scheduler_routine(&mut self, browser_timestamp: f64) {
        if !self.running {
            return
        }

        // Get current events and post them to main thread.
        self.generate_and_send_events();

        // Calculate drift, correct timing.
        // The time at which this is called is most likely later, but never earlier,
        // than the time it SHOULD have been called at (self.browser_logical_time).
        // To compensate for the delay, we schedule the next call a bit earlier
        // than the actual interval.
        self.next_schedule_time = self.tempo - (browser_timestamp - self.browser_logical_time);

        // Advance timestamps!
        // audio time in seconds
        self.audio_logical_time += self.tempo / 1000.0;

        // browser time in milliseconds
        self.browser_logical_time += self.tempo;
        
        // Time-recursive call to scheduler function.
        // i'm looking forward to the day I can do that in pure rust ... 
        js! {            
            self.sleep( @{ self.next_schedule_time } ).then( () => self.scheduler.scheduler_routine( performance.now()));
        };                
    }

    /// Start this scheduler.
    pub fn start(&mut self, audio_timestamp: f64, browser_timestamp: f64) {
        self.audio_start_time = audio_timestamp;
        self.browser_start_time = browser_timestamp;
        self.audio_logical_time = self.audio_start_time;
        self.browser_logical_time = self.browser_start_time;
        self.running = true;
        self.scheduler_routine(browser_timestamp);
    }

    /// Stop this scheduler.
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Set tick duration.
    pub fn set_tempo(&mut self, tempo: f64) {
        self.tempo = tempo;
    }
}
