//! Local data, installed extensions, settings, and background tools.
//! Everything here blocks, so it only runs on background threads.

use crate::extensions::{self, Manifest};
use crate::{protocol, runner::{self, JobEvent}};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::AeadInPlace};
use reqwest::{StatusCode, blocking::{Client, RequestBuilder, Response}, header};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Cursor, Read, Seek},
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, Mutex, mpsc::channel},
    thread,
    time::{Duration, Instant, SystemTime},
};

#[derive(Clone, Debug)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    /// `FLACORA_HOME`, else the platform's per-user data folder.
    pub fn detect() -> Paths {
        let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
        let root = var("FLACORA_HOME").unwrap_or_else(|| {
            if cfg!(windows) {
                var("LOCALAPPDATA").map(|p| p.join("Flacora"))
            } else if cfg!(target_os = "macos") {
                var("HOME").map(|p| p.join("Library/Application Support/Flacora"))
            } else {
                var("XDG_DATA_HOME").or_else(|| var("HOME").map(|p| p.join(".local/share"))).map(|p| p.join("flacora"))
            }
            .unwrap_or_else(|| PathBuf::from("flacora-data"))
        });
        Paths { root }
    }

    pub fn extensions(&self) -> PathBuf {
        self.root.join("extensions")
    }

    pub fn extension(&self, id: &str) -> PathBuf {
        self.extensions().join(id)
    }

    /// Per-extension working data (sessions, browser profiles), kept across updates.
    pub fn data(&self, id: &str) -> PathBuf {
        self.root.join("data").join(id)
    }

    /// CloakBrowser install shared by every extension that needs it.
    pub fn browser(&self) -> PathBuf {
        self.root.join("cloakbrowser")
    }

    fn settings(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    pub fn history(&self) -> PathBuf {
        self.root.join("history.json")
    }

    /// Cached cover art, named by content hash.
    pub fn covers(&self) -> PathBuf {
        self.root.join("covers")
    }

    fn lock_file(&self) -> PathBuf {
        self.root.join(".flacora.lock")
    }
}

/// Runs a filesystem mutation under an advisory file lock so concurrent instances do not race.
pub fn with_lock<T>(paths: &Paths, work: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    fs::create_dir_all(&paths.root).map_err(|e| e.to_string())?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(paths.lock_file())
        .map_err(|e| e.to_string())?;
    lock.lock().map_err(|e| e.to_string())?;
    let result = work();
    let _ = lock.unlock();
    result
}

/// Removes leftover staging or backup folders from interrupted extension installs.
pub fn cleanup_stale_temps(paths: &Paths) {
    let Ok(entries) = fs::read_dir(paths.extensions()) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let text = name.to_string_lossy();
        if text.starts_with(".import-") || text.starts_with(".superseded-") || text.ends_with(".old") {
            let path = entry.path();
            if path.is_dir() {
                let _ = fs::remove_dir_all(&path);
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Installed {
    pub manifest: Manifest,
    pub dir: PathBuf,
}

impl Installed {
    pub fn executable(&self) -> Option<PathBuf> {
        let relative = self.manifest.executable.get(extensions::platform())?;
        extensions::is_safe_relative_path(relative).then(|| self.dir.join(relative))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::System, Theme::Light, Theme::Dark];

    pub fn key(self) -> &'static str {
        match self {
            Theme::System => "system",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Theme::System => "System",
            Theme::Light => "Light",
            Theme::Dark => "Dark",
        }
    }
}

pub use crate::astryx::ThemePreset;

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub output: PathBuf,
    pub layout: crate::output::Layout,
    pub auto_tag: bool,
    pub max_concurrent_jobs: usize,
    pub theme: Theme,
    pub theme_preset: ThemePreset,
    /// Extension id to use per service key, when several are installed.
    pub preferred: BTreeMap<String, String>,
    /// Last choice per "<extension id>/<option key>".
    pub options: BTreeMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            output: default_output(), layout: crate::output::Layout::default(), auto_tag: false,
            max_concurrent_jobs: 1, theme: Theme::System,
            theme_preset: ThemePreset::default(), preferred: BTreeMap::new(), options: BTreeMap::new(),
        }
    }
}

