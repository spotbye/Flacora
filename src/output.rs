//! Optional naming and folder organization for completed downloads.

use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use tempfile::{NamedTempFile, TempDir};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Layout {
    pub filename: Filename,
    pub folders: Folders,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Filename {
    #[default]
    ExtensionDefault,
    ArtistTitle,
    TitleArtist,
    Title,
}

impl Filename {
    pub const ALL: [Self; 4] = [
        Self::ExtensionDefault,
        Self::ArtistTitle,
        Self::TitleArtist,
        Self::Title,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::ExtensionDefault => "Keep Original",
            Self::ArtistTitle => "Artist - Title",
            Self::TitleArtist => "Title - Artist",
            Self::Title => "Title Only",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Folders {
    #[default]
    Single,
    Artist,
    ArtistAlbum,
}

impl Folders {
    pub const ALL: [Self; 3] = [Self::Single, Self::Artist, Self::ArtistAlbum];

    pub fn label(self) -> &'static str {
        match self {
            Self::Single => "Single Folder",
            Self::Artist => "By Artist",
            Self::ArtistAlbum => "Artist / Album",
        }
    }
}

#[derive(Default)]
pub struct TrackNames {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
}

impl Layout {
    pub fn staging(self, output: &Path) -> io::Result<Option<TempDir>> {
        if self == Self::default() {
            return Ok(None);
        }
        let output = std::path::absolute(output)?;
        fs::create_dir_all(&output)?;
        tempfile::Builder::new()
            .prefix(".flacora-")
            .tempdir_in(output)
            .map(Some)
    }

    pub fn place(
        self,
        staging: TempDir,
        output: &Path,
        files: &[String],
        names: TrackNames,
    ) -> Result<Vec<String>, String> {
        match self.publish(staging.path(), output, files, names) {
            Ok(paths) => Ok(paths
                .into_iter()
                .map(|path| crate::runner::strip_win_verbatim(&path).to_string_lossy().into_owned())
                .collect()),
            Err(error) => Err(format!(
                "Could not organize the download: {error}. Files kept in {}",
                staging.keep().display()
            )),
        }
    }

    fn publish(
        self,
        staging: &Path,
        output: &Path,
        files: &[String],
        names: TrackNames,
    ) -> io::Result<Vec<PathBuf>> {
        let staging = staging.canonicalize()?;
        let mut sources = Vec::new();
        for file in files {
            let path = Path::new(file);
            let source = if path.is_absolute() {
                path.canonicalize()?
            } else {
                staging.join(path).canonicalize()?
            };
            if !source.starts_with(&staging) || !source.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "The extension reported a file outside its download folder",
                ));
            }
            if !crate::runner::is_safe_open_file(&source) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "The extension reported an unsupported file type",
                ));
            }
            if !sources.contains(&source) {
                sources.push(source);
            }
        }
        let main = sources
            .iter()
            .find(|path| {
                path.extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("flac"))
            })
            .or_else(|| {
                sources
                    .iter()
                    .find(|path| crate::library::is_audio_file(&path.to_string_lossy()))
            })
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "The extension did not report a saved audio file",
                )
            })?;
        let original = main
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Track");
        let is_flac = main
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("flac"));
        let tags = is_flac.then(|| metaflac::Tag::read_from_path(main).ok()).flatten();
        let tag = |key| {
            tags.as_ref()?
                .get_vorbis(key)?
                .find(|value| !value.trim().is_empty())
        };
        let artist = tag("ARTIST")
            .or(names.artist.as_deref())
            .unwrap_or("Unknown Artist");
        let album_artist = tag("ALBUMARTIST").unwrap_or(artist);
        let title = tag("TITLE").or(names.title.as_deref()).unwrap_or(original);
        let album = tag("ALBUM")
            .or(names.album.as_deref())
            .unwrap_or("Unknown Album");
        let stem = match self.filename {
            Filename::ExtensionDefault => original.to_owned(),
            Filename::ArtistTitle => format!("{artist} - {title}"),
            Filename::TitleArtist => format!("{title} - {artist}"),
            Filename::Title => title.to_owned(),
        };

        let (folder_cap, file_cap) = match self.folders {
            Folders::Single => (160, 160),
            Folders::Artist => (110, 140),
            Folders::ArtistAlbum => (70, 100),
        };
        let mut directory = std::path::absolute(output)?;
        let base = directory.canonicalize()?;
        if self.folders != Folders::Single {
            let folder_artist = if self.folders == Folders::ArtistAlbum {
                album_artist
            } else {
                artist
            };
            directory = subfolder(
                &directory,
                &component_capped(folder_artist, "Unknown Artist", folder_cap),
                &base,
            )?;
        }
        if self.folders == Folders::ArtistAlbum {
            directory = subfolder(
                &directory,
                &component_capped(album, "Unknown Album", folder_cap),
                &base,
            )?;
        }

        let main_stem = main.file_stem().and_then(|s| s.to_str());
        let same_track = |path: &Path| {
            if path.parent() != main.parent() {
                return false;
            }
            if path.file_stem() == main.file_stem() {
                return true;
            }
            let (Some(ms), Some(name)) = (main_stem, path.file_name().and_then(|n| n.to_str())) else {
                return false;
            };
            sidecar_suffix_for_stem(name, ms).is_some()
        };
        let bundle = sources
            .iter()
            .map(PathBuf::as_path)
            .filter(|path| same_track(path))
            .collect::<Vec<_>>();
        let clean_stem = component_capped(&stem, "Track", file_cap);
        for attempt in 0..8 {
            let bundle_stem = available_stem(&directory, &clean_stem, &bundle, main_stem, true)?;
            let mut published = Vec::new();
            let result: io::Result<()> = (|| {
                for source in &sources {
                    let is_bundle = same_track(source);
                    let path = if is_bundle {
                        target_path(&directory, &bundle_stem, source, main_stem)
                    } else {
                        let orig = source.file_stem().unwrap_or_default().to_string_lossy();
                        let other_stem = available_stem(
                            &directory,
                            &component_capped(&orig, "File", file_cap),
                            &[source],
                            None,
                            false,
                        )?;
                        target_path(&directory, &other_stem, source, None)
                    };
                    copy_file(source, &path)?;
                    published.push(path);
                }
                Ok(())
            })();
            match result {
                Ok(()) => return Ok(published),
                Err(error) => {
                    for path in &published {
                        let _ = fs::remove_file(path);
                    }
                    if error.kind() == io::ErrorKind::AlreadyExists && attempt < 7 {
                        continue;
                    }
                    return Err(error);
                }
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "No free filename for the download",
        ))
    }
}

