//! Download history (`history.json`) and the cover art cache (`covers/`).
//! Blocking; background threads only.

use crate::store::{self, Paths};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SidecarInfo {
    pub exists: bool,
    /// Detected sidecar files alongside the audio file: (short badge label, full path).
    pub sidecars: Vec<(String, String)>,
}

/// Checks whether each audio file exists on disk and discovers sibling sidecar files
/// (`.lrc`, `.ttml`, `.srt`, `.json`) in one directory scan per unique parent folder.
pub fn inspect_files(audio_paths: &[String]) -> BTreeMap<String, SidecarInfo> {
    let mut dir_cache: BTreeMap<PathBuf, Vec<(String, &'static str, PathBuf)>> = BTreeMap::new();
    let mut results = BTreeMap::new();
    for raw in audio_paths {
        if results.contains_key(raw) {
            continue;
        }
        let path = Path::new(raw);
        let exists = path.is_file();
        let mut sidecars = Vec::new();
        if let (Some(parent), Some(stem)) = (path.parent(), path.file_stem().and_then(|s| s.to_str())) {
            let entries = dir_cache.entry(parent.to_path_buf()).or_insert_with(|| {
                fs::read_dir(parent)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter_map(|entry| {
                        let name_os = entry.file_name();
                        let name = name_os.to_str()?;
                        let label = sidecar_label(Path::new(name))?;
                        let is_file = entry
                            .file_type()
                            .map(|ft| ft.is_file() || (ft.is_symlink() && entry.path().is_file()))
                            .unwrap_or(false);
                        if !is_file {
                            return None;
                        }
                        Some((name.to_lowercase(), label, entry.path()))
                    })
                    .collect()
            });
            let prefix = format!("{}.", stem.to_lowercase());
            let mut seen_labels = BTreeSet::new();
            for (lower, label, candidate) in entries {
                let Some(rest) = lower.strip_prefix(&prefix) else {
                    continue;
                };
                let matches_exact = match *label {
                    "Meta" => rest == "json" || rest.split_once('.').is_some_and(|(tag, ext)| !tag.is_empty() && !tag.contains('.') && ext == "json"),
                    _ => !rest.contains('.'),
                };
                if !matches_exact {
                    continue;
                }
                if seen_labels.insert(*label) {
                    sidecars.push(((*label).to_owned(), candidate.to_string_lossy().into_owned()));
                }
            }
            sidecars.sort_by(|a, b| a.0.cmp(&b.0));
        }
        results.insert(raw.clone(), SidecarInfo { exists, sidecars });
    }
    results
}

fn sidecar_label(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?;
    if ext.eq_ignore_ascii_case("lrc") {
        Some("LRC")
    } else if ext.eq_ignore_ascii_case("ttml") {
        Some("TTML")
    } else if ext.eq_ignore_ascii_case("srt") {
        Some("SRT")
    } else if ext.eq_ignore_ascii_case("json") {
        Some("Meta")
    } else {
        None
    }
}

/// Technical audio specs and Vorbis tags read on demand for the Library Inspector.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrackSpecs {
    pub path: String,
    pub size_bytes: u64,
    pub sample_rate: Option<u32>,
    pub bits_per_sample: Option<u8>,
    pub channels: Option<u8>,
    pub duration_secs: Option<u64>,
    pub cover_summary: Option<String>,
    pub tags: Vec<(String, String)>,
}