/// `~/Music/Flacora`.
pub fn default_output() -> PathBuf {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    home.unwrap_or_else(|| PathBuf::from(".")).join("Music").join("Flacora")
}

pub fn load_settings(paths: &Paths) -> Settings {
    let settings_path = paths.settings();
    let Ok(text) = fs::read_to_string(&settings_path) else {
        return Settings::default();
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        backup_corrupt(&settings_path);
        return Settings::default();
    };
    let map = |key: &str| -> BTreeMap<String, String> {
        json[key]
            .as_object()
            .map(|o| o.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned()))).collect())
            .unwrap_or_default()
    };
    Settings {
        output: json["output"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(default_output),
        layout: serde_json::from_value(json["layout"].clone()).unwrap_or_default(),
        auto_tag: json["auto_tag"].as_bool().unwrap_or_default(),
        max_concurrent_jobs: json["max_concurrent_jobs"]
            .as_u64()
            .map(|n| (n as usize).clamp(1, 4))
            .unwrap_or(1),
        theme: Theme::ALL.into_iter().find(|t| json["theme"].as_str() == Some(t.key())).unwrap_or_default(),
        theme_preset: ThemePreset::ALL.into_iter().find(|t| json["theme_preset"].as_str() == Some(t.key())).unwrap_or_default(),
        preferred: map("preferred"),
        options: map("options"),
    }
}

pub fn save_settings(paths: &Paths, settings: &Settings) -> Result<(), String> {
    with_lock(paths, || {
        fs::create_dir_all(&paths.root).map_err(|e| e.to_string())?;
        let json = serde_json::json!({
            "output": settings.output.to_string_lossy(),
            "layout": settings.layout,
            "auto_tag": settings.auto_tag,
            "max_concurrent_jobs": settings.max_concurrent_jobs.clamp(1, 4),
            "theme": settings.theme.key(),
            "theme_preset": settings.theme_preset.key(),
            "preferred": settings.preferred,
            "options": settings.options,
        });
        let text = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
        write_atomic(&paths.settings(), text.as_bytes())
    })
}

/// Preserves a corrupt JSON file beside its original path before defaults overwrite it.
pub fn backup_corrupt(path: &Path) {
    let mut name = path.as_os_str().to_os_string();
    name.push(".corrupt");
    let backup = PathBuf::from(&name);
    if !backup.exists() {
        let _ = fs::copy(path, backup);
        return;
    }
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    name.push(format!("-{stamp}"));
    let _ = fs::copy(path, PathBuf::from(name));
}

/// Helper programs the extensions rely on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tools {
    /// FFmpeg version when both helper programs are available.
    pub ffmpeg: Option<String>,
    pub browser: Option<String>,
}

pub const CAN_DOWNLOAD_FFMPEG: bool = cfg!(any(
    all(windows, target_arch = "x86_64"),
    all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64")),
    all(target_os = "macos", any(target_arch = "x86_64", target_arch = "aarch64"))
));

const CLOAKBROWSER_EXECUTABLE: &str = if cfg!(windows) {
    "chrome.exe"
} else if cfg!(target_os = "macos") {
    "Chromium.app/Contents/MacOS/Chromium"
} else {
    "chrome"
};

pub fn tools(paths: &Paths) -> Tools {
    let ffmpeg = find_ffmpeg(&paths.root)
        .and_then(|program| {
            let mut command = std::process::Command::new(program);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x0800_0000);
            }
            command.arg("-version").stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().ok()
        })
        .filter(|o| o.status.success())
        .and_then(|o| ffmpeg_version(&String::from_utf8_lossy(&o.stdout)).map(str::to_owned));
    let browser = installed_browser(&paths.browser());
    Tools { ffmpeg, browser }
}

