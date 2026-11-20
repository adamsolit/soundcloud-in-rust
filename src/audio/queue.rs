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
            // Restore remaining queue items to their original sequential order in the playlist relative to current track
            if let Some(ref curr) = self.current {
                let pool_len = self.original_pool.len();
                if pool_len > 0 {
                    let curr_idx = self.original_pool.iter().position(|t| t.id == curr.id).unwrap_or(0);
                    let mut remaining = self.items.clone();
                    remaining.sort_by_key(|t| {
                        if let Some(idx) = self.original_pool.iter().position(|p| p.id == t.id) {
                            if idx > curr_idx {
                                idx - curr_idx
                            } else {
                                pool_len + idx - curr_idx
                            }
                        } else {
                            usize::MAX
                        }
                    });
                    self.items = remaining;
                }
            }
        }
        self.shuffle
    }

    pub fn cycle_repeat(&mut self) -> RepeatMode {
        self.repeat_mode = self.repeat_mode.next();
        self.repeat_mode
    }

    pub fn jump_to(&mut self, idx: usize) -> Option<Track> {
        if idx >= self.items.len() {
            return None;
        }
        if let Some(curr) = self.current.take() {
            self.history.push(curr);
        }
        for _ in 0..idx {
            if !self.items.is_empty() {
                self.history.push(self.items.remove(0));
            }
        }
        let target = self.items.remove(0);
        self.current = Some(target.clone());
        Some(target)
    }

    pub fn remove_at(&mut self, idx: usize) -> Option<Track> {
        if idx < self.items.len() {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.original_pool.clear();
    }
}
