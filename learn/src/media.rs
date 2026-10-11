//! The videos of the chapters (#534).
//!
//! They are recorded by `learn/media/record.mjs` from the course app, encoded
//! by `encode.sh`, and uploaded to the server's disk: they are not in git. What
//! is committed is `content/media.json`, which lists every video with its
//! files' sizes and hashes. The `learn:video` directive renders from it, and
//! `tests/content.rs` checks that the chapters and the manifest agree.

use std::collections::BTreeMap;
use std::sync::{LazyLock, Once};

use renox::prelude::AppState;
use serde::Deserialize;

include!(concat!(env!("OUT_DIR"), "/media_json.rs"));

/// Where videos are served from when `LEARN_MEDIA_URL` is not set: Renox's
/// local-disk public route, `STORAGE_PATH/app/public/media/`.
pub const DEFAULT_URL: &str = "/storage/media";

/// What the chapters put where the media URL goes; the stage page replaces it
/// on each response with [`media_url`].
pub const PLACEHOLDER: &str = "__LEARN_MEDIA__";

/// `content/media.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// The file's format; 1.
    pub version: u32,
    /// Every video by its scene id, sorted.
    pub videos: BTreeMap<String, Video>,
}

/// One recorded video.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Video {
    /// The stage the scene belongs to.
    pub stage: u8,
    /// What the video shows.
    pub title: String,
    /// Pixels.
    pub width: u32,
    /// Pixels.
    pub height: u32,
    /// Seconds.
    pub duration: f64,
    /// The first frame readers see.
    pub poster: MediaFile,
    /// The encodings, best first (the browser plays the first it can).
    pub sources: Vec<MediaFile>,
    /// Hash of the scene file this was recorded from.
    pub scene_sha256: String,
    /// `YYYY-MM-DD`.
    pub recorded_at: String,
}

/// One file of a video, as uploaded.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaFile {
    /// The name under the media URL: `<id>.webm`.
    pub file: String,
    /// Its size in bytes (`upload.sh` compares it with `Content-Length`).
    pub bytes: u64,
    /// Its SHA-256, hex.
    pub sha256: String,
    /// Its MIME type (a poster has none).
    #[serde(rename = "type")]
    pub mime: Option<String>,
}

impl MediaFile {
    /// The file's URL under `base`, with `?v=` (8 hex digits of its hash) so a
    /// re-encoded file is not served from a cache.
    pub fn url(&self, base: &str) -> String {
        let v = self.sha256.get(..8).unwrap_or(&self.sha256);
        format!("{base}/{}?v={v}", self.file)
    }
}

/// Parses a manifest's text.
pub fn parse(text: &str) -> Result<Manifest, String> {
    let manifest: Manifest = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if manifest.version != 1 {
        return Err(format!("unknown manifest version {}", manifest.version));
    }
    Ok(manifest)
}

/// The compiled-in manifest.
pub fn manifest() -> &'static Manifest {
    static MANIFEST: LazyLock<Manifest> = LazyLock::new(|| {
        parse(MEDIA_JSON).unwrap_or_else(|e| panic!("content/media.json is not valid: {e}"))
    });
    &MANIFEST
}

/// Where videos are served from: `LEARN_MEDIA_URL` without a trailing `/`,
/// else [`DEFAULT_URL`]. It must start with `/` (the same origin, which the
/// default CSP allows for `<video>`); any other value logs an error once and
/// the default is used.
pub fn media_url(state: &AppState) -> String {
    let Some(value) = state.config.var("LEARN_MEDIA_URL") else {
        return DEFAULT_URL.to_owned();
    };
    let url = value.trim_end_matches('/');
    if url.starts_with('/') && !url.starts_with("//") {
        return url.to_owned();
    }
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        tracing::error!(
            "LEARN_MEDIA_URL must start with `/` (the same origin); using {DEFAULT_URL}"
        );
    });
    DEFAULT_URL.to_owned()
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// The figure of a `learn:video`: a player with each source, the poster, a
/// download link and the caption. `title` labels it for screen readers;
/// `base` is the media URL (the chapters use [`PLACEHOLDER`]).
pub fn figure(video: &Video, title: &str, caption: Option<&str>, base: &str) -> String {
    let mut out = format!(
        "<figure class=\"learn-video\"><video controls preload=\"none\" playsinline width=\"{}\" height=\"{}\" poster=\"{}\" aria-label=\"{}\">",
        video.width,
        video.height,
        escape(&video.poster.url(base)),
        escape(title)
    );
    for source in &video.sources {
        out.push_str(&format!(
            "<source src=\"{}\" type=\"{}\">",
            escape(&source.url(base)),
            escape(source.mime.as_deref().unwrap_or("video/mp4"))
        ));
    }
    // The last source (the most widely played) is the download.
    if let Some(last) = video.sources.last() {
        out.push_str(&format!(
            "<a href=\"{}\">Download the video</a>",
            escape(&last.url(base))
        ));
    }
    out.push_str("</video>");
    if let Some(caption) = caption {
        out.push_str(&format!("<figcaption>{}</figcaption>", escape(caption)));
    }
    out.push_str("</figure>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"version":1,"videos":{"s05-x":{"stage":5,"title":"T","width":1280,"height":720,"duration":3.5,
      "poster":{"file":"s05-x.webp","bytes":1,"sha256":"aaaaaaaa11"},
      "sources":[{"file":"s05-x.webm","type":"video/webm","bytes":2,"sha256":"bbbbbbbb22"},{"file":"s05-x.mp4","type":"video/mp4","bytes":3,"sha256":"cccccccc33"}],
      "scene_sha256":"d","recorded_at":"2026-10-11"}}}"#;

    #[test]
    fn the_committed_manifest_parses() {
        let m = manifest();
        assert_eq!(m.version, 1);
        for (id, v) in &m.videos {
            assert!(v.sources.iter().all(|s| s.file.starts_with(id.as_str())));
        }
    }

    #[test]
    fn unknown_keys_and_versions_are_refused() {
        assert!(parse(SAMPLE).is_ok());
        assert!(parse(&SAMPLE.replace("\"stage\":5,", "\"stage\":5,\"extra\":1,")).is_err());
        assert!(parse(&SAMPLE.replace("\"version\":1", "\"version\":2")).is_err());
    }

    #[test]
    fn the_figure_has_both_sources_the_poster_and_a_version_per_file() {
        let m = parse(SAMPLE).unwrap();
        let html = figure(&m.videos["s05-x"], "Adding \"x\"", Some("Cap <1>"), "/m");
        assert!(html.starts_with("<figure class=\"learn-video\"><video controls preload=\"none\" playsinline width=\"1280\" height=\"720\" poster=\"/m/s05-x.webp?v=aaaaaaaa\" aria-label=\"Adding &quot;x&quot;\">"), "{html}");
        assert!(html.contains("<source src=\"/m/s05-x.webm?v=bbbbbbbb\" type=\"video/webm\">"));
        assert!(html.contains("<source src=\"/m/s05-x.mp4?v=cccccccc\" type=\"video/mp4\">"));
        assert!(
            html.contains("<a href=\"/m/s05-x.mp4?v=cccccccc\">Download the video</a></video>")
        );
        assert!(html.ends_with("<figcaption>Cap &lt;1&gt;</figcaption></figure>"));
        assert!(!html.contains("autoplay"));
    }
}
