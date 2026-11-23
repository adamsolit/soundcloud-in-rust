use anyhow::Result;
#[cfg(target_os = "linux")]
use mpris_server::zbus::{self, fdo};
#[cfg(target_os = "linux")]
use mpris_server::{
    Metadata, PlaybackRate, PlayerInterface, Property,
    RootInterface, Server, Time, TrackId, Volume,
};
#[cfg(target_os = "linux")]
pub use mpris_server::{LoopStatus, PlaybackStatus};

#[cfg(not(target_os = "linux"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
}

#[cfg(not(target_os = "linux"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopStatus {
    None,
    Track,
    Playlist,
}

#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(target_os = "linux")]
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedSender;
#[cfg(target_os = "linux")]
use tracing::{error, info};

use crate::models::track::Track;

#[derive(Debug, Clone)]
pub enum MprisCommand {
    Play,
    Pause,
    PlayPause,
    Next,
    Previous,
    Stop,
    Seek(i64),          // Relative seek offset in ms
    SetPosition(u64),   // Absolute target position in ms
    SetVolume(f64),
    SetShuffle(bool),
    SetLoopStatus(LoopStatus),
}

#[cfg(target_os = "linux")]
#[derive(Clone)]
pub struct MprisHandler {
    cmd_tx: UnboundedSender<MprisCommand>,
    playback_status: Arc<Mutex<PlaybackStatus>>,
    current_track: Arc<Mutex<Option<Track>>>,
    current_volume: Arc<Mutex<f64>>,
    current_position_us: Arc<AtomicU64>,
    loop_status: Arc<Mutex<LoopStatus>>,
    shuffle: Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(target_os = "linux")]
impl MprisHandler {
    pub fn new(cmd_tx: UnboundedSender<MprisCommand>) -> Self {
        Self {
            cmd_tx,
            playback_status: Arc::new(Mutex::new(PlaybackStatus::Stopped)),
            current_track: Arc::new(Mutex::new(None)),
            current_volume: Arc::new(Mutex::new(1.0)),
            current_position_us: Arc::new(AtomicU64::new(0)),
            loop_status: Arc::new(Mutex::new(LoopStatus::None)),
            shuffle: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn set_playback_status(&self, status: PlaybackStatus) {
        *self.playback_status.lock().unwrap() = status;
    }

    pub fn set_track(&self, track: Option<Track>) {
        *self.current_track.lock().unwrap() = track;
    }

    pub fn set_volume(&self, vol: f64) {
        *self.current_volume.lock().unwrap() = vol;
    }

    pub fn set_position_micros(&self, pos_us: u64) {
        self.current_position_us.store(pos_us, Ordering::Relaxed);
    }
}

#[cfg(target_os = "linux")]
fn build_metadata(track: Option<&Track>) -> Metadata {
    if let Some(t) = track {
        let mut b = Metadata::builder()
            .title(&t.title)
            .artist(vec![t.artist_name().to_string()])
            .length(Time::from_micros((t.duration * 1000) as i64));

        if let Ok(obj_path) = zbus::ObjectPath::try_from(format!("/org/mpris/MediaPlayer2/track/{}", t.id)) {
            b = b.trackid(TrackId::from(obj_path));
        }

        if let Some(art) = t.best_artwork_url() {
            b = b.art_url(art);
        }
        if let Some(url) = t.permalink_url.as_ref() {
            b = b.url(url.clone());
        }

        b.build()
    } else {
        Metadata::new()
    }
}

#[cfg(target_os = "linux")]
impl RootInterface for MprisHandler {
    async fn raise(&self) -> fdo::Result<()> {
        Ok(())
    }

    async fn quit(&self) -> fdo::Result<()> {
        let _ = self.cmd_tx.send(MprisCommand::Stop);
        Ok(())
    }

    async fn can_quit(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_fullscreen(&self, _fullscreen: bool) -> zbus::Result<()> {
        Ok(())
    }

    async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_raise(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn has_track_list(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn identity(&self) -> fdo::Result<String> {
        Ok("SoundCloud".to_string())
    }

    async fn desktop_entry(&self) -> fdo::Result<String> {
        Ok("soundcloud".to_string())
    }

    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(vec!["https".to_string(), "http".to_string()])
    }

    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok(vec![
            "audio/mpeg".to_string(),
            "audio/ogg".to_string(),
            "application/x-mpegURL".to_string(),
        ])
    }
}

#[cfg(target_os = "linux")]
impl PlayerInterface for MprisHandler {
    async fn next(&self) -> fdo::Result<()> {
        let _ = self.cmd_tx.send(MprisCommand::Next);
        Ok(())
    }

    async fn previous(&self) -> fdo::Result<()> {
        let _ = self.cmd_tx.send(MprisCommand::Previous);
        Ok(())
    }

    async fn pause(&self) -> fdo::Result<()> {
        let _ = self.cmd_tx.send(MprisCommand::Pause);
        Ok(())
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        let _ = self.cmd_tx.send(MprisCommand::PlayPause);
        Ok(())
    }

    async fn stop(&self) -> fdo::Result<()> {
        let _ = self.cmd_tx.send(MprisCommand::Stop);
        Ok(())
    }

    async fn play(&self) -> fdo::Result<()> {
        let _ = self.cmd_tx.send(MprisCommand::Play);
        Ok(())
    }

    async fn seek(&self, offset: Time) -> fdo::Result<()> {
        let _ = self.cmd_tx.send(MprisCommand::Seek(offset.as_micros() as i64 / 1000));
        Ok(())
    }

    async fn set_position(&self, _track_id: TrackId, position: Time) -> fdo::Result<()> {
        let pos_ms = (position.as_micros().max(0) / 1000) as u64;
        let _ = self.cmd_tx.send(MprisCommand::SetPosition(pos_ms));
        Ok(())
    }

    async fn open_uri(&self, _uri: String) -> fdo::Result<()> {
        Ok(())
    }

    async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
        Ok(*self.playback_status.lock().unwrap())
    }

    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        Ok(*self.loop_status.lock().unwrap())
    }

    async fn set_loop_status(&self, loop_status: LoopStatus) -> zbus::Result<()> {
        *self.loop_status.lock().unwrap() = loop_status;
        let _ = self.cmd_tx.send(MprisCommand::SetLoopStatus(loop_status));
        Ok(())
    }

    async fn rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(PlaybackRate::default())
    }

