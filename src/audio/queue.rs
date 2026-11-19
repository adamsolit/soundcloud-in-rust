use crate::audio::types::RepeatMode;
use crate::models::track::Track;

#[derive(Debug, Clone, Default)]
pub struct PlaybackQueue {
    pub current: Option<Track>,
    pub items: Vec<Track>,
    pub original_pool: Vec<Track>,
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
            self.original_pool.clear();
            return None;
        }

        self.original_pool = tracks.clone();
        let idx = start_idx.min(tracks.len() - 1);
        let current = tracks[idx].clone();
        let mut remaining = tracks[idx + 1..].to_vec();

        if self.shuffle && remaining.len() > 1 {
            fastrand::shuffle(&mut remaining);
        }

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

    /// Automatically advances to next track upon track completion (respects RepeatMode::One).
    pub fn next_track(&mut self) -> Option<Track> {
        if self.repeat_mode == RepeatMode::One {
            return self.current.clone();
        }
        self.skip_next_track()
    }

    /// Explicitly skips to next track (bypasses RepeatMode::One on manual skip button click).
    pub fn skip_next_track(&mut self) -> Option<Track> {
        if let Some(curr) = self.current.take() {
            self.history.push(curr.clone());
            if self.repeat_mode == RepeatMode::All {
                self.items.push(curr);
            }
        }

        if self.items.is_empty() {
            if self.repeat_mode == RepeatMode::All && !self.original_pool.is_empty() {
                let mut pool = self.original_pool.clone();
                if self.shuffle && pool.len() > 1 {
                    fastrand::shuffle(&mut pool);
                }
                let first = pool.remove(0);
                self.current = Some(first.clone());
                self.items = pool;
                return Some(first);
            }
            return None;
        }

        let next = self.items.remove(0);
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
        self.set_shuffle(!self.shuffle)
    }

    pub fn set_shuffle(&mut self, enabled: bool) -> bool {
        self.shuffle = enabled;
        if self.shuffle {
            // Shuffle the remaining queue items
            if self.items.len() > 1 {
                fastrand::shuffle(&mut self.items);
            }
        } else {
            // Restore remaining queue items to their original sequential order in the playlist!
            if let Some(ref curr) = self.current {
                if let Some(curr_idx) = self.original_pool.iter().position(|t| t.id == curr.id) {
                    let mut sequential_remaining = Vec::new();
                    for t in &self.original_pool[curr_idx + 1..] {
                        if self.items.iter().any(|item| item.id == t.id) {
                            sequential_remaining.push(t.clone());
                        }
                    }
                    if !sequential_remaining.is_empty() {
                        self.items = sequential_remaining;
                    }
                }
            }
        }
        self.shuffle
    }

    pub fn cycle_repeat(&mut self) -> RepeatMode {
        self.repeat_mode = self.repeat_mode.next();
        self.repeat_mode
    }

    pub fn clear(&mut self) {
        self.current = None;
        self.items.clear();
        self.original_pool.clear();
        self.history.clear();
    }
}
