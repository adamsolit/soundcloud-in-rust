use anyhow::{Context, Result};
use std::sync::{Arc, Mutex};
#[cfg(feature = "gstreamer")]
use tracing::error;
use tracing::{info, warn};

use crate::audio::mpv::MpvDriver;
use crate::audio::queue::PlaybackQueue;
use crate::audio::types::{AudioEvent, PlaybackState};
use crate::models::track::Track;

pub type AudioCallback = Arc<dyn Fn(AudioEvent) + Send + Sync + 'static>;

pub struct AudioEngine {
    mpv: Option<Arc<MpvDriver>>,
    #[cfg(feature = "gstreamer")]
    gst_pipeline: Option<gstreamer::Element>,
    #[cfg(feature = "gstreamer")]
    _bus_watch: Arc<Mutex<Option<gstreamer::bus::BusWatchGuard>>>,
    queue: Arc<Mutex<PlaybackQueue>>,
    state: Arc<Mutex<PlaybackState>>,
    callback: Arc<Mutex<Option<AudioCallback>>>,
    current_duration_ms: Arc<Mutex<u64>>,
    volume: Arc<std::sync::atomic::AtomicU64>,
    speed: Arc<std::sync::atomic::AtomicU64>,
}

impl AudioEngine {
    pub fn new() -> Result<Self> {
        let queue = Arc::new(Mutex::new(PlaybackQueue::new()));
        let state = Arc::new(Mutex::new(PlaybackState::Stopped));
        let callback: Arc<Mutex<Option<AudioCallback>>> = Arc::new(Mutex::new(None));
        let current_duration_ms = Arc::new(Mutex::new(0));
        let volume = Arc::new(std::sync::atomic::AtomicU64::new(1000));
        let speed = Arc::new(std::sync::atomic::AtomicU64::new(1000));

        // 1. Prefer hardware-accelerated PipeWire/ALSA MPV backend
        let mpv_driver = match MpvDriver::new() {
            Ok(driver) => {
                let driver_arc = Arc::new(driver);
                let state_c = Arc::clone(&state);
                let cb_c = Arc::clone(&callback);

                driver_arc.set_on_eof(move || {
                    *state_c.lock().unwrap() = PlaybackState::Stopped;
                    if let Ok(guard) = cb_c.lock() {
                        if let Some(ref cb) = *guard {
                            cb(AudioEvent::TrackEnded);
                            cb(AudioEvent::StateChanged(PlaybackState::Stopped));
                        }
                    }
                });

                Some(driver_arc)
            }
            Err(e) => {
                warn!("MPV backend not available: {}", e);
                None
            }
        };

        // 2. Initialize GStreamer if MPV is unavailable (optional fallback)
        #[cfg(feature = "gstreamer")]
        let (gst_pipeline, _bus_watch) = if mpv_driver.is_none() {
            match gstreamer::init() {
                Ok(_) => {
                    let pipeline = gstreamer::ElementFactory::make("playbin3")
                        .build()
                        .or_else(|_| gstreamer::ElementFactory::make("playbin").build())
                        .ok();
                    (pipeline, Arc::new(Mutex::new(None)))
                }
                Err(e) => {
                    warn!("Failed to init GStreamer: {}", e);
                    (None, Arc::new(Mutex::new(None)))
                }
            }
        } else {
            (None, Arc::new(Mutex::new(None)))
        };

        let engine = Self {
            mpv: mpv_driver,
            #[cfg(feature = "gstreamer")]
            gst_pipeline,
            #[cfg(feature = "gstreamer")]
            _bus_watch,
            queue,
            state,
            callback,
            current_duration_ms,
            volume,
            speed,
        };

        #[cfg(feature = "gstreamer")]
        if engine.gst_pipeline.is_some() {
            engine.setup_gst_bus();
        }

        info!("CloudPulse Audio Engine successfully initialized");
        Ok(engine)
    }

    pub fn set_callback(&self, cb: AudioCallback) {
        *self.callback.lock().unwrap() = Some(cb);
    }