pub fn read_track_specs(path: &Path) -> Option<TrackSpecs> {
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let mut specs = TrackSpecs {
        path: path.to_string_lossy().into_owned(),
        size_bytes: meta.len(),
        ..TrackSpecs::default()
    };
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("flac"))
        && let Ok(tag) = metaflac::Tag::read_from_path(path)
    {
        if let Some(info) = tag.get_streaminfo() {
            specs.sample_rate = Some(info.sample_rate);
            specs.bits_per_sample = Some(info.bits_per_sample);
            specs.channels = Some(info.num_channels);
            if info.sample_rate > 0 {
                specs.duration_secs = Some(info.total_samples / u64::from(info.sample_rate));
            }
        }
        if let Some(pic) = tag
            .pictures()
            .find(|p| p.picture_type == metaflac::block::PictureType::CoverFront)
            .or_else(|| tag.pictures().next())
        {
            let kb = (pic.data.len() as u64).div_ceil(1024);
            let dims = if pic.width > 0 && pic.height > 0 {
                format!("{}x{}, ", pic.width, pic.height)
            } else {
                String::new()
            };
            specs.cover_summary = Some(format!("{dims}{} ({kb} KB)", pic.mime_type));
        }
        for (key, label) in [
            ("TITLE", "Title"),
            ("ARTIST", "Artist"),
            ("ALBUMARTIST", "Album Artist"),
            ("ALBUM", "Album"),
            ("DATE", "Date"),
            ("TRACKNUMBER", "Track"),
            ("DISCNUMBER", "Disc"),
            ("GENRE", "Genre"),
            ("ISRC", "ISRC"),
        ] {
            if let Some(values) = tag.get_vorbis(key) {
                let joined: Vec<&str> = values.map(str::trim).filter(|v| !v.is_empty()).collect();
                if !joined.is_empty() {
                    specs.tags.push((label.to_owned(), joined.join(", ")));
                }
            }
        }
    }
    Some(specs)
}

/// History entries kept on disk.
pub const LIMIT: usize = 500;
const MAX_COVER: u64 = 10 << 20;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Entry {
    pub title: String,
    pub artist: String,
    #[serde(default)]
    pub album: String,
    /// Service display name, e.g. "Tidal".
    #[serde(default)]
    pub service: String,
    /// Extension display name.
    #[serde(default)]
    pub extension: String,
    #[serde(default)]
    pub quality: String,
    pub url: String,
    /// Saved files; the first audio file is the main one.
    pub files: Vec<String>,
    /// Cached cover art file.
    #[serde(default)]
    pub cover: Option<String>,
    /// Unix seconds.
    pub date: u64,
}

impl Entry {
    pub fn flac_file(&self) -> Option<&str> {
        self.files.iter().find(|file| {
            Path::new(file).extension().is_some_and(|ext| ext.eq_ignore_ascii_case("flac"))
        }).map(String::as_str)
    }

    /// The audio file to play or reveal.
    pub fn main_file(&self) -> Option<&str> {
        self.files.iter().find(|f| is_audio_file(f)).or(self.files.first()).map(String::as_str)
    }
}

pub fn is_audio_file(path: &str) -> bool {
    Path::new(path).extension().is_some_and(|ext| {
        ["flac", "m4a", "mp3", "ogg", "wav", "opus"]
            .iter()
            .any(|candidate| ext.eq_ignore_ascii_case(candidate))
    })
}

pub fn load(paths: &Paths) -> Vec<Entry> {
    let history_path = paths.history();
    let Ok(text) = fs::read_to_string(&history_path) else {
        return Vec::new();
    };
    match serde_json::from_str(&text) {
        Ok(entries) => entries,
        Err(_) => {
            store::backup_corrupt(&history_path);
            Vec::new()
        }
    }
}

/// Newest first, at most `LIMIT` entries.
pub fn save(paths: &Paths, entries: &[Entry]) -> Result<(), String> {
    store::with_lock(paths, || {
        fs::create_dir_all(&paths.root).map_err(|e| e.to_string())?;
        let kept = &entries[..entries.len().min(LIMIT)];
        let json = serde_json::to_string_pretty(kept).map_err(|e| e.to_string())?;
        store::write_atomic(&paths.history(), json.as_bytes())
    })
}

/// Exports library entries to CSV (`.csv`) or pretty JSON (all other extensions).
pub fn export_entries(path: &Path, entries: &[Entry]) -> Result<usize, String> {
    let is_csv = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("csv"));
    let payload = if is_csv {
        let mut out = String::from("title,artist,album,service,extension,quality,url,file,date\n");
        for entry in entries {
            let fields = [
                entry.title.as_str(),
                entry.artist.as_str(),
                entry.album.as_str(),
                entry.service.as_str(),
                entry.extension.as_str(),
                entry.quality.as_str(),
                entry.url.as_str(),
                entry.main_file().unwrap_or(""),
            ];
            for field in fields {
                write_csv_field(&mut out, field);
                out.push(',');
            }
            out.push_str(&entry.date.to_string());
            out.push('\n');
        }
        out.into_bytes()
    } else {
        serde_json::to_string_pretty(entries)
            .map_err(|e| e.to_string())?
            .into_bytes()
    };
    store::write_atomic(path, &payload)?;
    Ok(entries.len())
}