fn subfolder(parent: &Path, name: &str, base: &Path) -> io::Result<PathBuf> {
    let folder = parent.join(name);
    match fs::create_dir(&folder) {
        Ok(()) => Ok(folder),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            if folder.is_dir() && folder.canonicalize()?.starts_with(base) {
                Ok(folder)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("Unsafe output folder {}", folder.display()),
                ))
            }
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
fn component(input: &str, fallback: &str) -> String {
    component_capped(input, fallback, 160)
}

fn component_capped(input: &str, fallback: &str, max_bytes: usize) -> String {
    let mut name: String = input
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .take(max_bytes)
        .collect();
    while name.len() > max_bytes {
        name.pop();
    }
    let name = name.trim_matches(|c: char| c.is_whitespace() || c == '.');
    if name.is_empty() {
        return fallback.to_owned();
    }
    let first = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end()
        .to_ascii_uppercase();
    let device = matches!(
        first.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|prefix| {
        first.strip_prefix(prefix).is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    });
    if device {
        format!("_{name}")
    } else {
        name.to_owned()
    }
}

/// Returns a multi-dot sidecar suffix (like `musicbrainz.json`) when `name` matches `<stem>.<tag>.json`.
fn sidecar_suffix_for_stem<'a>(name: &'a str, stem: &str) -> Option<&'a str> {
    let rest = name.strip_prefix(stem)?.strip_prefix('.')?;
    let (mid, ext) = rest.split_once('.')?;
    if !mid.is_empty() && !mid.contains('.') && ext.eq_ignore_ascii_case("json") {
        Some(rest)
    } else {
        None
    }
}

fn target_path(directory: &Path, stem: &str, source: &Path, main_stem: Option<&str>) -> PathBuf {
    if let (Some(ms), Some(name)) = (main_stem, source.file_name().and_then(|n| n.to_str()))
        && let Some(multi_ext) = sidecar_suffix_for_stem(name, ms)
    {
        return directory.join(format!("{stem}.{multi_ext}"));
    }
    let extension = source
        .extension()
        .map(|ext| format!(".{}", component_capped(&ext.to_string_lossy(), "file", 32)))
        .unwrap_or_default();
    directory.join(format!("{stem}{extension}"))
}

fn available_stem(
    directory: &Path,
    stem: &str,
    files: &[&Path],
    main_stem: Option<&str>,
    reserve_lookup_files: bool,
) -> io::Result<String> {
    let unused = |path: PathBuf| {
        path.symlink_metadata()
            .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
    };
    (1..=1000)
        .map(|number| {
            if number == 1 {
                stem.to_owned()
            } else {
                format!("{stem} ({number})")
            }
        })
        .find(|candidate| {
            files
                .iter()
                .all(|file| unused(target_path(directory, candidate, file, main_stem)))
                && (!reserve_lookup_files
                    || [
                        "lrc",
                        "ttml",
                        "txt",
                        "musicbrainz.json",
                        "spotify.json",
                        "tidal.json",
                        "deezer.json",
                        "qobuz.json",
                        "amazon.json",
                    ]
                    .iter()
                    .all(|suffix| unused(directory.join(format!("{candidate}.{suffix}")))))
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "No free filename for the download",
            )
        })
}