    #[cfg(feature = "gstreamer")]
    fn setup_gst_bus(&self) {
        use gstreamer::prelude::*;
        if let Some(ref pipeline) = self.gst_pipeline {
            if let Some(bus) = pipeline.bus() {
                let state_arc = Arc::clone(&self.state);
                let duration_arc = Arc::clone(&self.current_duration_ms);
                let pipeline_clone = pipeline.clone();

                let watch_guard = bus
                    .add_watch_local(move |_bus, msg| {
                        use gstreamer::MessageView;
                        match msg.view() {
                            MessageView::Eos(..) => {
                                info!("GStreamer: End of stream reached");
                                *state_arc.lock().unwrap() = PlaybackState::Stopped;
                            }
                            MessageView::Error(err) => {
                                error!("GStreamer error: {} ({:?})", err.error(), err.debug());
                                *state_arc.lock().unwrap() = PlaybackState::Stopped;
                            }
                            MessageView::DurationChanged(..) => {
                                if let Some(dur) = pipeline_clone.query_duration::<gstreamer::ClockTime>() {
                                    *duration_arc.lock().unwrap() = dur.mseconds();
                                }
                            }
                            _ => {}
                        }
                        glib::ControlFlow::Continue
                    })
                    .expect("Failed to attach bus watch");

                *self._bus_watch.lock().unwrap() = Some(watch_guard);
            }
        }
    }

    pub fn play_stream(&self, uri: &str, track: Track, start_pos_ms: u64) -> Result<()> {
        info!("Loading audio stream URI: {} (start: {}ms)", uri, start_pos_ms);

        // Update queue current track
        {
            let mut q = self.queue.lock().unwrap();
            q.current = Some(track.clone());
        }

        *self.current_duration_ms.lock().unwrap() = track.duration;

        if let Some(ref mpv) = self.mpv {
            mpv.load_file(uri, track.duration, start_pos_ms)
                .context("Loading stream in MPV driver")?;
            *self.state.lock().unwrap() = PlaybackState::Playing;
        } else {
            #[cfg(feature = "gstreamer")]
            if let Some(ref pipeline) = self.gst_pipeline {
                use gstreamer::prelude::*;
                pipeline.set_property("uri", uri);
                pipeline
                    .set_state(gstreamer::State::Playing)
                    .context("Setting GStreamer pipeline to Playing")?;
                *self.state.lock().unwrap() = PlaybackState::Playing;
            } else {
                anyhow::bail!("No audio driver available to play stream");
            }
            #[cfg(not(feature = "gstreamer"))]
            {
                anyhow::bail!("No audio driver available to play stream");
            }
        }

        if let Ok(guard) = self.callback.lock() {
            if let Some(ref cb) = *guard {
                cb(AudioEvent::TrackStarted(Box::new(track)));
                cb(AudioEvent::StateChanged(PlaybackState::Playing));
            }
        }

        Ok(())
    }

    pub fn pause(&self) {
        if let Some(ref mpv) = self.mpv {
            mpv.pause();
        } else {
            #[cfg(feature = "gstreamer")]
            if let Some(ref pipeline) = self.gst_pipeline {
                use gstreamer::prelude::*;
                let _ = pipeline.set_state(gstreamer::State::Paused);
            }
        }
        *self.state.lock().unwrap() = PlaybackState::Paused;
        if let Ok(guard) = self.callback.lock() {
            if let Some(ref cb) = *guard {
                cb(AudioEvent::StateChanged(PlaybackState::Paused));
            }
        }
    }

    pub fn resume(&self) {
        if let Some(ref mpv) = self.mpv {
            mpv.resume();
        } else {
            #[cfg(feature = "gstreamer")]
            if let Some(ref pipeline) = self.gst_pipeline {
                use gstreamer::prelude::*;
                let _ = pipeline.set_state(gstreamer::State::Playing);
            }
        }
        *self.state.lock().unwrap() = PlaybackState::Playing;
        if let Ok(guard) = self.callback.lock() {
            if let Some(ref cb) = *guard {
                cb(AudioEvent::StateChanged(PlaybackState::Playing));
            }
        }
    }

    pub fn toggle_play_pause(&self) {
        let current_state = self.get_state();
        match current_state {
            PlaybackState::Playing => self.pause(),
            PlaybackState::Paused => self.resume(),
            PlaybackState::Stopped => {
                if let Some(ref mpv) = self.mpv {
                    if mpv.get_position_ms() > 0 {
                        self.resume();
                    }
                }
            }
            PlaybackState::Buffering => self.pause(),
        }
    }

    pub fn stop(&self) {
        if let Some(ref mpv) = self.mpv {
            mpv.stop();
        } else {
            #[cfg(feature = "gstreamer")]
            if let Some(ref pipeline) = self.gst_pipeline {
                use gstreamer::prelude::*;
                let _ = pipeline.set_state(gstreamer::State::Null);
            }
        }
        *self.state.lock().unwrap() = PlaybackState::Stopped;
        if let Ok(guard) = self.callback.lock() {
            if let Some(ref cb) = *guard {
                cb(AudioEvent::StateChanged(PlaybackState::Stopped));
            }
        }
    }

