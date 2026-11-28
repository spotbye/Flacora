//! Extension model: the manifest inside each extension zip (`extension.json`),
//! URL matching, and the command line an extension runs with.
//!
//! An extension is a command-line downloader. Flacora runs its executable with
//! the manifest's `args`, where `{url}` and `{output}` are filled in, plus the
//! arguments of every option the user picked (`{value}` is the choice). `env`
//! values may use `{data}` (the extension's own data folder) and `{browser}`
//! (the shared CloakBrowser folder, also exported as `CLOAKBROWSER_HOME` when
//! `browser` is true). Its stdout and stderr lines are shown as the job log and
//! exit code 0 means success.

use semver::Version;
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path};

pub const MIN_TAG_WRITER_VERSION: Version = Version::new(1, 0, 1);

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Choice {
    pub value: String,
    pub label: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ExtOption {
    pub key: String,
    pub label: String,
    pub choices: Vec<Choice>,
    /// Value of the default choice; the first choice when absent.
    #[serde(default)]
    pub default: Option<String>,
    /// Arguments added for this option; `{value}` is the picked choice.
    pub args: Vec<String>,
}

impl ExtOption {
    pub fn default_index(&self) -> usize {
        self.default
            .as_ref()
            .and_then(|value| self.choices.iter().position(|c| &c.value == value))
            .unwrap_or(0)
    }
}

/// What an extension does with a link.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Downloads it.
    #[default]
    Downloader,
    /// Finds it on other services (`links` event); a downloader then takes one of those.
    Resolver,
    Lyrics,
    Metadata,
}

impl Kind {
    pub fn is_lookup(self) -> bool {
        matches!(self, Self::Lyrics | Self::Metadata)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Downloader => "Downloader",
            Self::Resolver => "Link Resolver",
            Self::Lyrics => "Lyrics",
            Self::Metadata => "Metadata",
        }
    }
}

/// `extension.json` at the root of an extension zip.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub kind: Kind,
    /// The site it works through, e.g. `lucida.to`.
    #[serde(default)]
    pub description: String,
    /// Display names of the services it downloads from.
    #[serde(default)]
    pub services: Vec<String>,
    /// URL hosts it accepts; a host also matches its subdomains.
    #[serde(default)]
    pub hosts: Vec<String>,
    /// Executable path inside the extension folder, per platform key.
    pub executable: BTreeMap<String, String>,
    pub args: Vec<String>,
    /// Runs catalog commands independently of a local FLAC lookup.
    #[serde(default)]
    pub catalog: bool,
    /// Exposes catalog commands without offering a local FLAC lookup.
    #[serde(default)]
    pub catalog_only: bool,
    /// Writes tags into the supplied FLAC and reports the same file as saved.
    #[serde(default)]
    pub writes_tags: bool,
    /// Arguments that make it print a `services` event with what works right now.
    #[serde(default)]
    pub status: Vec<String>,
    /// Public HTTPS endpoint for extensions without a status command.
    #[serde(default)]
    pub status_url: Option<String>,
    #[serde(default)]
    pub options: Vec<ExtOption>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Needs the shared CloakBrowser install.
    #[serde(default)]
    pub browser: bool,
    /// Programs it expects on PATH, e.g. `ffmpeg`.
    #[serde(default)]
    pub requires: Vec<String>,
}

impl Manifest {
    pub fn supports_lookup(&self) -> bool {
        self.kind.is_lookup() && !self.catalog_only
    }

    pub fn tag_writer_update(&self) -> Option<String> {
        let needs_version = self.writes_tags || legacy_web_id(&self.id).is_some() || web_replacement(&self.id).is_some();
        (needs_version && !Version::parse(&self.version).is_ok_and(|version| version >= MIN_TAG_WRITER_VERSION))
            .then(|| format!("Update {} to v{MIN_TAG_WRITER_VERSION} or newer to tag FLAC files.", self.name))
    }

