use crate::audio::types::RepeatMode;
use crate::models::track::Track;
use fastrand::Rng;

#[derive(Debug, Clone, Default)]
pub struct PlaybackQueue {
    pub current: Option<Track>,
    pub items: Vec<Track>,
    pub history: Vec<Track>,
    pub shuffle: bool,
    pub repeat_mode: RepeatMode,
}

impl PlaybackQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_queue(&mut self, tracks: Vec<Track>, start_idx: usize) -> Option<Track> {
        if tracks.is_empty() {
            self.current = None;
            self.items.clear();
            return None;
        }

        let idx = start_idx.min(tracks.len() - 1);
        let current = tracks[idx].clone();
        let remaining = tracks[idx + 1..].to_vec();

        self.current = Some(current.clone());
        self.items = remaining;
        Some(current)
    }

    pub fn add_next(&mut self, track: Track) {
        self.items.insert(0, track);
    }

    pub fn add_tail(&mut self, track: Track) {
        self.items.push(track);
    }

    pub fn next_track(&mut self) -> Option<Track> {
        if self.repeat_mode == RepeatMode::One {
            return self.current.clone();
        }

        if let Some(curr) = self.current.take() {
            self.history.push(curr.clone());
            if self.repeat_mode == RepeatMode::All {
                self.items.push(curr);
            }
        }

        if self.items.is_empty() {
            return None;
        }

        let next = if self.shuffle && self.items.len() > 1 {
            let mut rng = Rng::new();
            let idx = rng.usize(..self.items.len());
            self.items.remove(idx)
        } else {
            self.items.remove(0)
        };

        self.current = Some(next.clone());
        Some(next)
    }

    pub fn previous_track(&mut self) -> Option<Track> {
        if let Some(prev) = self.history.pop() {
            if let Some(curr) = self.current.take() {
                self.items.insert(0, curr);
            }
            self.current = Some(prev.clone());
            Some(prev)
        } else {
            self.current.clone()
        }
    }

    pub fn toggle_shuffle(&mut self) -> bool {
        self.shuffle = !self.shuffle;
        self.shuffle
    }

    pub fn cycle_repeat(&mut self) -> RepeatMode {
        self.repeat_mode = self.repeat_mode.next();
        self.repeat_mode
    }

    pub fn clear(&mut self) {
        self.current = None;
        self.items.clear();
        self.history.clear();
    }
}