    pub fn terminate(&self) {
        if let Some(ref mpv) = self.mpv {
            mpv.terminate();
        } else {
            #[cfg(feature = "gstreamer")]
            if let Some(ref pipeline) = self.gst_pipeline {
                use gstreamer::prelude::*;
                let _ = pipeline.set_state(gstreamer::State::Null);
            }
        }
        *self.state.lock().unwrap() = PlaybackState::Stopped;
    }

    pub fn seek(&self, position_ms: u64) -> Result<()> {
        if let Some(ref mpv) = self.mpv {
            mpv.seek(position_ms)?;
        } else {
            #[cfg(feature = "gstreamer")]
            if let Some(ref pipeline) = self.gst_pipeline {
                use gstreamer::prelude::*;
                let time = gstreamer::ClockTime::from_mseconds(position_ms);
                pipeline
                    .seek_simple(gstreamer::SeekFlags::FLUSH | gstreamer::SeekFlags::KEY_UNIT, time)
                    .context("GStreamer seek_simple failed")?;
            }
        }
        Ok(())
    }

    pub fn set_volume(&self, volume: f64) {
        self.volume.store((volume * 1000.0) as u64, std::sync::atomic::Ordering::Relaxed);
        if let Some(ref mpv) = self.mpv {
            mpv.set_volume(volume);
        } else {
            #[cfg(feature = "gstreamer")]
            if let Some(ref pipeline) = self.gst_pipeline {
                use gstreamer::prelude::*;
                let clamped = volume.clamp(0.0, 1.5);
                pipeline.set_property("volume", clamped);
            }
        }
    }

    pub fn get_volume(&self) -> f64 {
        self.volume.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1000.0
    }

    pub fn set_speed(&self, speed: f64) {
        self.speed.store((speed * 1000.0) as u64, std::sync::atomic::Ordering::Relaxed);
        if let Some(ref mpv) = self.mpv {
            mpv.set_speed(speed);
        }
    }

    pub fn get_speed(&self) -> f64 {
        if let Some(ref mpv) = self.mpv {
            mpv.get_speed()
        } else {
            self.speed.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1000.0
        }
    }

    pub fn set_audio_normalization(&self, enabled: bool) {
        if let Some(ref mpv) = self.mpv {
            mpv.set_audio_normalization(enabled);
        }
    }

    pub fn get_position_ms(&self) -> u64 {
        if let Some(ref mpv) = self.mpv {
            mpv.get_position_ms()
        } else {
            #[cfg(feature = "gstreamer")]
            if let Some(ref pipeline) = self.gst_pipeline {
                use gstreamer::prelude::*;
                return pipeline
                    .query_position::<gstreamer::ClockTime>()
                    .map(|t| t.mseconds())
                    .unwrap_or(0);
            }
            0
        }
    }

    pub fn get_duration_ms(&self) -> u64 {
        if let Some(ref mpv) = self.mpv {
            let dur = mpv.get_duration_ms();
            if dur > 0 {
                return dur;
            }
        }
        let cached = *self.current_duration_ms.lock().unwrap();
        if cached > 0 {
            return cached;
        }
        #[cfg(feature = "gstreamer")]
        if let Some(ref pipeline) = self.gst_pipeline {
            use gstreamer::prelude::*;
            return pipeline
                .query_duration::<gstreamer::ClockTime>()
                .map(|t| t.mseconds())
                .unwrap_or(0);
        }
        0
    }

    pub fn is_loaded(&self) -> bool {
        if let Some(ref mpv) = self.mpv {
            mpv.is_loaded()
        } else {
            *self.state.lock().unwrap() != PlaybackState::Stopped
        }
    }

    pub fn get_state(&self) -> PlaybackState {
        if let Some(ref mpv) = self.mpv {
            if mpv.is_playing() {
                PlaybackState::Playing
            } else if mpv.is_paused() {
                PlaybackState::Paused
            } else {
                PlaybackState::Stopped
            }
        } else {
            *self.state.lock().unwrap()
        }
    }

    pub fn get_queue(&self) -> Arc<Mutex<PlaybackQueue>> {
        Arc::clone(&self.queue)
    }

    pub fn current_track(&self) -> Option<Track> {
        self.queue.lock().unwrap().current.clone()
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.stop();
    }
}