    pub fn can_write_tags(&self) -> bool {
        self.writes_tags && self.tag_writer_update().is_none()
    }
}

/// Keep the established fallback order for installed downloaders and resolvers.
/// Other extensions sort by name after these.
pub fn rank(id: &str) -> usize {
    ["flacdownloader", "odskyler", "cyan-dl", "lucida", "doubledouble", "monochrome", "odesli", "squigly"]
        .iter()
        .position(|&known| known == id)
        .unwrap_or(usize::MAX)
}

const WEB_RENAMES: [(&str, &str); 5] = [
    ("spotify-metadata", "spotify-web"),
    ("tidal-metadata", "tidal-web"),
    ("deezer-metadata", "deezer-web"),
    ("qobuz-metadata", "qobuz-web"),
    ("amazon-metadata", "amazon-web"),
];

pub fn legacy_web_id(new_id: &str) -> Option<&'static str> {
    WEB_RENAMES.iter().find(|(_, new)| *new == new_id).map(|(old, _)| *old)
}

pub fn web_replacement(old_id: &str) -> Option<&'static str> {
    WEB_RENAMES.iter().find(|(old, _)| *old == old_id).map(|(_, new)| *new)
}

pub fn parse_manifest(json: &str) -> Result<Manifest, String> {
    let mut manifest: Manifest = serde_json::from_str(json).map_err(|e| format!("Invalid extension.json: {e}"))?;
    valid_id(&manifest.id)?;
    if !manifest.kind.is_lookup() && manifest.hosts.is_empty() {
        return Err("extension.json lists no hosts".into());
    }
    if manifest.catalog && (manifest.kind != Kind::Metadata || manifest.hosts.is_empty()) {
        return Err("Catalog extensions must be metadata providers with hosts".into());
    }
    if manifest.catalog_only && (!manifest.catalog || manifest.writes_tags) {
        return Err("Catalog-only extensions must provide a catalog without a tag writer".into());
    }
    if manifest.writes_tags && manifest.kind != Kind::Metadata {
        return Err("Tag writers must be metadata providers".into());
    }
    if manifest.status_url.as_deref().is_some_and(|url| https_status_url(url).is_none()) {
        return Err("extension.json status_url must be an HTTPS URL without credentials or a fragment".into());
    }
    let input = if manifest.kind.is_lookup() { "{file}" } else { "{url}" };
    if !manifest.catalog_only && !manifest.args.iter().any(|arg| arg.contains(input)) {
        return Err(format!("extension.json args never pass {input}"));
    }
    for path in manifest.executable.values() {
        if !is_safe_relative_path(path) {
            return Err("extension.json executable must stay inside the extension".into());
        }
    }
    for key in manifest.env.keys() {
        if is_disallowed_env_key(key) {
            return Err(format!("extension.json env cannot override {key}"));
        }
    }
    let mut option_keys = std::collections::BTreeSet::new();
    for option in &manifest.options {
        if option.key.trim().is_empty() || !option_keys.insert(option.key.as_str()) {
            return Err("extension.json options must have unique non-empty keys".into());
        }
        if option.choices.is_empty() {
            return Err(format!("extension.json option {:?} has no choices", option.key));
        }
    }
    for host in &manifest.hosts {
        if !is_valid_manifest_host(host) {
            return Err(format!("extension.json contains an invalid host: {host:?}"));
        }
    }
    // Ignore retired routes in packages already installed by older builds.
    manifest.hosts.retain(|host| !crate::services::is_excluded_host(host));
    manifest.services.retain(|name| !crate::services::is_excluded_service(name));
    Ok(manifest)
}

fn is_valid_manifest_host(host: &str) -> bool {
    let trimmed = host.trim();
    !trimmed.is_empty()
        && trimmed == host
        && trimmed.contains('.')
        && !trimmed.starts_with('.')
        && !trimmed.ends_with('.')
        && !trimmed.contains("..")
        && trimmed.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
}

