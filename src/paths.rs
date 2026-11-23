use std::path::PathBuf;

/// Returns the primary application data directory:
/// - Windows: %LOCALAPPDATA%\cloudpulse
/// - Linux: $XDG_DATA_HOME/cloudpulse or ~/.local/share/cloudpulse
pub fn data_dir() -> PathBuf {
    let base = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .or_else(|| {
            #[cfg(windows)]
            {
                std::env::var_os("LOCALAPPDATA")
                    .or_else(|| std::env::var_os("APPDATA"))
                    .or_else(|| std::env::var_os("USERPROFILE"))
                    .map(PathBuf::from)
            }
            #[cfg(not(windows))]
            {
                std::env::var_os("XDG_DATA_HOME")
                    .or_else(|| std::env::var_os("HOME"))
                    .map(PathBuf::from)
            }
        })
        .unwrap_or_else(|| PathBuf::from("."));

    let dir = base.join("cloudpulse");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Returns the primary cache directory:
/// - Windows: %LOCALAPPDATA%\cloudpulse\cache
/// - Linux: $XDG_CACHE_HOME/cloudpulse or ~/.cache/cloudpulse
pub fn cache_dir() -> PathBuf {
    let base = dirs::cache_dir()
        .unwrap_or_else(|| data_dir());

    let dir = base.join("cloudpulse");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Returns a suitable runtime / temporary directory for IPC communication:
/// - Windows: %TEMP%
/// - Linux: $XDG_RUNTIME_DIR or /tmp
pub fn runtime_dir() -> PathBuf {
    dirs::runtime_dir()
        .or_else(|| std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from))
        .unwrap_or_else(std::env::temp_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paths_exist_and_accessible() {
        let d = data_dir();
        assert!(d.exists(), "data_dir should exist: {:?}", d);

        let c = cache_dir();
        assert!(c.exists(), "cache_dir should exist: {:?}", c);

        let r = runtime_dir();
        assert!(r.exists(), "runtime_dir should exist: {:?}", r);
    }
}