fn copy_file(source: &Path, target: &Path) -> io::Result<()> {
    match fs::hard_link(source, target) {
        Ok(()) => return Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return Err(error),
        Err(_) => {}
    }
    let mut input = fs::File::open(source)?;
    let mut temp = NamedTempFile::new_in(target.parent().expect("target is in the output folder"))?;
    io::copy(&mut input, &mut temp)?;
    temp.persist_noclobber(target)
        .map(|_| ())
        .map_err(|error| error.error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn organizes_downloads_and_matching_sidecars_without_overwriting() {
        let root = tempfile::tempdir().unwrap();
        let layout = Layout {
            filename: Filename::Title,
            folders: Folders::ArtistAlbum,
        };
        let destination = root.path().join("HUNTR_X").join("Soundtrack");
        fs::create_dir_all(&destination).unwrap();
        fs::write(destination.join("Golden.flac"), b"original").unwrap();
        fs::write(destination.join("Golden (2).lrc"), b"existing lyrics").unwrap();
        let stage = layout.staging(root.path()).unwrap().unwrap();
        fs::write(stage.path().join("track.flac"), b"new FLAC").unwrap();
        fs::write(stage.path().join("track.lrc"), b"new lyrics").unwrap();
        let names = TrackNames {
            title: Some("Golden".into()),
            artist: Some("HUNTR/X".into()),
            album: Some("Soundtrack".into()),
        };
        let saved = layout
            .place(
                stage,
                root.path(),
                &["track.flac".into(), "track.lrc".into()],
                names,
            )
            .unwrap();
        assert_eq!(Path::new(&saved[0]), destination.join("Golden (3).flac"));
        assert_eq!(Path::new(&saved[1]), destination.join("Golden (3).lrc"));
        assert_eq!(fs::read(&saved[0]).unwrap(), b"new FLAC");
        assert_eq!(fs::read(&saved[1]).unwrap(), b"new lyrics");
        assert_eq!(
            fs::read(destination.join("Golden.flac")).unwrap(),
            b"original"
        );
        assert_eq!(
            fs::read(destination.join("Golden (2).lrc")).unwrap(),
            b"existing lyrics"
        );
    }

    #[test]
    fn rejects_files_outside_staging_and_retains_the_download() {
        let root = tempfile::tempdir().unwrap();
        let layout = Layout {
            filename: Filename::Title,
            ..Layout::default()
        };
        let stage = layout.staging(root.path()).unwrap().unwrap();
        let retained = stage.path().join("actual.flac");
        fs::write(&retained, b"keep").unwrap();
        let unrelated = root.path().join("existing.flac");
        fs::write(&unrelated, b"existing").unwrap();
        let error = layout
            .place(
                stage,
                root.path(),
                &[unrelated.to_string_lossy().into_owned()],
                TrackNames::default(),
            )
            .unwrap_err();
        assert!(error.contains("outside its download folder"));
        assert_eq!(fs::read(retained).unwrap(), b"keep");
        assert_eq!(fs::read(unrelated).unwrap(), b"existing");
    }

    #[test]
    fn names_cannot_create_paths_or_windows_device_files() {
        assert_eq!(component("../A:B?", "Track"), "_A_B_");
        assert_eq!(component("  nul.backup. ", "Track"), "_nul.backup");
        assert_eq!(component("...  ", "Track"), "Track");
        assert!(component(&"é".repeat(200), "Track").len() <= 160);
    }

    #[test]
    fn preserves_multi_dot_metadata_sidecars_and_rejects_executables() {
        let root = tempfile::tempdir().unwrap();
        let layout = Layout {
            filename: Filename::Title,
            folders: Folders::Single,
        };
        let stage = layout.staging(root.path()).unwrap().unwrap();
        fs::write(stage.path().join("track.flac"), b"fLaC").unwrap();
        fs::write(stage.path().join("track.musicbrainz.json"), b"{}").unwrap();
        let names = TrackNames {
            title: Some("Golden".into()),
            ..TrackNames::default()
        };
        let saved = layout
            .place(
                stage,
                root.path(),
                &["track.flac".into(), "track.musicbrainz.json".into()],
                names,
            )
            .unwrap();
        assert_eq!(Path::new(&saved[0]).file_name().unwrap(), "Golden.flac");
        assert_eq!(Path::new(&saved[1]).file_name().unwrap(), "Golden.musicbrainz.json");

        let stage2 = layout.staging(root.path()).unwrap().unwrap();
        fs::write(stage2.path().join("track.flac"), b"fLaC").unwrap();
        fs::write(stage2.path().join("evil.hta"), b"<script>").unwrap();
        let err = layout
            .place(
                stage2,
                root.path(),
                &["track.flac".into(), "evil.hta".into()],
                TrackNames::default(),
            )
            .unwrap_err();
        assert!(err.contains("unsupported file type"), "{err}");
    }
}
