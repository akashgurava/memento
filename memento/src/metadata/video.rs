use std::process::Command;
use std::sync::Once;

use super::MetadataEntry;

static FFPROBE_WARN: Once = Once::new();

/// Extract video metadata using ffprobe (must be in PATH)
pub fn extract_video_metadata(path: &str) -> Vec<MetadataEntry> {
    let mut entries = Vec::new();

    let output = match Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            path,
        ])
        .output()
    {
        Ok(o) => o,
        Err(_) => {
            FFPROBE_WARN.call_once(|| {
                tracing::warn!("EXTRACT_VIDEO_METADATA_INIT: SKIPPED. reason: ffprobe not found in PATH, video metadata will not be extracted");
            });
            return entries;
        }
    };

    if !output.status.success() {
        return entries;
    }

    let json: serde_json::Value = match serde_json::from_slice(&output.stdout) {
        Ok(v) => v,
        Err(_) => return entries,
    };

    // Extract format-level tags
    if let Some(format) = json.get("format") {
        if let Some(duration) = format.get("duration").and_then(|v| v.as_str()) {
            entries.push(("video".into(), "duration".into(), Some(duration.into())));
        }
        if let Some(bit_rate) = format.get("bit_rate").and_then(|v| v.as_str()) {
            entries.push(("video".into(), "bit_rate".into(), Some(bit_rate.into())));
        }
        if let Some(format_name) = format.get("format_name").and_then(|v| v.as_str()) {
            entries.push(("video".into(), "format_name".into(), Some(format_name.into())));
        }

        // Extract format tags (creation_time, etc.)
        if let Some(tags) = format.get("tags").and_then(|v| v.as_object()) {
            for (key, value) in tags {
                let text = value
                    .as_str()
                    .map(|s| s.to_string())
                    .or_else(|| Some(value.to_string()));
                entries.push(("video_tag".into(), key.clone(), text));
            }
        }
    }

    // Extract stream info
    if let Some(streams) = json.get("streams").and_then(|v| v.as_array()) {
        for (i, stream) in streams.iter().enumerate() {
            let prefix = format!("stream_{}", i);
            let codec_type = stream
                .get("codec_type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            entries.push((
                "video".into(),
                format!("{}_codec_type", prefix),
                Some(codec_type.into()),
            ));

            if let Some(codec_name) = stream.get("codec_name").and_then(|v| v.as_str()) {
                entries.push((
                    "video".into(),
                    format!("{}_codec_name", prefix),
                    Some(codec_name.into()),
                ));
            }

            if codec_type == "video" {
                if let Some(w) = stream.get("width").and_then(|v| v.as_i64()) {
                    entries.push((
                        "video".into(),
                        format!("{}_width", prefix),
                        Some(w.to_string()),
                    ));
                }
                if let Some(h) = stream.get("height").and_then(|v| v.as_i64()) {
                    entries.push((
                        "video".into(),
                        format!("{}_height", prefix),
                        Some(h.to_string()),
                    ));
                }
                if let Some(fps) = stream.get("r_frame_rate").and_then(|v| v.as_str()) {
                    entries.push((
                        "video".into(),
                        format!("{}_frame_rate", prefix),
                        Some(fps.into()),
                    ));
                }
            }

            if codec_type == "audio" {
                if let Some(sr) = stream.get("sample_rate").and_then(|v| v.as_str()) {
                    entries.push((
                        "video".into(),
                        format!("{}_sample_rate", prefix),
                        Some(sr.into()),
                    ));
                }
                if let Some(channels) = stream.get("channels").and_then(|v| v.as_i64()) {
                    entries.push((
                        "video".into(),
                        format!("{}_channels", prefix),
                        Some(channels.to_string()),
                    ));
                }
            }
        }
    }

    entries
}