fn find_ffmpeg(root: &Path) -> Option<PathBuf> {
    let names = if cfg!(windows) { ["ffmpeg.exe", "ffprobe.exe"] } else { ["ffmpeg", "ffprobe"] };
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut folders: Vec<PathBuf> = std::env::split_paths(&path)
        .filter(|dir| !dir.as_os_str().is_empty())
        .collect();
    #[cfg(target_os = "macos")]
    for extra in ["/opt/homebrew/bin", "/usr/local/bin"] {
        let dir = PathBuf::from(extra);
        if !folders.contains(&dir) {
            folders.push(dir);
        }
    }
    let cache = root.join("ffmpeg");
    if let Ok(entries) = fs::read_dir(&cache) {
        let mut cached: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        cached.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
        folders.extend(cached);
    }
    if let Some(dir) = folders.iter().find(|dir| dir.join(names[0]).is_file() && dir.join(names[1]).is_file()) {
        return std::path::absolute(dir.join(names[0])).ok();
    }
    let find = |name: &str| {
        folders
            .iter()
            .map(|folder| folder.join(name))
            .find(|file| file.is_file())
            .and_then(|file| std::path::absolute(file).ok())
    };
    find(names[1])?;
    find(names[0])
}

fn ffmpeg_version(output: &str) -> Option<&str> {
    let version = output.lines().next()?.strip_prefix("ffmpeg version ")?.split_whitespace().next()?;
    let release = version.split(['-', '+']).next()?;
    let number = release.strip_prefix('n').unwrap_or(release);
    let numeric_version = number.contains('.') && number.split('.').all(|part| {
        !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit())
    });
    Some(if numeric_version { number } else { version })
}

#[derive(Debug, PartialEq, Eq)]
pub enum ExtensionStatus {
    Online,
    Degraded,
    Offline(String),
    Unknown(String),
    Services(Vec<String>),
}

const MUSICBRAINZ_INTERVAL: Duration = Duration::from_millis(1100);
static MUSICBRAINZ_NEXT: Mutex<Option<Instant>> = Mutex::new(None);
static STATUS_HTTP: LazyLock<Result<Client, reqwest::Error>> = LazyLock::new(|| {
    Client::builder().https_only(true)
        .user_agent(concat!("Flacora/", env!("CARGO_PKG_VERSION"), " (https://github.com/spotbye/Flacora)"))
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()
});

pub fn check_status(paths: &Paths, installed: &Installed) -> ExtensionStatus {
    if installed.manifest.status.is_empty() {
        return check_endpoint(&installed.manifest);
    }
    match check_services(paths, installed) {
        Ok(services) => ExtensionStatus::Services(services),
        Err(error) => ExtensionStatus::Unknown(error),
    }
}

fn check_endpoint(manifest: &Manifest) -> ExtensionStatus {
    let Some(url) = extensions::status_url(manifest) else {
        return ExtensionStatus::Unknown("This extension does not declare a status endpoint.".into());
    };
    let built_in = manifest.status_url.is_none().then_some(manifest.id.as_str());
    let Ok(http) = STATUS_HTTP.as_ref() else {
        return ExtensionStatus::Unknown("Could not start the status check.".into());
    };
    let request = http.get(url);
    let result = if built_in == Some("musicbrainz") {
        musicbrainz_request(request.header(header::ACCEPT, "application/json"))
    } else {
        request.send()
    };
    let response = match result {
        Ok(response) => response,
        Err(error) if error.is_timeout() => return ExtensionStatus::Offline("The status endpoint timed out.".into()),
        Err(error) if error.is_connect() => return ExtensionStatus::Offline("Could not reach the status endpoint.".into()),
        Err(_) => return ExtensionStatus::Unknown("Could not complete the status request.".into()),
    };
    let status = response.status();
    if built_in == Some("musicbrainz") && musicbrainz_busy(status) {
        return ExtensionStatus::Unknown(format!("MusicBrainz is busy or rate limiting requests (HTTP {}).", status.as_u16()));
    }
    if !status.is_success() {
        return http_status(status);
    }
    match built_in {
        Some(id @ ("betterlyrics" | "lyricsplus" | "musicbrainz")) => {
            serde_json::from_reader(response.take(64 * 1024)).ok()
                .and_then(|body| reported_health(id, &body))
                .unwrap_or_else(|| ExtensionStatus::Unknown("The endpoint did not return the expected result.".into()))
        }
        _ => ExtensionStatus::Online,
    }
}

