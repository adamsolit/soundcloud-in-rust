use anyhow::{bail, Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{debug, info, warn};

#[cfg(unix)]
pub type IpcStream = std::os::unix::net::UnixStream;

#[cfg(windows)]
pub type IpcStream = std::fs::File;

fn connect_ipc(sock_path: &str) -> std::io::Result<IpcStream> {
    #[cfg(unix)]
    {
        let stream = std::os::unix::net::UnixStream::connect(sock_path)?;
        let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
        let _ = stream.set_write_timeout(Some(Duration::from_millis(100)));
        Ok(stream)
    }
    #[cfg(windows)]
    {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(sock_path)
    }
}

pub type EofCallback = Box<dyn Fn() + Send + Sync + 'static>;

pub struct MpvDriver {
    process: Mutex<Option<Child>>,
    sock_path: String,
    cmd_stream: Mutex<Option<IpcStream>>,
    position_ms: Arc<AtomicU64>,
    duration_ms: Arc<AtomicU64>,
    is_playing: Arc<AtomicBool>,
    is_paused: Arc<AtomicBool>,
    has_file: Arc<AtomicBool>,
    speed: Arc<AtomicU64>,
    eof_cb: Arc<Mutex<Option<EofCallback>>>,
}

impl MpvDriver {
    pub fn new() -> Result<Self> {
        let rand_id = fastrand::u64(..);
        #[cfg(windows)]
        let sock_path = format!(r"\\.\pipe\cloudpulse_mpv_{}_{}", std::process::id(), rand_id);

        #[cfg(unix)]
        let sock_path = {
            let runtime_dir = crate::paths::runtime_dir();
            let path = runtime_dir
                .join(format!("cloudpulse_mpv_{}_{}.sock", std::process::id(), rand_id))
                .to_string_lossy()
                .to_string();
            let _ = std::fs::remove_file(&path);
            path
        };

        let mut mpv_path = std::path::PathBuf::from("mpv");
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let local_mpv = dir.join(if cfg!(windows) { "mpv.exe" } else { "mpv" });
                if local_mpv.exists() {
                    mpv_path = local_mpv;
                }
            }
        }

        let mut cmd = Command::new(mpv_path);
        cmd.arg("--idle=yes")
            .arg("--no-video")
            .arg(format!("--input-ipc-server={}", sock_path))
            .arg("--audio-buffer=0.2")
            .arg("--demuxer-max-bytes=24M")
            .arg("--demuxer-max-back-bytes=6M")
            .arg("--network-timeout=15")
            .arg("--user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36")
            .arg("--force-window=no")
            .arg("--terminal=no")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
            cmd.arg("--ao=wasapi");
        }

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.arg("--ao=pipewire,pulse,alsa");
            unsafe {
                cmd.pre_exec(|| {
                    libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
                    Ok(())
                });
            }
        }

        let child = cmd.spawn().context("Failed to spawn mpv background process")?;

        // Wait up to 1500ms for IPC socket creation
        let mut connected_cmd = None;
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(50));
            if let Ok(stream) = connect_ipc(&sock_path) {
                connected_cmd = Some(stream);
                break;
            }
        }

        let cmd_stream = connected_cmd.context("Timed out connecting to mpv IPC socket")?;

        // Continually drain command replies so kernel socket buffer never blocks mpv IPC
        if let Ok(reader_stream) = cmd_stream.try_clone() {
            #[cfg(unix)]
            let _ = reader_stream.set_read_timeout(None);
            std::thread::spawn(move || {
                let mut reader = BufReader::new(reader_stream);
                let mut line = String::new();
                while let Ok(n) = reader.read_line(&mut line) {
                    if n == 0 {
                        break; // EOF: socket disconnected
                    }
                    line.clear();
                }
            });
        }

        // Establish second connection for event monitoring
        let mut evt_stream = connect_ipc(&sock_path)
            .context("Connecting event socket to mpv IPC")?;
        #[cfg(unix)]
        let _ = evt_stream.set_read_timeout(None); // blocking read in dedicated thread

        // Register property observers on event socket
        let obs1 = serde_json::json!({"command": ["observe_property", 1, "time-pos"]}).to_string();
        let obs2 = serde_json::json!({"command": ["observe_property", 2, "pause"]}).to_string();
        let obs3 = serde_json::json!({"command": ["observe_property", 3, "duration"]}).to_string();
        let _ = evt_stream.write_all(format!("{}\n{}\n{}\n", obs1, obs2, obs3).as_bytes());

        let position_ms = Arc::new(AtomicU64::new(0));
        let duration_ms = Arc::new(AtomicU64::new(0));
        let is_playing = Arc::new(AtomicBool::new(false));
        let is_paused = Arc::new(AtomicBool::new(false));
        let has_file = Arc::new(AtomicBool::new(false));
        let speed = Arc::new(AtomicU64::new(1000));
        let eof_cb: Arc<Mutex<Option<EofCallback>>> = Arc::new(Mutex::new(None));

        let pos_arc = Arc::clone(&position_ms);
        let dur_arc = Arc::clone(&duration_ms);
        let play_arc = Arc::clone(&is_playing);
        let pause_arc = Arc::clone(&is_paused);
        let has_file_arc = Arc::clone(&has_file);
        let cb_arc = Arc::clone(&eof_cb);

        // Dedicated event reader thread
        std::thread::spawn(move || {
            let reader = BufReader::new(evt_stream);
            for line_res in reader.lines() {
                match line_res {
                    Ok(line) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }

                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                            // Check property change events
                            if val.get("event").and_then(|v| v.as_str()) == Some("property-change") {
                                match val.get("name").and_then(|v| v.as_str()) {
                                    Some("time-pos") => {
                                        if let Some(secs) = val.get("data").and_then(|d| d.as_f64()) {
                                            pos_arc.store((secs * 1000.0) as u64, Ordering::Relaxed);
                                            if has_file_arc.load(Ordering::Relaxed) {
                                                play_arc.store(!pause_arc.load(Ordering::Relaxed), Ordering::Relaxed);
                                            }
                                        }
                                    }
                                    Some("duration") => {
                                        if let Some(secs) = val.get("data").and_then(|d| d.as_f64()) {
                                            dur_arc.store((secs * 1000.0) as u64, Ordering::Relaxed);
                                        }
                                    }
                                    Some("pause") => {
                                        if let Some(paused) = val.get("data").and_then(|d| d.as_bool()) {
                                            pause_arc.store(paused, Ordering::Relaxed);
                                            if has_file_arc.load(Ordering::Relaxed) {
                                                if paused {
                                                    play_arc.store(false, Ordering::Relaxed);
                                                } else {
                                                    play_arc.store(true, Ordering::Relaxed);
                                                }
                                            } else {
                                                // When idle with no file loaded, never report playing!
                                                play_arc.store(false, Ordering::Relaxed);
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }

                            if val.get("event").and_then(|v| v.as_str()) == Some("start-file") 
                                || val.get("event").and_then(|v| v.as_str()) == Some("file-loaded") {
                                has_file_arc.store(true, Ordering::Relaxed);
                                play_arc.store(true, Ordering::Relaxed);
                                pause_arc.store(false, Ordering::Relaxed);
                            }

                            // Check end-file event (track finished naturally or stopped/errored)
                            if val.get("event").and_then(|v| v.as_str()) == Some("end-file") {
                                has_file_arc.store(false, Ordering::Relaxed);
                                play_arc.store(false, Ordering::Relaxed);
                                pause_arc.store(false, Ordering::Relaxed);
                                pos_arc.store(0, Ordering::Relaxed);
                                let reason = val.get("reason").and_then(|v| v.as_str()).unwrap_or("");
                                if reason == "eof" {
                                    debug!("MPV reached EOF on current stream");
                                    if let Ok(guard) = cb_arc.lock() {
                                        if let Some(ref cb) = *guard {
                                            cb();
                                        }
                                    }
                                } else if reason == "error" {
                                    warn!("MPV end-file due to playback error");
                                }
                            }
                        }
                    }
                    Err(e) => {
                        debug!("MPV event reader closed: {}", e);
                        break;
                    }
                }
            }
        });

        let driver = Self {
            process: Mutex::new(Some(child)),
            sock_path,
            cmd_stream: Mutex::new(Some(cmd_stream)),
            position_ms,
            duration_ms,
            is_playing,
            is_paused,
            has_file,
            speed,
            eof_cb,
        };

        info!("MPV hardware-accelerated PipeWire audio backend ready");
        Ok(driver)
    }

    pub fn set_on_eof<F: Fn() + Send + Sync + 'static>(&self, f: F) {
        let mut guard = self.eof_cb.lock().unwrap();
        *guard = Some(Box::new(f));
    }

    fn send_command(&self, json_cmd: &str) -> Result<()> {
        let mut stream_guard = self.cmd_stream.lock().unwrap();
        if stream_guard.is_none() {
            if let Ok(s) = connect_ipc(&self.sock_path) {
                *stream_guard = Some(s);
            }
        }

        let payload = format!("{}\n", json_cmd);
        if let Some(ref mut stream) = *stream_guard {
            if stream.write_all(payload.as_bytes()).is_ok() {
                return Ok(());
            }
            warn!("First attempt to send command to MPV failed, attempting reconnection...");
        }

        // Reconnect and retry once
        *stream_guard = None;
        if let Ok(mut s) = connect_ipc(&self.sock_path) {
            if s.write_all(payload.as_bytes()).is_ok() {
                *stream_guard = Some(s);
                return Ok(());
            }
        }

        bail!("Failed to send command to MPV IPC socket");
    }

    pub fn load_file(&self, uri: &str, duration_ms: u64, start_pos_ms: u64) -> Result<()> {
        self.has_file.store(true, Ordering::Relaxed);
        self.duration_ms.store(duration_ms, Ordering::Relaxed);
        self.position_ms.store(start_pos_ms, Ordering::Relaxed);

        let cmd = if start_pos_ms > 0 {
            let start_sec = start_pos_ms as f64 / 1000.0;
            serde_json::json!({
                "command": ["loadfile", uri, "replace", 0, format!("start={}", start_sec)]
            })
            .to_string()
        } else {
            serde_json::json!({
                "command": ["loadfile", uri, "replace"]
            })
            .to_string()
        };

        self.send_command(&cmd)?;

        // CRITICAL: Explicitly unpause so the loaded audio immediately starts playing
        let unpause_cmd = serde_json::json!({
            "command": ["set_property", "pause", false]
        })
        .to_string();
        let _ = self.send_command(&unpause_cmd);

        self.is_playing.store(true, Ordering::Relaxed);
        self.is_paused.store(false, Ordering::Relaxed);
        Ok(())
    }

    pub fn pause(&self) {
        let cmd = serde_json::json!({
            "command": ["set_property", "pause", true]
        })
        .to_string();
        let _ = self.send_command(&cmd);
        self.is_playing.store(false, Ordering::Relaxed);
        self.is_paused.store(true, Ordering::Relaxed);
    }

    pub fn resume(&self) {
        let cmd = serde_json::json!({
            "command": ["set_property", "pause", false]
        })
        .to_string();
        let _ = self.send_command(&cmd);
        self.is_playing.store(true, Ordering::Relaxed);
        self.is_paused.store(false, Ordering::Relaxed);
    }

    pub fn seek(&self, position_ms: u64) -> Result<()> {
        let secs = position_ms as f64 / 1000.0;
        let cmd = serde_json::json!({
            "command": ["seek", secs, "absolute+keyframes"]
        })
        .to_string();
        self.send_command(&cmd)?;
        self.position_ms.store(position_ms, Ordering::Relaxed);
        Ok(())
    }

    pub fn set_volume(&self, vol: f64) {
        let pct = (vol * 100.0).clamp(0.0, 150.0);
        let cmd = serde_json::json!({
            "command": ["set_property", "volume", pct]
        })
        .to_string();
        let _ = self.send_command(&cmd);
    }

    pub fn set_speed(&self, speed: f64) {
        let clamped = speed.clamp(0.25, 4.0);
        let cmd = serde_json::json!({
            "command": ["set_property", "speed", clamped]
        })
        .to_string();
        let _ = self.send_command(&cmd);
        self.speed.store((clamped * 1000.0) as u64, Ordering::Relaxed);
    }

    pub fn get_speed(&self) -> f64 {
        self.speed.load(Ordering::Relaxed) as f64 / 1000.0
    }

    pub fn set_audio_normalization(&self, enabled: bool) {
        let filter = if enabled {
            "lavfi=[dynaudnorm=f=75:g=25:p=0.95:m=10.0:r=0.9]"
        } else {
            ""
        };
        let cmd = serde_json::json!({
            "command": ["set_property", "af", filter]
        })
        .to_string();
        let _ = self.send_command(&cmd);
    }

    pub fn stop(&self) {
        let cmd = serde_json::json!({
            "command": ["stop"]
        })
        .to_string();
        let _ = self.send_command(&cmd);
        self.has_file.store(false, Ordering::Relaxed);
        self.is_playing.store(false, Ordering::Relaxed);
        self.is_paused.store(false, Ordering::Relaxed);
        self.position_ms.store(0, Ordering::Relaxed);
    }

    pub fn get_position_ms(&self) -> u64 {
        self.position_ms.load(Ordering::Relaxed)
    }

    pub fn get_duration_ms(&self) -> u64 {
        self.duration_ms.load(Ordering::Relaxed)
    }

    pub fn is_playing(&self) -> bool {
        self.has_file.load(Ordering::Relaxed) && self.is_playing.load(Ordering::Relaxed)
    }

    pub fn is_paused(&self) -> bool {
        self.has_file.load(Ordering::Relaxed) && self.is_paused.load(Ordering::Relaxed)
    }

    pub fn is_loaded(&self) -> bool {
        self.has_file.load(Ordering::Relaxed)
    }

    pub fn terminate(&self) {
        let _ = self.send_command("{\"command\": [\"quit\"]}");
        if let Ok(mut child_guard) = self.process.lock() {
            if let Some(ref mut child) = *child_guard {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        let _ = std::fs::remove_file(&self.sock_path);
    }
}

impl Drop for MpvDriver {
    fn drop(&mut self) {
        let _ = self.send_command("{\"command\": [\"quit\"]}");
        if let Ok(mut child_guard) = self.process.lock() {
            if let Some(ref mut child) = *child_guard {
                let _ = child.kill();
            }
        }
        let _ = std::fs::remove_file(&self.sock_path);
    }
}