    async fn set_rate(&self, _rate: PlaybackRate) -> zbus::Result<()> {
        Ok(())
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(self.shuffle.load(Ordering::Relaxed))
    }

    async fn set_shuffle(&self, shuffle: bool) -> zbus::Result<()> {
        self.shuffle.store(shuffle, Ordering::Relaxed);
        let _ = self.cmd_tx.send(MprisCommand::SetShuffle(shuffle));
        Ok(())
    }

    async fn metadata(&self) -> fdo::Result<Metadata> {
        let track_guard = self.current_track.lock().unwrap();
        Ok(build_metadata(track_guard.as_ref()))
    }

    async fn volume(&self) -> fdo::Result<Volume> {
        Ok(*self.current_volume.lock().unwrap())
    }

    async fn set_volume(&self, volume: Volume) -> zbus::Result<()> {
        *self.current_volume.lock().unwrap() = volume;
        let _ = self.cmd_tx.send(MprisCommand::SetVolume(volume));
        Ok(())
    }

    async fn position(&self) -> fdo::Result<Time> {
        let us = self.current_position_us.load(Ordering::Relaxed);
        Ok(Time::from_micros(us as i64))
    }

    async fn minimum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn maximum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn can_go_next(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_go_previous(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_play(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_seek(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}

#[cfg(target_os = "linux")]
pub struct MprisService {
    pub handler: MprisHandler,
    server: Option<Server<MprisHandler>>,
}

#[cfg(target_os = "linux")]
impl MprisService {
    pub async fn start(cmd_tx: UnboundedSender<MprisCommand>) -> Result<Self> {
        let handler = MprisHandler::new(cmd_tx);
        match Server::new("soundcloud", handler.clone()).await {
            Ok(server) => {
                info!("Registered MPRIS D-Bus interface: org.mpris.MediaPlayer2.soundcloud");
                Ok(Self {
                    handler,
                    server: Some(server),
                })
            }
            Err(e) => {
                error!("Could not register MPRIS D-Bus service: {}", e);
                Ok(Self { handler, server: None })
            }
        }
    }

    pub async fn update_metadata(&self, track: Option<&Track>) {
        self.handler.set_track(track.cloned());
        if let Some(ref server) = self.server {
            let metadata = build_metadata(track);
            let _ = server.properties_changed([Property::Metadata(metadata)]).await;
        }
    }

    pub async fn update_status(&self, status: PlaybackStatus) {
        self.handler.set_playback_status(status);
        if let Some(ref server) = self.server {
            let _ = server.properties_changed([Property::PlaybackStatus(status)]).await;
        }
    }

    pub async fn update_shuffle(&self, shuffle: bool) {
        self.handler.shuffle.store(shuffle, Ordering::Relaxed);
        if let Some(ref server) = self.server {
            let _ = server.properties_changed([Property::Shuffle(shuffle)]).await;
        }
    }

    pub async fn update_loop_status(&self, loop_status: LoopStatus) {
        *self.handler.loop_status.lock().unwrap() = loop_status;
        if let Some(ref server) = self.server {
            let _ = server.properties_changed([Property::LoopStatus(loop_status)]).await;
        }
    }

    pub async fn emit_seeked(&self, pos_us: u64) {
        self.handler.set_position_micros(pos_us);
        if let Some(ref server) = self.server {
            let time = Time::from_micros(pos_us as i64);
            let _ = server.emit(mpris_server::Signal::Seeked { position: time }).await;
        }
    }
}

#[cfg(not(target_os = "linux"))]
#[derive(Clone, Default)]
pub struct MprisHandler;

#[cfg(not(target_os = "linux"))]
impl MprisHandler {
    pub fn set_playback_status(&self, _status: PlaybackStatus) {}
    pub fn set_track(&self, _track: Option<Track>) {}
    pub fn set_volume(&self, _vol: f64) {}
    pub fn set_position_micros(&self, _pos_us: u64) {}
}

#[cfg(not(target_os = "linux"))]
pub struct MprisService {
    pub handler: MprisHandler,
}

#[cfg(not(target_os = "linux"))]
impl MprisService {
    pub async fn start(_cmd_tx: UnboundedSender<MprisCommand>) -> Result<Self> {
        Ok(Self { handler: MprisHandler })
    }

    pub async fn update_metadata(&self, _track: Option<&Track>) {}

    pub async fn update_status(&self, _status: PlaybackStatus) {}

    pub async fn update_shuffle(&self, _shuffle: bool) {}

    pub async fn update_loop_status(&self, _loop_status: LoopStatus) {}

    pub async fn emit_seeked(&self, _pos_us: u64) {}
}
