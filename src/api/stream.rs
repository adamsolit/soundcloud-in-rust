use anyhow::{bail, Result};
use reqwest::Client;
use tracing::{debug, info};

use crate::models::track::{StreamResolutionResponse, Track, Transcoding};

pub async fn resolve_stream_url(
    http: &Client,
    track: &Track,
    client_id: &str,
    oauth_token: Option<&str>,
) -> Result<String> {
    let transcodings = match track.media.as_ref() {
        Some(m) if !m.transcodings.is_empty() => &m.transcodings,
        _ => bail!("No valid audio transcodings found on track {}", track.id),
    };

    // Rank candidates: progressive mp3 > hls aac/mp4 > hls ogg > other
    let mut candidates: Vec<&Transcoding> = transcodings.iter().collect();
    candidates.sort_by_key(|t| {
        let is_prog_mp3 = t.format.protocol == "progressive" && t.format.mime_type.contains("mpeg");
        let is_hls_mp4 = t.format.protocol == "hls" && (t.format.mime_type.contains("mp4") || t.format.mime_type.contains("aac"));
        let is_hls_ogg = t.format.protocol == "hls" && t.format.mime_type.contains("ogg");
        let is_mpegurl = t.format.mime_type.contains("mpegurl");

        if is_prog_mp3 {
            0
        } else if is_hls_mp4 {
            1
        } else if is_hls_ogg {
            2
        } else if !is_mpegurl {
            3
        } else {
            4
        }
    });

    for transcoding in candidates {
        let base_url = &transcoding.url;
        let separator = if base_url.contains('?') { '&' } else { '?' };
        let request_url = format!("{}{}client_id={}", base_url, separator, client_id);

        debug!(
            "Attempting transcoding stream: protocol={}, mime={}, url={}",
            transcoding.format.protocol, transcoding.format.mime_type, request_url
        );

        let mut req = http.get(&request_url);
        if let Some(token) = oauth_token {
            req = req.header("Authorization", format!("OAuth {}", token));
        }

        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(payload) = resp.json::<StreamResolutionResponse>().await {
                    if !payload.url.is_empty() {
                        info!(
                            "Successfully resolved playable stream URL for track {} (protocol: {}, mime: {})",
                            track.id, transcoding.format.protocol, transcoding.format.mime_type
                        );
                        return Ok(payload.url);
                    }
                }
            }
            Ok(resp) => {
                debug!(
                    "Transcoding {} returned HTTP {}, trying next candidate...",
                    transcoding.format.mime_type,
                    resp.status()
                );
            }
            Err(e) => {
                debug!("Network error requesting transcoding: {}, trying next...", e);
            }
        }
    }

    bail!("Failed to resolve any valid stream URL for track {}", track.id)
}