fn reported_health(id: &str, body: &serde_json::Value) -> Option<ExtensionStatus> {
    if id == "musicbrainz" {
        return body["recordings"].as_array()?.iter().any(|recording| {
            recording["id"].as_str().is_some_and(|id| !id.is_empty())
        }).then_some(ExtensionStatus::Online);
    }
    match body["status"].as_str()? {
        "ok" => Some(ExtensionStatus::Online),
        "degraded" => Some(ExtensionStatus::Degraded),
        "unhealthy" => Some(ExtensionStatus::Offline("The endpoint reports a service outage.".into())),
        _ => None,
    }
}

fn musicbrainz_busy(status: StatusCode) -> bool {
    matches!(status, StatusCode::TOO_MANY_REQUESTS | StatusCode::SERVICE_UNAVAILABLE)
}

fn musicbrainz_retry_delay(retry_after: Option<&str>, now: SystemTime) -> Option<Duration> {
    let delay = retry_after.and_then(|value| {
        value.parse::<u64>().ok().map(Duration::from_secs).or_else(|| {
            let at = httpdate::parse_http_date(value).ok()?;
            Some(at.duration_since(now).unwrap_or_default())
        })
    }).unwrap_or(Duration::from_secs(3));
    (delay <= Duration::from_secs(8)).then(|| delay.max(MUSICBRAINZ_INTERVAL))
}

fn musicbrainz_request(request: RequestBuilder) -> Result<Response, reqwest::Error> {
    let retry = request.try_clone();
    let response = send_musicbrainz(request)?;
    if musicbrainz_busy(response.status())
        && let Some(retry) = retry
        && let Some(delay) = musicbrainz_retry_delay(response.headers().get(header::RETRY_AFTER).and_then(|value| value.to_str().ok()), SystemTime::now())
    {
        drop(response);
        thread::sleep(delay);
        return send_musicbrainz(retry);
    }
    Ok(response)
}

fn send_musicbrainz(request: RequestBuilder) -> Result<Response, reqwest::Error> {
    let mut next = MUSICBRAINZ_NEXT.lock().unwrap_or_else(|error| error.into_inner());
    let now = Instant::now();
    let start = next.unwrap_or(now).max(now);
    *next = Some(start + MUSICBRAINZ_INTERVAL);
    drop(next);
    thread::sleep(start - now);
    request.send()
}

fn http_status(status: StatusCode) -> ExtensionStatus {
    if status.is_success() {
        ExtensionStatus::Online
    } else {
        let reason = format!("The status endpoint returned HTTP {}.", status.as_u16());
        if status.is_server_error() { ExtensionStatus::Offline(reason) } else { ExtensionStatus::Unknown(reason) }
    }
}

fn check_services(paths: &Paths, installed: &Installed) -> Result<Vec<String>, String> {
    let manifest = &installed.manifest;
    let program = installed.executable().ok_or("No build for this computer")?;
    let (data, browser) = (paths.data(&manifest.id), paths.browser());
    let (data_text, browser_text) = (data.to_string_lossy().into_owned(), browser.to_string_lossy().into_owned());
    let fill = extensions::Fill { url: "", file: "", output: "", data: &data_text, browser: &browser_text };
    let spec = runner::Spec {
        program,
        args: extensions::status_args(manifest, &fill),
        env: extensions::command_env(manifest, &fill),
        cwd: data,
    };
    let (send, events) = channel();
    let handle = runner::spawn(spec, Arc::new(move |event| { let _ = send.send(event); }))?;
    // A browser-backed check can include the first CloakBrowser download.
    let deadline = Instant::now() + Duration::from_secs(if manifest.browser { 300 } else { 65 });
    let mut services = None;
    let mut reason = None;
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        let event = match events.recv_timeout(remaining) {
            Ok(event) => event,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err(reason.unwrap_or_else(|| format!("{} stopped before reporting its services.", manifest.name)));
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
        };
        match event {
            JobEvent::Line(line) => {
                if let Some(protocol::Event::Services(working)) = protocol::parse(&line) {
                    services.get_or_insert(working);
                } else if let Some(error) = protocol::error_line(&line) {
                    reason = Some(error);
                }
            }
            JobEvent::Exit(Ok(0)) if services.is_some() => return Ok(services.unwrap()),
            JobEvent::Exit(exit) => return Err(reason.or_else(|| exit.err())
                .unwrap_or_else(|| format!("{} did not report its services.", manifest.name))),
        }
    }
    handle.cancel();
    Err("The extension's status command timed out.".into())
}

