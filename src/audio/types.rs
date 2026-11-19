use crate::models::track::Track;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackState {
    #[default]
    Stopped,
    Playing,
    Paused,
    Buffering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

impl RepeatMode {
    pub fn next(&self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::All => "Repeat All",
            Self::One => "Repeat Track",
        }
    }
}

#[derive(Debug, Clone)]
pub enum AudioEvent {
    StateChanged(PlaybackState),
    PositionChanged { position_ms: u64, duration_ms: u64 },
    Buffering(f64),
    TrackStarted(Box<Track>),
    TrackEnded,
    Error(String),
}
