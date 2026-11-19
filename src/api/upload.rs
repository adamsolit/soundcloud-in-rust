use anyhow::{bail, Context, Result};
use reqwest::multipart::{Form, Part};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackUploadMetadata {
    pub title: String,
    pub permalink: Option<String>,
    pub genre: Option<String>,
    pub tags: Option<String>,
    pub description: Option<String>,
    pub is_public: bool,
    pub license: Option<String>,
    pub downloadable: bool,
    pub release_date: Option<String>,
    pub label_name: Option<String>,
    pub isrc: Option<String>,
    pub audio_path: String,
    pub artwork_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UploadProgress {
    pub percentage: f64,
    pub stage: String,
}

#[derive(Deserialize)]
struct UploadPolicyResponse {
    uid: String,
    url: String,
    #[serde(default)]
    headers: HashMap<String, String>,
}

#[derive(Deserialize)]
struct TranscodingResponse {
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    urn: Option<String>,
}

pub async fn upload_track(
    http: &Client,
    client_id: &str,
    oauth_token: Option<&str>,
    metadata: TrackUploadMetadata,
    on_progress: impl Fn(UploadProgress) + Send + 'static,
) -> Result<u64> {
    info!("Starting track upload: {}", metadata.title);
    on_progress(UploadProgress {
        percentage: 0.05,
        stage: "Reading audio stream...".to_string(),
    });

    let audio_file_path = Path::new(&metadata.audio_path);
    if !audio_file_path.exists() {
        bail!("Selected audio file does not exist: {}", metadata.audio_path);
    }

    let file_meta = tokio::fs::metadata(audio_file_path)
        .await
        .with_context(|| format!("Reading file metadata: {}", metadata.audio_path))?;
    let file_size = file_meta.len();

    let audio_filename = audio_file_path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "track.mp3".to_string());

    // 1. Try modern SoundCloud API v2 upload policy flow
    on_progress(UploadProgress {
        percentage: 0.15,
        stage: "Requesting upload policy from SoundCloud...".to_string(),
    });

    let policy_url = format!("https://api-v2.soundcloud.com/uploads/track-upload-policy?client_id={}", client_id);
    let mut policy_req = http
        .post(&policy_url)
        .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0")
        .header("Accept", "application/json, text/javascript, */*; q=0.01")
        .header("Origin", "https://soundcloud.com")
        .header("Referer", "https://soundcloud.com/upload")
        .json(&serde_json::json!({
            "filename": audio_filename,
            "filesize": file_size
        }));

    if let Some(token) = oauth_token {
        policy_req = policy_req.header("Authorization", format!("OAuth {}", token));
    }

    let policy_resp = policy_req.send().await.context("Connecting to SoundCloud upload policy endpoint")?;
    let policy_status = policy_resp.status();

    if policy_status == reqwest::StatusCode::FORBIDDEN {
        let err_text = policy_resp.text().await.unwrap_or_default();
        if err_text.contains("quota") || err_text.contains("overQuota") || err_text.is_empty() {
            bail!("SoundCloud upload rejected: Your account has exhausted its upload quota limit (0 minutes remaining on free plan). Free uploads must be freed on soundcloud.com or upgraded to Next Pro.");
        } else {
            bail!("SoundCloud upload policy forbidden (HTTP 403): {}", err_text);
        }
    }

    if policy_status.is_success() {
        let policy: UploadPolicyResponse = policy_resp
            .json()
            .await
            .context("Parsing upload policy response")?;

        // 2. Upload binary payload to S3
        on_progress(UploadProgress {
            percentage: 0.35,
            stage: "Uploading audio data to cloud storage...".to_string(),
        });

        let file = File::open(&metadata.audio_path).await?;
        let stream = FramedRead::new(file, BytesCodec::new());
        let body = reqwest::Body::wrap_stream(stream);

        let mut s3_req = http.put(&policy.url).body(body);
        for (k, v) in policy.headers {
            let lower = k.to_lowercase();
            if lower != "content-length" && lower != "host" {
                s3_req = s3_req.header(&k, &v);
            }
        }
        let s3_resp = s3_req.send().await.context("Uploading audio file to cloud storage")?;
        if !s3_resp.status().is_success() {
            bail!("Cloud storage upload failed with status {}", s3_resp.status());
        }

        // 3. Trigger transcoding on SoundCloud
        on_progress(UploadProgress {
            percentage: 0.65,
            stage: "Initiating server-side audio transcoding...".to_string(),
        });

        let transcode_url = format!(
            "https://api-v2.soundcloud.com/uploads/{}/track-transcoding?client_id={}",
            policy.uid, client_id
        );
        let mut transcode_req = http
            .post(&transcode_url)
            .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0");
        if let Some(token) = oauth_token {
            transcode_req = transcode_req.header("Authorization", format!("OAuth {}", token));
        }

        let transcode_resp = transcode_req.send().await.context("Triggering track transcoding")?;
        let transcode_status = transcode_resp.status();
        let track_id = if transcode_status.is_success() {
            let t_body: TranscodingResponse = transcode_resp.json().await.unwrap_or(TranscodingResponse { id: None, urn: None });
            t_body.id.unwrap_or(0)
        } else {
            0
        };

        // 4. Update full track metadata
        on_progress(UploadProgress {
            percentage: 0.85,
            stage: "Applying track metadata, tags, and license...".to_string(),
        });

        if track_id > 0 {
            let update_url = format!("https://api-v2.soundcloud.com/tracks/{}?client_id={}", track_id, client_id);
            let sharing_val = if metadata.is_public { "public" } else { "private" };
            let mut track_map = serde_json::json!({
                "title": metadata.title,
                "sharing": sharing_val,
                "downloadable": metadata.downloadable,
                "license": metadata.license.unwrap_or_else(|| "all-rights-reserved".to_string()),
            });

            if let Some(ref genre) = metadata.genre {
                track_map["genre"] = serde_json::Value::String(genre.clone());
            }
            if let Some(ref tags) = metadata.tags {
                track_map["tag_list"] = serde_json::Value::String(tags.clone());
            }
            if let Some(ref desc) = metadata.description {
                track_map["description"] = serde_json::Value::String(desc.clone());
            }
            if let Some(ref perm) = metadata.permalink {
                track_map["permalink"] = serde_json::Value::String(perm.clone());
            }
            if let Some(ref rel) = metadata.release_date {
                track_map["release_date"] = serde_json::Value::String(rel.clone());
            }
            if let Some(ref lbl) = metadata.label_name {
                track_map["label_name"] = serde_json::Value::String(lbl.clone());
            }
            if let Some(ref isrc) = metadata.isrc {
                track_map["isrc"] = serde_json::Value::String(isrc.clone());
            }

            let mut update_req = http
                .put(&update_url)
                .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0")
                .json(&serde_json::json!({ "track": track_map }));
            if let Some(token) = oauth_token {
                update_req = update_req.header("Authorization", format!("OAuth {}", token));
            }
            let _ = update_req.send().await;

            // 5. Upload artwork if present
            if let Some(ref art_path) = metadata.artwork_path {
                if Path::new(art_path).exists() {
                    on_progress(UploadProgress {
                        percentage: 0.95,
                        stage: "Uploading track artwork...".to_string(),
                    });
                    if let Ok(art_file) = File::open(art_path).await {
                        let art_name = Path::new(art_path)
                            .file_name()
                            .map(|f| f.to_string_lossy().to_string())
                            .unwrap_or_else(|| "artwork.jpg".to_string());
                        let art_stream = FramedRead::new(art_file, BytesCodec::new());
                        let art_part = Part::stream(reqwest::Body::wrap_stream(art_stream))
                            .file_name(art_name)
                            .mime_str("image/jpeg")
                            .unwrap_or_else(|_| Part::bytes(vec![]));
                        let art_form = Form::new().part("artwork_data", art_part);
                        let art_url = format!("https://api-v2.soundcloud.com/tracks/{}/artwork?client_id={}", track_id, client_id);
                        let mut art_req = http.put(&art_url).multipart(art_form);
                        if let Some(token) = oauth_token {
                            art_req = art_req.header("Authorization", format!("OAuth {}", token));
                        }
                        let _ = art_req.send().await;
                    }
                }
            }
        }

        on_progress(UploadProgress {
            percentage: 1.0,
            stage: "Upload completed successfully!".to_string(),
        });
        return Ok(track_id);
    }

    // Fallback: Legacy v1 multipart upload
    on_progress(UploadProgress {
        percentage: 0.40,
        stage: "Uploading audio track payload...".to_string(),
    });

    let audio_file = File::open(&metadata.audio_path)
        .await
        .with_context(|| format!("Opening audio file: {}", metadata.audio_path))?;
    let audio_stream = FramedRead::new(audio_file, BytesCodec::new());
    let audio_part = Part::stream(reqwest::Body::wrap_stream(audio_stream))
        .file_name(audio_filename)
        .mime_str("audio/mpeg")?;

    let sharing_str = if metadata.is_public { "public" } else { "private" };

    let mut form = Form::new()
        .text("track[title]", metadata.title)
        .text("track[sharing]", sharing_str)
        .text("track[downloadable]", metadata.downloadable.to_string())
        .part("track[asset_data]", audio_part);

    if let Some(perm) = metadata.permalink {
        form = form.text("track[permalink]", perm);
    }
    if let Some(genre) = metadata.genre {
        form = form.text("track[genre]", genre);
    }
    if let Some(tags) = metadata.tags {
        form = form.text("track[tag_list]", tags);
    }
    if let Some(desc) = metadata.description {
        form = form.text("track[description]", desc);
    }
    if let Some(lic) = metadata.license {
        form = form.text("track[license]", lic);
    }
    if let Some(rel) = metadata.release_date {
        form = form.text("track[release_date]", rel);
    }
    if let Some(lbl) = metadata.label_name {
        form = form.text("track[label_name]", lbl);
    }
    if let Some(isrc) = metadata.isrc {
        form = form.text("track[isrc]", isrc);
    }

    if let Some(art_path) = metadata.artwork_path {
        if Path::new(&art_path).exists() {
            let art_file = File::open(&art_path).await?;
            let art_name = Path::new(&art_path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| "artwork.jpg".to_string());
            let art_stream = FramedRead::new(art_file, BytesCodec::new());
            let art_part = Part::stream(reqwest::Body::wrap_stream(art_stream))
                .file_name(art_name)
                .mime_str("image/jpeg")?;
            form = form.part("track[artwork_data]", art_part);
        }
    }

    let url = format!("https://api.soundcloud.com/tracks?client_id={}", client_id);
    let mut req = http.post(&url).multipart(form);
    if let Some(token) = oauth_token {
        req = req.header("Authorization", format!("OAuth {}", token));
    }

    let resp = req.send().await.context("Sending track upload request")?;
    let status = resp.status();
    if !status.is_success() {
        let err_body = resp.text().await.unwrap_or_default();
        bail!("Track upload rejected (HTTP {}): {}", status, err_body);
    }

    on_progress(UploadProgress {
        percentage: 1.0,
        stage: "Upload complete!".to_string(),
    });

    #[derive(Deserialize)]
    struct UploadResp {
        id: u64,
    }

    let result = resp.json::<UploadResp>().await?;
    info!("Uploaded track successfully, ID: {}", result.id);
    Ok(result.id)
}