fn write_csv_field(out: &mut String, field: &str) {
    let needs_quotes = field.contains([',', '"', '\n', '\r']);
    if !needs_quotes {
        out.push_str(field);
        return;
    }
    out.push('"');
    for ch in field.chars() {
        if ch == '"' {
            out.push('"');
        }
        out.push(ch);
    }
    out.push('"');
}

/// Downloads cover art into the cache: HTTPS, JPEG, PNG, or WebP, at most 10 MB.
pub fn cover_from_url(paths: &Paths, url: &str) -> Option<(PathBuf, Vec<u8>)> {
    cache(paths, fetch_image(&store::client().ok()?, url, MAX_COVER)?)
}

pub fn fetch_image(client: &reqwest::blocking::Client, url: &str, limit: u64) -> Option<Vec<u8>> {
    let parsed = reqwest::Url::parse(url).ok()?;
    if parsed.scheme() != "https" || !parsed.username().is_empty() || parsed.password().is_some() {
        return None;
    }
    let response = client.get(parsed).send().ok()?.error_for_status().ok()?;
    let mut bytes = Vec::new();
    response.take(limit + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= limit && is_image(&bytes)).then_some(bytes)
}

/// The front cover embedded in a FLAC file (any picture when there is no front cover).
pub fn cover_from_flac(paths: &Paths, file: &Path) -> Option<(PathBuf, Vec<u8>)> {
    let tag = metaflac::Tag::read_from_path(file).ok()?;
    cache_embedded_cover(paths, &tag)
}

pub struct TaggedTrack {
    pub file: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub cover: Option<(PathBuf, Vec<u8>)>,
}

pub fn read_tagged_track(paths: &Paths, file: String) -> Result<TaggedTrack, String> {
    let tag = metaflac::Tag::read_from_path(&file).map_err(|_| "Could not read the updated FLAC")?;
    let field = |key| tag.get_vorbis(key)
        .and_then(|mut values| values.find(|value| !value.trim().is_empty()))
        .unwrap_or_default().to_owned();
    Ok(TaggedTrack {
        file,
        title: field("TITLE"),
        artist: field("ARTIST"),
        album: field("ALBUM"),
        cover: cache_embedded_cover(paths, &tag),
    })
}

fn cache_embedded_cover(paths: &Paths, tag: &metaflac::Tag) -> Option<(PathBuf, Vec<u8>)> {
    let pictures: Vec<_> = tag.pictures().collect();
    let picture = pictures
        .iter()
        .find(|p| p.picture_type == metaflac::block::PictureType::CoverFront)
        .or(pictures.first())?;
    cache(paths, picture.data.clone())
}

/// Reads a cached cover, ensuring it stays within `paths.covers()` and the size limit.
pub fn read_cover(paths: &Paths, path: &str) -> Option<Vec<u8>> {
    let covers_dir = paths.covers().canonicalize().ok()?;
    let candidate = Path::new(path).canonicalize().ok()?;
    if !candidate.starts_with(&covers_dir) {
        return None;
    }
    let metadata = fs::metadata(&candidate).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_COVER {
        return None;
    }
    let bytes = fs::read(&candidate).ok()?;
    is_image(&bytes).then_some(bytes)
}

/// Removes cached cover files in `paths.covers()` that are no longer referenced.
pub fn prune_unused_covers(paths: &Paths, keep: &[String]) {
    let Ok(entries) = fs::read_dir(paths.covers()) else { return };
    let kept: std::collections::BTreeSet<PathBuf> = keep
        .iter()
        .filter_map(|p| Path::new(p).canonicalize().ok())
        .collect();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.canonicalize().is_ok_and(|c| !kept.contains(&c)) {
            let _ = fs::remove_file(path);
        }
    }
}

fn cache(paths: &Paths, bytes: Vec<u8>) -> Option<(PathBuf, Vec<u8>)> {
    if !is_image(&bytes) || bytes.len() as u64 > MAX_COVER {
        return None;
    }
    let extension = if bytes.starts_with(b"\x89PNG") {
        "png"
    } else if is_webp(&bytes) {
        "webp"
    } else {
        "jpg"
    };
    let name = format!("{:x}", Sha256::digest(&bytes));
    let path = paths.covers().join(format!("{}.{extension}", &name[..32]));
    if !path.is_file() {
        fs::create_dir_all(paths.covers()).ok()?;
        if store::write_atomic(&path, &bytes).is_err() && !path.is_file() {
            return None;
        }
    }
    Some((path, bytes))
}