fn version_sort_key(version: &str) -> Vec<u64> {
    version.split('.').map(|part| part.parse::<u64>().unwrap_or(0)).collect()
}

fn installed_browser(root: &Path) -> Option<String> {
    let mut versions: Vec<String> = fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name();
            let version = name.to_str()?.strip_prefix("chromium-")?;
            let dir = entry.path();
            if version.is_empty() || version.ends_with(".tmp") || !dir.join(CLOAKBROWSER_EXECUTABLE).is_file() {
                return None;
            }
            Some(version.to_owned())
        })
        .collect();
    versions.sort_by(|a, b| version_sort_key(b).cmp(&version_sort_key(a)).then_with(|| b.cmp(a)));
    versions.into_iter().next()
}

/// Every extension folder with a valid `extension.json`, sorted by name.
pub fn load_installed(paths: &Paths) -> Vec<Installed> {
    let Ok(entries) = fs::read_dir(paths.extensions()) else { return Vec::new() };
    let mut installed: Vec<Installed> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir() && !entry.file_name().to_string_lossy().contains('.'))
        .filter_map(|entry| {
            let manifest = extensions::parse_manifest(&fs::read_to_string(entry.path().join("extension.json")).ok()?).ok()?;
            (manifest.id == entry.file_name().to_string_lossy()).then(|| Installed { manifest, dir: entry.path() })
        })
        .collect();
    installed.sort_by_key(|i| i.manifest.name.to_lowercase());
    installed
}

/// File extension of a package holding `extension.json` and the executable.
pub const PACKAGE: &str = "fext";
pub const PACKAGE_EXTENSIONS: [&str; 3] = [PACKAGE, "flacora", "zip"];

/// Installs a dropped package, replacing an older install if it is idle.
pub fn install_file(paths: &Paths, package: &Path, can_replace: impl Fn(&str) -> bool) -> Result<Installed, String> {
    with_lock(paths, || install_file_locked(paths, package, can_replace))
}

