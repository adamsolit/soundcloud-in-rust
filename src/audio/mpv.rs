use anyhow::{bail, Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{debug, info, warn};

#[cfg(unix)]
pub type IpcWriter = std::os::unix::net::UnixStream;
#[cfg(unix)]
pub type IpcReader = std::os::unix::net::UnixStream;

#[cfg(windows)]
mod win_pipe {
    use std::ffi::c_void;
    use std::io::{self, Read, Write};
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::{null, null_mut};
    use std::sync::Arc;

    type Handle = *mut c_void;
    const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const OPEN_EXISTING: u32 = 3;
    const FILE_FLAG_OVERLAPPED: u32 = 0x4000_0000;
    const ERROR_BROKEN_PIPE: u32 = 109;
    const ERROR_PIPE_BUSY: u32 = 231;
    const ERROR_NO_DATA: u32 = 232;
    const ERROR_PIPE_NOT_CONNECTED: u32 = 233;
    const ERROR_MORE_DATA: u32 = 234;
    const ERROR_OPERATION_ABORTED: u32 = 995;
    const ERROR_IO_PENDING: u32 = 997;

    #[repr(C)]
    struct Overlapped {
        internal: usize,
        internal_high: usize,
        offset: u32,
        offset_high: u32,
        h_event: Handle,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateFileW(
            lp_file_name: *const u16,
            dw_desired_access: u32,
            dw_share_mode: u32,
            lp_security_attributes: *const c_void,
            dw_creation_disposition: u32,
            dw_flags_and_attributes: u32,
            h_template_file: Handle,
        ) -> Handle;
        fn WaitNamedPipeW(lp_named_pipe_name: *const u16, n_time_out: u32) -> i32;
        fn ReadFile(
            h_file: Handle,
            lp_buffer: *mut u8,
            n_number_of_bytes_to_read: u32,
            lp_number_of_bytes_read: *mut u32,
            lp_overlapped: *mut Overlapped,
        ) -> i32;
        fn WriteFile(
            h_file: Handle,
            lp_buffer: *const u8,
            n_number_of_bytes_to_write: u32,
            lp_number_of_bytes_written: *mut u32,
            lp_overlapped: *mut Overlapped,
        ) -> i32;
        fn CreateEventW(
            lp_event_attributes: *const c_void,
            b_manual_reset: i32,
            b_initial_state: i32,
            lp_name: *const u16,
        ) -> Handle;
        fn ResetEvent(h_event: Handle) -> i32;
        fn GetOverlappedResult(
            h_file: Handle,
            lp_overlapped: *mut Overlapped,
            lp_number_of_bytes_transferred: *mut u32,
            b_wait: i32,
        ) -> i32;
        fn CancelIoEx(h_file: Handle, lp_overlapped: *mut Overlapped) -> i32;
        fn CloseHandle(h_object: Handle) -> i32;
        fn GetLastError() -> u32;
    }

    struct WinPipeHandle {
        handle: Handle,
    }

    // Overlapped Win32 named pipe HANDLE supports concurrent full-duplex ReadFile/WriteFile
    // across threads when each call uses its own independent OVERLAPPED + h_event.
    unsafe impl Send for WinPipeHandle {}
    unsafe impl Sync for WinPipeHandle {}

    impl Drop for WinPipeHandle {
        fn drop(&mut self) {
            unsafe {
                CancelIoEx(self.handle, null_mut());
                CloseHandle(self.handle);
            }
        }
    }

    pub struct WinPipeReader {
        pipe: Arc<WinPipeHandle>,
        event: Handle,
    }

    unsafe impl Send for WinPipeReader {}

    impl Drop for WinPipeReader {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.event);
            }
        }
    }

    pub struct WinPipeWriter {
        pipe: Arc<WinPipeHandle>,
        event: Handle,
    }

    unsafe impl Send for WinPipeWriter {}

    impl Drop for WinPipeWriter {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.event);
            }
        }
    }

    pub fn connect(pipe_path: &str) -> io::Result<(WinPipeWriter, WinPipeReader)> {
        let wide: Vec<u16> = std::ffi::OsStr::new(pipe_path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                null(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                null_mut(),
            )
        };

        if handle == INVALID_HANDLE_VALUE && unsafe { GetLastError() } == ERROR_PIPE_BUSY {
            unsafe {
                WaitNamedPipeW(wide.as_ptr(), 200);
                handle = CreateFileW(
                    wide.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    0,
                    null(),
                    OPEN_EXISTING,
                    FILE_FLAG_OVERLAPPED,
                    null_mut(),
                );
            }
        }

        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }

        let pipe = Arc::new(WinPipeHandle { handle });
        let write_event = unsafe { CreateEventW(null(), 1, 0, null()) };
        if write_event.is_null() {
            return Err(io::Error::last_os_error());
        }
        let read_event = unsafe { CreateEventW(null(), 1, 0, null()) };
        if read_event.is_null() {
            let err = io::Error::last_os_error();
            unsafe {
                CloseHandle(write_event);
            }
            return Err(err);
        }

        Ok((
            WinPipeWriter {
                pipe: Arc::clone(&pipe),
                event: write_event,
            },
            WinPipeReader {
                pipe,
                event: read_event,
            },
        ))
    }

    fn finish_overlapped(handle: Handle, ov: &mut Overlapped, is_read: bool) -> io::Result<usize> {
        let mut transferred: u32 = 0;
        let ok = unsafe { GetOverlappedResult(handle, ov, &mut transferred, 1) };
        if ok != 0 {
            return Ok(transferred as usize);
        }
        let code = unsafe { GetLastError() };
        if is_read && code == ERROR_MORE_DATA && transferred > 0 {
            return Ok(transferred as usize);
        }
        if matches!(
            code,
            ERROR_BROKEN_PIPE | ERROR_NO_DATA | ERROR_PIPE_NOT_CONNECTED | ERROR_OPERATION_ABORTED
        ) {
            return if is_read {
                Ok(0)
            } else {
                Err(io::Error::from_raw_os_error(code as i32))
            };
        }
        Err(io::Error::from_raw_os_error(code as i32))
    }

    impl Read for WinPipeReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if buf.is_empty() {
                return Ok(0);
            }
            unsafe {
                ResetEvent(self.event);
            }
            let mut ov = Overlapped {
                internal: 0,
                internal_high: 0,
                offset: 0,
                offset_high: 0,
                h_event: self.event,
            };
            let chunk = buf.len().min(u32::MAX as usize) as u32;
            let ok = unsafe { ReadFile(self.pipe.handle, buf.as_mut_ptr(), chunk, null_mut(), &mut ov) };
            if ok != 0 {
                return finish_overlapped(self.pipe.handle, &mut ov, true);
            }
            let code = unsafe { GetLastError() };
            if code == ERROR_IO_PENDING {
                return finish_overlapped(self.pipe.handle, &mut ov, true);
            }
            if matches!(
                code,
                ERROR_BROKEN_PIPE | ERROR_NO_DATA | ERROR_PIPE_NOT_CONNECTED | ERROR_OPERATION_ABORTED
            ) {
                return Ok(0);
            }
            Err(io::Error::from_raw_os_error(code as i32))
        }
    }

    impl Write for WinPipeWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if buf.is_empty() {
                return Ok(0);
            }
            unsafe {
                ResetEvent(self.event);
            }
            let mut ov = Overlapped {
                internal: 0,
                internal_high: 0,
                offset: 0,
                offset_high: 0,
                h_event: self.event,
            };
            let chunk = buf.len().min(u32::MAX as usize) as u32;
            let ok = unsafe { WriteFile(self.pipe.handle, buf.as_ptr(), chunk, null_mut(), &mut ov) };
            if ok != 0 {
                return finish_overlapped(self.pipe.handle, &mut ov, false);
            }
            let code = unsafe { GetLastError() };
            if code == ERROR_IO_PENDING {
                return finish_overlapped(self.pipe.handle, &mut ov, false);
            }
            Err(io::Error::from_raw_os_error(code as i32))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}

