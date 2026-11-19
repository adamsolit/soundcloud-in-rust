use reqwest::Client;
use serde::Deserialize;
use tracing::debug;

#[derive(Debug, Deserialize)]
struct WaveformResponse {
    samples: Vec<i32>,
    #[serde(default)]
    height: Option<i32>,
}

pub async fn fetch_waveform_samples(
    http: &Client,
    waveform_url: Option<&str>,
    track_id: u64,
) -> Vec<f32> {
    if let Some(url) = waveform_url {
        let json_url = if url.ends_with(".png") {
            url.replace(".png", ".json")
        } else {
            url.to_string()
        };

        if let Ok(resp) = http.get(&json_url).send().await {
            if let Ok(waveform_data) = resp.json::<WaveformResponse>().await {
                if !waveform_data.samples.is_empty() {
                    let max_sample = waveform_data
                        .height
                        .unwrap_or_else(|| *waveform_data.samples.iter().max().unwrap_or(&100))
                        as f32;
                    let divisor = if max_sample > 0.0 { max_sample } else { 100.0 };

                    let normalized: Vec<f32> = waveform_data
                        .samples
                        .iter()
                        .map(|&s| (s as f32 / divisor).clamp(0.02, 1.0))
                        .collect();
                    debug!("Parsed {} real waveform points from CDN", normalized.len());
                    return normalized;
                }
            }
        }
    }

    // Procedural deterministic waveform fallback matching acoustic dynamic profile
    generate_synthetic_waveform(track_id, 140)
}

pub fn generate_synthetic_waveform(track_id: u64, points: usize) -> Vec<f32> {
    let mut samples = Vec::with_capacity(points);
    let seed = track_id as f32;
    for i in 0..points {
        let phase = i as f32 / points as f32;
        let envelope = (std::f32::consts::PI * phase).sin().powf(0.85);
        let detail1 = ((phase * 42.0 + seed * 0.13).sin() * 0.25).abs();
        let detail2 = ((phase * 115.0 + seed * 0.77).cos() * 0.15).abs();
        let base = 0.15 + 0.65 * envelope * (0.6 + detail1 + detail2);
        samples.push(base.clamp(0.08, 1.0));
    }
    samples
}