fn install_file_locked(paths: &Paths, package: &Path, can_replace: impl Fn(&str) -> bool) -> Result<Installed, String> {
    if !package.is_file() {
        return Err(format!("{} is not a file", package.display()));
    }
    fs::create_dir_all(paths.extensions()).map_err(|e| e.to_string())?;
    // A dot in the folder name keeps load_installed from reading it as an extension.
    let staging = tempfile::Builder::new()
        .prefix(".import-")
        .tempdir_in(paths.extensions())
        .map_err(|e| e.to_string())?;
    unpack(package, staging.path())?;
    let manifest_path = staging.path().join("extension.json");
    if !fs::symlink_metadata(&manifest_path).is_ok_and(|m| m.file_type().is_file()) {
        return Err("This package has no extension.json".to_string());
    }
    let manifest = extensions::parse_manifest(
        &fs::read_to_string(&manifest_path).map_err(|_| "This package has no extension.json".to_string())?,
    )?;
    if let Some(message) = manifest.tag_writer_update() {
        return Err(message);
    }
    if let Some(new) = extensions::web_replacement(&manifest.id)
        && paths.extension(new).join("extension.json").is_file()
    {
        return Err(format!("Use the installed {new} extension."));
    }
    let alias = extensions::legacy_web_id(&manifest.id);
    if !can_replace(&manifest.id) || alias.is_some_and(|legacy| !can_replace(legacy)) {
        return Err(format!("{} is in use. Try dropping the package again when it is idle.", manifest.name));
    }
    let installed = Installed { dir: paths.extension(&manifest.id), manifest };
    let relative = installed.manifest.executable.get(extensions::platform()).ok_or(format!(
        "{} has no build for {}",
        installed.manifest.name,
        extensions::platform()
    ))?;
    let staging_exe = staging.path().join(relative);
    let staging_root = staging.path().canonicalize().map_err(|e| e.to_string())?;
    let valid_exe = fs::symlink_metadata(&staging_exe).is_ok_and(|m| m.file_type().is_file())
        && staging_exe.canonicalize().is_ok_and(|p| p.starts_with(&staging_root));
    if !valid_exe {
        return Err(format!("The package is missing its executable ({relative})"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(&staging_exe) {
            let mode = meta.permissions().mode() | 0o700;
            let _ = fs::set_permissions(&staging_exe, fs::Permissions::from_mode(mode));
        }
    }
    let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let archived = if let Some(legacy) = alias {
        let old = paths.extension(legacy);
        if old.join("extension.json").is_file() {
            let backup = paths.extensions().join(format!(".superseded-{legacy}-{unique}"));
            fs::rename(&old, &backup).map_err(|error| format!("Could not replace {legacy}: {error}"))?;
            Some((old, backup))
        } else {
            None
        }
    } else {
        None
    };
    if let Err(error) = replace_dir(staging.path(), &installed.dir) {
        if let Some((old, backup)) = archived
            && let Err(restore) = fs::rename(backup, old)
        {
            return Err(format!("{error}. Could not restore the previous extension: {restore}"));
        }
        return Err(error);
    }
    let _ = staging.keep();
    if let Some((_, backup)) = archived {
        let _ = fs::remove_dir_all(&backup);
    }
    Ok(installed)
}

/// Removes the extension; its data folder stays for a reinstall.
pub fn remove(paths: &Paths, id: &str) -> Result<(), String> {
    extensions::valid_id(id)?;
    let dir = paths.extension(id);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("Could not remove {id}: {e}"))?;
    }
    Ok(())
}