#[cfg(windows)]
pub type IpcWriter = win_pipe::WinPipeWriter;
#[cfg(windows)]
pub type IpcReader = win_pipe::WinPipeReader;

fn connect_ipc(sock_path: &str) -> std::io::Result<(IpcWriter, IpcReader)> {
    #[cfg(unix)]
    {
        let writer = std::os::unix::net::UnixStream::connect(sock_path)?;
        let _ = writer.set_read_timeout(None);
        let _ = writer.set_write_timeout(None);
        let reader = writer.try_clone()?;
        let _ = reader.set_read_timeout(None);
        Ok((writer, reader))
    }
    #[cfg(windows)]
    {
        win_pipe::connect(sock_path)
    }
}

pub type EofCallback = Box<dyn Fn() + Send + Sync + 'static>;

pub struct MpvDriver {
    process: Mutex<Option<Child>>,
    sock_path: String,
    cmd_stream: Mutex<Option<IpcWriter>>,
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
            .arg("--vo=null")
            .arg("--no-config")
            .arg("--load-scripts=no")
            .arg("--ytdl=no")
            .arg(format!("--input-ipc-server={}", sock_path))
            .arg("--audio-buffer=0.2")
            .arg("--demuxer-max-bytes=24M")
            .arg("--demuxer-max-back-bytes=6M")
            .arg("--network-timeout=15")
            .arg("--volume-max=150")
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

        let mut child = cmd.spawn().context("Failed to spawn mpv background process")?;

        // Wait up to 8000ms for IPC socket/pipe creation, failing early if mpv exits
        let mut connected_cmd = None;
        for _ in 0..160 {
            if let Ok(pair) = connect_ipc(&sock_path) {
                connected_cmd = Some(pair);
                break;
            }
            if let Ok(Some(status)) = child.try_wait() {
                bail!("mpv process exited prematurely during startup ({})", status);
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        let (mut cmd_stream, reader_stream) =
            connected_cmd.context("Timed out connecting to mpv IPC socket")?;

        // Register property observers on connected IPC stream
        let obs1 = serde_json::json!({"command": ["observe_property", 1, "time-pos"]}).to_string();
        let obs2 = serde_json::json!({"command": ["observe_property", 2, "pause"]}).to_string();
        let obs3 = serde_json::json!({"command": ["observe_property", 3, "duration"]}).to_string();
        let _ = cmd_stream.write_all(format!("{}\n{}\n{}\n", obs1, obs2, obs3).as_bytes());

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
            let reader = BufReader::new(reader_stream);
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

        #[cfg(windows)]
        info!("MPV WASAPI audio backend ready");
        #[cfg(not(windows))]
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
            if let Ok((writer, _reader)) = connect_ipc(&self.sock_path) {
                *stream_guard = Some(writer);
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
        if let Ok((mut writer, _reader)) = connect_ipc(&self.sock_path) {
            if writer.write_all(payload.as_bytes()).is_ok() {
                *stream_guard = Some(writer);
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
        #[cfg(unix)]
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
        #[cfg(unix)]
        let _ = std::fs::remove_file(&self.sock_path);
    }
}