/// True when `value` is a non-empty relative path made solely of normal components.
pub fn is_safe_relative_path(value: &str) -> bool {
    if value.is_empty()
        || value.starts_with('/')
        || value.starts_with('\\')
        || value.contains(':')
        || value.contains('\0')
    {
        return false;
    }
    let mut components = Path::new(value).components();
    let Some(first) = components.next() else { return false };
    matches!(first, std::path::Component::Normal(_))
        && components.all(|c| matches!(c, std::path::Component::Normal(_)))
}

fn is_disallowed_env_key(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "FLACORA_EVENTS"
            | "FLACORA_HOME"
            | "FLACORA_MONOCHROME"
            | "PATH"
            | "PATHEXT"
            | "LD_PRELOAD"
            | "LD_LIBRARY_PATH"
            | "LD_AUDIT"
            | "LD_DYNAMIC_WEAK"
            | "DYLD_INSERT_LIBRARIES"
            | "DYLD_LIBRARY_PATH"
            | "DYLD_FRAMEWORK_PATH"
            | "DYLD_FORCE_FLAT_NAMESPACE"
            | "PYTHONSTARTUP"
            | "PYTHONPATH"
            | "NODE_OPTIONS"
            | "JAVA_TOOL_OPTIONS"
            | "BASH_ENV"
            | "ENV"
            | "SSL_CERT_FILE"
            | "SSL_CERT_DIR"
            | "HTTP_PROXY"
            | "HTTPS_PROXY"
            | "ALL_PROXY"
            | "NO_PROXY"
    )
}

/// Ids name folders, so they are lowercase ASCII words.
pub fn valid_id(id: &str) -> Result<(), String> {
    let ok = !id.is_empty()
        && id.len() <= 48
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
    if ok { Ok(()) } else { Err(format!("Invalid extension id: {id:?}")) }
}

/// Platform key used in manifests, e.g. `windows-x64`.
pub fn platform() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => "windows-x64",
        ("windows", "aarch64") => "windows-arm64",
        ("linux", "x86_64") => "linux-x64",
        ("linux", "aarch64") => "linux-arm64",
        ("macos", "aarch64") => "macos-arm64",
        ("macos", "x86_64") => "macos-x64",
        _ => "unsupported",
    }
}

/// The host of a pasted link, lowercased, when it is an http(s) URL without credentials.
pub fn url_host(input: &str) -> Option<String> {
    let url = reqwest::Url::parse(input.trim()).ok()?;
    if !matches!(url.scheme(), "https" | "http") || !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    let host = url.host_str()?.trim_end_matches('.');
    (!host.is_empty() && host.contains('.')).then(|| host.to_ascii_lowercase())
}

/// True when `host` is one of `hosts` or a subdomain of one.
pub fn host_matches<S: AsRef<str>>(hosts: &[S], host: &str) -> bool {
    hosts.iter().any(|h| {
        let h = h.as_ref();
        host.eq_ignore_ascii_case(h)
            || (host.len() > h.len()
                && host.as_bytes()[host.len() - h.len() - 1] == b'.'
                && host[host.len() - h.len()..].eq_ignore_ascii_case(h))
    })
}

/// Paths and values filled into an extension's command line.
pub struct Fill<'a> {
    pub url: &'a str,
    pub file: &'a str,
    pub output: &'a str,
    pub data: &'a str,
    pub browser: &'a str,
}

/// Arguments for a run: the manifest's args, then each option's args.
pub fn command_args(manifest: &Manifest, fill: &Fill, picked: &[usize]) -> Vec<String> {
    let mut args: Vec<String> = manifest.args.iter().map(|a| fill_in(a, fill, None)).collect();
    for (option, &index) in manifest.options.iter().zip(picked) {
        let value = option.choices.get(index).map(|c| c.value.as_str());
        args.extend(option.args.iter().map(|a| fill_in(a, fill, value)));
    }
    args
}

/// Arguments for a status check (`status`, with placeholders filled).
pub fn status_args(manifest: &Manifest, fill: &Fill) -> Vec<String> {
    manifest.status.iter().map(|a| fill_in(a, fill, None)).collect()
}