fn is_webp(bytes: &[u8]) -> bool {
    bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP")
}

fn is_image(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8]) || bytes.starts_with(b"\x89PNG") || is_webp(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_cover_rejects_paths_outside_covers_dir_and_corrupt_history_is_backed_up() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths { root: dir.path().to_path_buf() };
        fs::create_dir_all(paths.covers()).unwrap();
        let inside = paths.covers().join("valid.png");
        fs::write(&inside, b"\x89PNG\r\n\x1a\nrest").unwrap();
        let outside = dir.path().join("outside.png");
        fs::write(&outside, b"\x89PNG\r\n\x1a\nrest").unwrap();

        assert!(read_cover(&paths, &inside.to_string_lossy()).is_some());
        assert!(read_cover(&paths, &outside.to_string_lossy()).is_none());

        fs::write(paths.history(), "{not json").unwrap();
        assert!(load(&paths).is_empty());
        assert!(paths.root.join("history.json.corrupt").is_file());
    }

    #[test]
    fn inspect_files_discovers_sibling_sidecars_and_missing_audio() {
        let dir = tempfile::tempdir().unwrap();
        let track = dir.path().join("Artist - Song.flac");
        let lrc = dir.path().join("Artist - Song.lrc");
        let unrelated = dir.path().join("Artist - Song.pt2.lrc");
        let meta = dir.path().join("Artist - Song.musicbrainz.json");
        let missing = dir.path().join("Missing.flac");
        fs::write(&track, b"fLaC").unwrap();
        fs::write(&lrc, b"[00:01.00] hi").unwrap();
        fs::write(&unrelated, b"[00:02.00] other").unwrap();
        fs::write(&meta, b"{}").unwrap();

        let track_s = track.to_string_lossy().into_owned();
        let missing_s = missing.to_string_lossy().into_owned();
        let info = inspect_files(&[track_s.clone(), missing_s.clone()]);
        assert!(info[&track_s].exists);
        let labels: Vec<&str> = info[&track_s].sidecars.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(labels, ["LRC", "Meta"]);
        assert_eq!(info[&track_s].sidecars[0].1, lrc.to_string_lossy());
        assert!(!info[&missing_s].exists);
        assert!(info[&missing_s].sidecars.is_empty());

        let paths = Paths { root: dir.path().to_path_buf() };
        let webp = b"RIFF\x10\x00\x00\x00WEBPVP8 ".to_vec();
        let (cached, _) = cache(&paths, webp.clone()).expect("webp cover caches");
        assert_eq!(cached.extension().and_then(|e| e.to_str()), Some("webp"));
        assert_eq!(read_cover(&paths, &cached.to_string_lossy()), Some(webp));
    }

    #[test]
    fn export_entries_writes_escaped_csv_and_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let entries = vec![Entry {
            title: "Song, \"Live\"".into(),
            artist: "HUNTR/X".into(),
            album: "OST".into(),
            service: "Tidal".into(),
            extension: "Rhythmax".into(),
            quality: "FLAC 24-bit".into(),
            url: "https://tidal.com/track/1".into(),
            files: vec!["C:/Music/Song.flac".into()],
            cover: None,
            date: 1_700_000_000,
        }];
        let csv_path = dir.path().join("lib.csv");
        assert_eq!(export_entries(&csv_path, &entries).unwrap(), 1);
        let csv = fs::read_to_string(&csv_path).unwrap();
        assert!(csv.starts_with("title,artist,album,service,extension,quality,url,file,date\n"));
        assert!(csv.contains("\"Song, \"\"Live\"\"\",HUNTR/X,OST,Tidal"));

        let json_path = dir.path().join("lib.json");
        assert_eq!(export_entries(&json_path, &entries).unwrap(), 1);
        let parsed: Vec<Entry> = serde_json::from_str(&fs::read_to_string(&json_path).unwrap()).unwrap();
        assert_eq!(parsed, entries);
    }
}
