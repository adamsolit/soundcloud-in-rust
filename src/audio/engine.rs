use anyhow::{Context, Result};
use std::sync::{Arc, Mutex};
use tracing::{error, info, warn};

use crate::audio::mpv::MpvDriver;
use crate::audio::queue::PlaybackQueue;
use crate::audio::types::{AudioEvent, PlaybackState};
use crate::models::track::Track;

pub type AudioCallback = Arc<dyn Fn(AudioEvent) + Send + Sync + 'static>;

pub struct AudioEngine {
    mpv: Option<Arc<MpvDriver>>,
    gst_pipeline: Option<gstreamer::Element>,
    _bus_watch: Arc<Mutex<Option<gstreamer::bus::BusWatchGuard>>>,
    queue: Arc<Mutex<PlaybackQueue>>,
    state: Arc<Mutex<PlaybackState>>,
    callback: Arc<Mutex<Option<AudioCallback>>>,
    current_duration_ms: Arc<Mutex<u64>>,
}

impl AudioEngine {
    pub fn new() -> Result<Self> {
        let queue = Arc::new(Mutex::new(PlaybackQueue::new()));
        let state = Arc::new(Mutex::new(PlaybackState::Stopped));
        let callback: Arc<Mutex<Option<AudioCallback>>> = Arc::new(Mutex::new(None));
        let current_duration_ms = Arc::new(Mutex::new(0));

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
                warn!("MPV backend not available ({}), falling back to GStreamer...", e);
                None
            }
        };

        // 2. Initialize GStreamer if MPV is unavailable
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
            gst_pipeline,
            _bus_watch,
            queue,
            state,
            callback,
            current_duration_ms,
        };

        if engine.gst_pipeline.is_some() {
            engine.setup_gst_bus();
        }

        info!("CloudPulse Audio Engine successfully initialized");
        Ok(engine)
    }

    pub fn set_callback(&self, cb: AudioCallback) {
        *self.callback.lock().unwrap() = Some(cb);
    }

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
        } else if let Some(ref pipeline) = self.gst_pipeline {
            use gstreamer::prelude::*;
            pipeline.set_property("uri", uri);
            pipeline
                .set_state(gstreamer::State::Playing)
                .context("Setting GStreamer pipeline to Playing")?;
            *self.state.lock().unwrap() = PlaybackState::Playing;
        } else {
            anyhow::bail!("No audio driver available to play stream");
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
        } else if let Some(ref pipeline) = self.gst_pipeline {
            use gstreamer::prelude::*;
            let _ = pipeline.set_state(gstreamer::State::Paused);
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
        } else if let Some(ref pipeline) = self.gst_pipeline {
            use gstreamer::prelude::*;
            let _ = pipeline.set_state(gstreamer::State::Playing);
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
        } else if let Some(ref pipeline) = self.gst_pipeline {
            use gstreamer::prelude::*;
            let _ = pipeline.set_state(gstreamer::State::Null);
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
        } else if let Some(ref pipeline) = self.gst_pipeline {
            use gstreamer::prelude::*;
            let _ = pipeline.set_state(gstreamer::State::Null);
        }
        *self.state.lock().unwrap() = PlaybackState::Stopped;
    }

    pub fn seek(&self, position_ms: u64) -> Result<()> {
        if let Some(ref mpv) = self.mpv {
            mpv.seek(position_ms)?;
        } else if let Some(ref pipeline) = self.gst_pipeline {
            use gstreamer::prelude::*;
            let time = gstreamer::ClockTime::from_mseconds(position_ms);
            pipeline
                .seek_simple(gstreamer::SeekFlags::FLUSH | gstreamer::SeekFlags::KEY_UNIT, time)
                .context("GStreamer seek_simple failed")?;
        }
        Ok(())
    }

    pub fn set_volume(&self, volume: f64) {
        if let Some(ref mpv) = self.mpv {
            mpv.set_volume(volume);
        } else if let Some(ref pipeline) = self.gst_pipeline {
            use gstreamer::prelude::*;
            let clamped = volume.clamp(0.0, 1.5);
            pipeline.set_property("volume", clamped);
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
        } else if let Some(ref pipeline) = self.gst_pipeline {
            use gstreamer::prelude::*;
            pipeline
                .query_position::<gstreamer::ClockTime>()
                .map(|t| t.mseconds())
                .unwrap_or(0)
        } else {
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
        if let Some(ref pipeline) = self.gst_pipeline {
            use gstreamer::prelude::*;
            pipeline
                .query_duration::<gstreamer::ClockTime>()
                .map(|t| t.mseconds())
                .unwrap_or(0)
        } else {
            0
        }
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
