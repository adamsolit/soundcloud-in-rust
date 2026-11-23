use gdk4 as gdk;
use gtk4 as gtk;
use gtk::gio;
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::LazyLock;
use tracing::{debug, warn};

static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .pool_max_idle_per_host(10)
        .build()
        .unwrap_or_default()
});

const MAX_MEM_TEXTURES: usize = 40;

struct TextureCache {
    map: HashMap<String, gdk::Texture>,
    order: VecDeque<String>,
}

impl TextureCache {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    fn get(&self, key: &str) -> Option<gdk::Texture> {
        self.map.get(key).cloned()
    }

    fn insert(&mut self, key: String, texture: gdk::Texture) {
        if self.map.contains_key(&key) {
            self.map.insert(key, texture);
            return;
        }

        while self.order.len() >= MAX_MEM_TEXTURES {
            if let Some(old_key) = self.order.pop_front() {
                self.map.remove(&old_key);
            }
        }

        self.order.push_back(key.clone());
        self.map.insert(key, texture);
    }
}

thread_local! {
    static MEM_CACHE: RefCell<TextureCache> = RefCell::new(TextureCache::new());
}

pub fn get_cache_dir() -> PathBuf {
    let dir = crate::paths::cache_dir().join("artwork");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn hash_url(url: &str) -> String {
    let mut h = 0xcbf29ce484222325u64;
    for b in url.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}

/// Loads artwork asynchronously into a `gtk::Picture`.
/// Checks memory cache, then disk cache, then fetches over HTTP.
pub fn load_into_picture(picture: &gtk::Picture, url: Option<&str>) {
    let Some(url_str) = url.filter(|s| !s.trim().is_empty()) else {
        picture.set_paintable(None::<&gdk::Texture>);
        return;
    };

    let url_key = url_str.to_string();

    // 1. Check in-memory texture cache
    let cached_texture = MEM_CACHE.with(|cache| cache.borrow().get(&url_key));
    if let Some(texture) = cached_texture {
        picture.set_paintable(Some(&texture));
        return;
    }

    // 2. Check disk cache
    let ext = if url_key.contains(".png") { "png" } else { "jpg" };
    let file_path = get_cache_dir().join(format!("{}.{}", hash_url(&url_key), ext));

    if file_path.exists() {
        let gio_file = gio::File::for_path(&file_path);
        if let Ok(texture) = gdk::Texture::from_file(&gio_file) {
            MEM_CACHE.with(|cache| {
                cache.borrow_mut().insert(url_key.clone(), texture.clone());
            });
            picture.set_paintable(Some(&texture));
            return;
        }
    }

    // 3. Asynchronously download via HTTP
    let picture_weak = picture.downgrade();
    let url_to_fetch = url_key.clone();

    glib::spawn_future_local(async move {
        match HTTP_CLIENT.get(&url_to_fetch).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(bytes) = resp.bytes().await {
                    let _ = tokio::fs::write(&file_path, &bytes).await;
                    let glib_bytes = glib::Bytes::from_owned(bytes);
                    match gdk::Texture::from_bytes(&glib_bytes) {
                        Ok(texture) => {
                            MEM_CACHE.with(|cache| {
                                cache.borrow_mut().insert(url_to_fetch, texture.clone());
                            });
                            if let Some(pic) = picture_weak.upgrade() {
                                pic.set_paintable(Some(&texture));
                            }
                        }
                        Err(e) => {
                            warn!("Failed to decode texture from bytes: {}", e);
                        }
                    }
                }
            }
            Ok(resp) => {
                debug!("Failed to download image HTTP {}: {}", resp.status(), url_to_fetch);
            }
            Err(e) => {
                debug!("Network error downloading artwork {}: {}", url_to_fetch, e);
            }
        }
    });
}

/// Loads artwork asynchronously into a `gtk::Image`.
/// If URL is None or download fails, resets to `fallback_icon`.
pub fn load_into_image(image: &gtk::Image, url: Option<&str>, fallback_icon: &'static str) {
    let Some(url_str) = url.filter(|s| !s.trim().is_empty()) else {
        image.set_icon_name(Some(fallback_icon));
        return;
    };

    let url_key = url_str.to_string();

    // 1. Check in-memory texture cache
    let cached_texture = MEM_CACHE.with(|cache| cache.borrow().get(&url_key));
    if let Some(texture) = cached_texture {
        image.set_paintable(Some(&texture));
        return;
    }

    // 2. Check disk cache
    let ext = if url_key.contains(".png") { "png" } else { "jpg" };
    let file_path = get_cache_dir().join(format!("{}.{}", hash_url(&url_key), ext));

    if file_path.exists() {
        let gio_file = gio::File::for_path(&file_path);
        if let Ok(texture) = gdk::Texture::from_file(&gio_file) {
            MEM_CACHE.with(|cache| {
                cache.borrow_mut().insert(url_key.clone(), texture.clone());
            });
            image.set_paintable(Some(&texture));
            return;
        }
    }

    // Set fallback while downloading
    image.set_icon_name(Some(fallback_icon));

    // 3. Asynchronously download via HTTP
    let image_weak = image.downgrade();
    let url_to_fetch = url_key.clone();

    glib::spawn_future_local(async move {
        match HTTP_CLIENT.get(&url_to_fetch).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(bytes) = resp.bytes().await {
                    let _ = tokio::fs::write(&file_path, &bytes).await;
                    let glib_bytes = glib::Bytes::from_owned(bytes);
                    match gdk::Texture::from_bytes(&glib_bytes) {
                        Ok(texture) => {
                            MEM_CACHE.with(|cache| {
                                cache.borrow_mut().insert(url_to_fetch, texture.clone());
                            });
                            if let Some(img) = image_weak.upgrade() {
                                img.set_paintable(Some(&texture));
                            }
                        }
                        Err(e) => {
                            warn!("Failed to decode texture from bytes: {}", e);
                        }
                    }
                }
            }
            Ok(resp) => {
                debug!("Failed to download image HTTP {}: {}", resp.status(), url_to_fetch);
            }
            Err(e) => {
                debug!("Network error downloading artwork {}: {}", url_to_fetch, e);
            }
        }
    });
}