/// Built-in probes also cover previously installed packages without `status_url`.
pub fn status_url(manifest: &Manifest) -> Option<reqwest::Url> {
    if let Some(url) = &manifest.status_url {
        return https_status_url(url);
    }
    let id = web_replacement(&manifest.id).unwrap_or(&manifest.id);
    let endpoint = match id {
        "betterlyrics" => "https://api.betterlyrics.org/health",
        "cyan-dl" => "https://cyan-dl.pages.dev/version.json",
        "deezer-web" => "https://api.deezer.com/track/3412534581",
        "flacdownloader" => "https://flacdownloader.com/api/qobuz/track?url=https%3A%2F%2Fopen.qobuz.com%2Ftrack%2F342213756",
        "lrc-red" => "https://lrc.red/search.json?q=Never%20Gonna%20Give%20You%20Up%20Rick%20Astley",
        "lrclib" => "https://lrclib.net/api/get?track_name=Never%20Gonna%20Give%20You%20Up&artist_name=Rick%20Astley",
        "monochrome" => "https://tracks.monochrome.st/search/tracks?q=Golden&limit=1",
        "musicbrainz" => "https://musicbrainz.org/ws/2/isrc/USUM71703861?fmt=json",
        "odesli" => "https://song.link/s/1CPZ5BxNNd0n0nF4Orb9JS",
        "odskyler" => "https://hifi.odskyler.com/",
        "qobuz-web" => "https://open.qobuz.com/track/342213756",
        "spotify-web" => "https://open.spotify.com/embed/track/1CPZ5BxNNd0n0nF4Orb9JS",
        "squigly" => "https://squigly.link/api/resolve/f99fhc",
        "tidal-web" => "https://embed.tidal.com/tracks/441821360",
        _ => {
            let domain = manifest.description.trim().trim_start_matches("https://");
            if !domain.contains('.') || domain.contains(['/', '?', '#', '@', ':', '\\']) {
                return None;
            }
            return https_status_url(&format!("https://{domain}/"));
        }
    };
    https_status_url(endpoint)
}

pub fn site_url(description: &str) -> Option<String> {
    let trimmed = description.trim();
    let domain = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    if !domain.contains('.') || domain.contains(['/', '?', '#', '@', ':', '\\']) {
        return None;
    }
    https_status_url(&format!("https://{domain}/")).map(|_| format!("https://{domain}"))
}

fn https_status_url(value: &str) -> Option<reqwest::Url> {
    let url = reqwest::Url::parse(value).ok()?;
    (url.scheme() == "https" && url.has_host() && url.username().is_empty()
        && url.password().is_none() && url.fragment().is_none()).then_some(url)
}

/// Environment for a run: the manifest's env, plus `CLOAKBROWSER_HOME` for browser extensions.
pub fn command_env(manifest: &Manifest, fill: &Fill) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> =
        manifest.env.iter().map(|(k, v)| (k.clone(), fill_in(v, fill, None))).collect();
    if manifest.browser && !manifest.env.keys().any(|k| k.eq_ignore_ascii_case("CLOAKBROWSER_HOME")) {
        env.push(("CLOAKBROWSER_HOME".into(), fill.browser.into()));
    }
    env
}

/// Fills placeholders in one pass, so a pasted URL containing `{output}` stays literal.
fn fill_in(template: &str, fill: &Fill, value: Option<&str>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let Some(end) = tail.find('}') else {
            out.push_str(tail);
            return out;
        };
        let replacement = match &tail[1..end] {
            "url" => Some(fill.url),
            "file" => Some(fill.file),
            "output" => Some(fill.output),
            "data" => Some(fill.data),
            "browser" => Some(fill.browser),
            "value" => value,
            _ => None,
        };
        match replacement {
            Some(text) => out.push_str(text),
            None => out.push_str(&tail[..=end]),
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
#[path = "extensions_tests.rs"]
mod tests;