static SHARED_HTTP: LazyLock<Result<reqwest::blocking::Client, String>> = LazyLock::new(|| {
    reqwest::blocking::Client::builder()
        .https_only(true)
        .user_agent(concat!("Flacora/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())
});

pub fn client() -> Result<reqwest::blocking::Client, String> {
    SHARED_HTTP.as_ref().cloned().map_err(Clone::clone)
}

const MAX_PACKAGE_ENTRIES: usize = 256;
const MAX_PACKAGE_BYTES: u64 = 256 * 1024 * 1024;
const FEXT_HEADER: &[u8; 5] = b"FEXT\x01";
// The embedded key is shared with the packager for offline installation.
const FEXT_KEY: &[u8; 32] = b"Flacora extension package key v1";
const FEXT_TAG_BYTES: u64 = 16;

/// Unpacks an encrypted package or a legacy ZIP, applying the same extraction limits.
fn unpack(archive: &Path, target: &Path) -> Result<(), String> {
    unpack_with_limits(archive, target, MAX_PACKAGE_ENTRIES, MAX_PACKAGE_BYTES)
}

fn unpack_with_limits(archive: &Path, target: &Path, max_entries: usize, max_bytes: u64) -> Result<(), String> {
    let mut file = File::open(archive).map_err(|e| e.to_string())?;
    let mut magic = [0; 4];
    file.read_exact(&mut magic).map_err(|e| match e.kind() {
        std::io::ErrorKind::UnexpectedEof => "This is not a Flacora extension package".to_owned(),
        _ => e.to_string(),
    })?;
    if &magic == b"FEXT" {
        return unpack_zip(Cursor::new(decrypt_package(file, max_bytes)?), target, max_entries, max_bytes);
    }
    file.rewind().map_err(|e| e.to_string())?;
    unpack_zip(file, target, max_entries, max_bytes)
}

fn decrypt_package(mut file: File, max_bytes: u64) -> Result<Vec<u8>, String> {
    let mut version = [0];
    let mut nonce = [0; 12];
    file.read_exact(&mut version).map_err(|e| format!("Could not read package version: {e}"))?;
    if version[0] != FEXT_HEADER[4] {
        return Err("This package uses an unsupported format version".to_owned());
    }
    file.read_exact(&mut nonce).map_err(|e| format!("Could not read package nonce: {e}"))?;
    let header_len = (FEXT_HEADER.len() + nonce.len()) as u64;
    let payload_len = file.metadata().map_err(|e| e.to_string())?.len().saturating_sub(header_len);
    if payload_len > max_bytes.saturating_add(FEXT_TAG_BYTES) {
        return Err("This package exceeds the maximum allowed size".to_owned());
    }
    let mut payload = vec![0; payload_len as usize];
    file.read_exact(&mut payload).map_err(|e| e.to_string())?;
    if file.read(&mut [0]).map_err(|e| e.to_string())? != 0 {
        return Err("The package changed while it was being read".to_owned());
    }
    Aes256Gcm::new(FEXT_KEY.into())
        .decrypt_in_place(&Nonce::from(nonce), FEXT_HEADER, &mut payload)
        .map_err(|_| "Could not decrypt this package. It may be damaged.".to_owned())?;
    Ok(payload)
}

fn unpack_zip(input: impl Read + Seek, target: &Path, max_entries: usize, max_bytes: u64) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(input).map_err(|_| "This is not a Flacora extension package".to_string())?;
    if zip.len() > max_entries {
        return Err("This package contains too many files".to_string());
    }
    fs::create_dir_all(target).map_err(|e| format!("Unpacking failed: {e}"))?;
    let mut total_bytes = 0u64;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|e| format!("Unpacking failed: {e}"))?;
        if entry.is_symlink() {
            return Err("This package contains an unsupported symbolic link".to_string());
        }
        let Some(relative) = entry.enclosed_name() else {
            return Err("This package contains an unsafe path".to_string());
        };
        if relative.as_os_str().is_empty() {
            if entry.is_dir() {
                continue;
            }
            return Err("This package contains an unsafe path".to_string());
        }
        if !relative.components().all(|c| match c {
            std::path::Component::Normal(part) => is_safe_zip_component(part),
            _ => false,
        }) {
            return Err("This package contains an unsafe path".to_string());
        }
        let destination = target.join(&relative);
        if entry.is_dir() {
            fs::create_dir_all(&destination).map_err(|e| format!("Unpacking failed: {e}"))?;
            continue;
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Unpacking failed: {e}"))?;
        }
        let remaining = max_bytes.saturating_sub(total_bytes);
        if entry.size() > remaining {
            return Err("This package exceeds the maximum allowed size".to_string());
        }
        let mut out = File::create_new(&destination).map_err(|e| format!("Unpacking failed: {e}"))?;
        let written = std::io::copy(&mut (&mut entry).take(remaining.saturating_add(1)), &mut out)
            .map_err(|e| format!("Unpacking failed: {e}"))?;
        if written > remaining {
            return Err("This package exceeds the maximum allowed size".to_string());
        }
        total_bytes += written;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode((mode & 0o777) | 0o600);
            fs::set_permissions(&destination, perms).map_err(|e| format!("Unpacking failed: {e}"))?;
        }
    }
    Ok(())
}

fn is_safe_zip_component(part: &std::ffi::OsStr) -> bool {
    let Some(text) = part.to_str() else { return false };
    if text.is_empty() || text.ends_with(['.', ' ']) || text.contains([':', '\0']) || text.chars().any(char::is_control) {
        return false;
    }
    let stem = text.split('.').next().unwrap_or_default().trim_end().to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix)
                .is_some_and(|suffix| matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
        });
    !reserved
}

/// Swaps `staging` in for `dir`, keeping the old copy until the new one is in place.
fn replace_dir(staging: &Path, dir: &Path) -> Result<(), String> {
    let old = dir.with_extension("old");
    if old.exists() {
        fs::remove_dir_all(&old).map_err(|e| e.to_string())?;
    }
    if dir.exists() {
        fs::rename(dir, &old).map_err(|e| format!("The extension is in use: {e}"))?;
    }
    if let Err(e) = fs::rename(staging, dir) {
        let _ = fs::rename(&old, dir);
        return Err(e.to_string());
    }
    let _ = fs::remove_dir_all(&old);
    Ok(())
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temp.write_all(bytes).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map(|_| ()).map_err(|e| e.error.to_string())
}
