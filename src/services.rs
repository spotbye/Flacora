//! Music services Flacora knows by name and host.

use crate::extensions::host_matches;

#[derive(Debug, PartialEq, Eq)]
pub struct Service {
    pub key: &'static str,
    pub name: &'static str,
    hosts: &'static [&'static str],
}

impl Service {
    pub fn primary_host(&self) -> &'static str {
        self.hosts[0]
    }
}

pub const AMAZON_HOSTS: &[&str] = &[
    "music.amazon.com",
    "music.amazon.co.uk",
    "music.amazon.de",
    "music.amazon.co.jp",
    "music.amazon.fr",
    "music.amazon.it",
    "music.amazon.es",
    "music.amazon.ca",
    "music.amazon.com.au",
    "music.amazon.com.br",
    "music.amazon.com.mx",
    "music.amazon.in",
];

/// The primary download services come first, followed by the optional Monochrome source and
/// resolver input services. YouTube Music comes before YouTube so its host matches first.
pub const SERVICES: [Service; 9] = [
    Service { key: "qobuz", name: "Qobuz", hosts: &["qobuz.com"] },
    Service { key: "tidal", name: "Tidal", hosts: &["tidal.com"] },
    Service { key: "deezer", name: "Deezer", hosts: &["deezer.com", "deezer.page.link"] },
    Service { key: "amazon", name: "Amazon Music", hosts: AMAZON_HOSTS },
    Service { key: "monochrome", name: "Monochrome", hosts: &["monochrome.st"] },
    Service { key: "spotify", name: "Spotify", hosts: &["open.spotify.com", "spotify.link"] },
    Service { key: "apple", name: "Apple Music", hosts: &["music.apple.com", "geo.music.apple.com", "itunes.apple.com"] },
    Service { key: "youtube-music", name: "YouTube Music", hosts: &["music.youtube.com"] },
    Service { key: "youtube", name: "YouTube", hosts: &["youtube.com", "youtu.be"] },
];

/// How many of `SERVICES` appear in the primary download settings.
pub const PRIMARY_DOWNLOAD_COUNT: usize = 4;

pub fn is_excluded_host(host: &str) -> bool {
    host_matches(&["music.yandex.ru", "music.yandex.com", "pandora.com"], host.trim_end_matches('.'))
}

pub fn is_excluded_service(name: &str) -> bool {
    ["yandex", "Yandex Music", "Pandora"].iter().any(|known| name.trim().eq_ignore_ascii_case(known))
}

/// Embedded artwork for music services.
pub const LOGOS: [(&str, &[u8]); 7] = [
    ("deezer", include_bytes!("../assets/providers/deezer.png")),
    ("qobuz", include_bytes!("../assets/providers/qobuz.png")),
    ("tidal", include_bytes!("../assets/providers/tidal.png")),
    ("youtube-music", include_bytes!("../assets/providers/ytmusic.png")),
    ("amazon", include_bytes!("../assets/providers/amazon.png")),
    ("apple", include_bytes!("../assets/providers/apple.png")),
    ("spotify", include_bytes!("../assets/providers/spotify.png")),
];

/// The service a link's host belongs to.
pub fn detect(host: &str) -> Option<&'static Service> {
    SERVICES.iter().find(|s| host_matches(s.hosts, host))
}

pub fn by_name(name: &str) -> Option<&'static Service> {
    SERVICES.iter().find(|s| s.name.eq_ignore_ascii_case(name) || s.key == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_resolve_to_services() {
        assert_eq!(detect("listen.tidal.com").map(|s| s.key), Some("tidal"));
        assert_eq!(detect("open.qobuz.com").map(|s| s.key), Some("qobuz"));
        assert_eq!(detect("music.amazon.com").map(|s| s.key), Some("amazon"));
        assert_eq!(detect("tracks.monochrome.st").map(|s| s.key), Some("monochrome"));
        assert!(detect("example.com").is_none());
        assert_eq!(detect("music.youtube.com").map(|s| s.key), Some("youtube-music"));
        assert_eq!(detect("www.youtube.com").map(|s| s.key), Some("youtube"));
        assert_eq!(detect("open.spotify.com").map(|s| s.key), Some("spotify"));
        assert_eq!(detect("geo.music.apple.com").map(|s| s.key), Some("apple"));
        assert_eq!(by_name("Amazon Music").map(|s| s.key), Some("amazon"));
        assert!(detect("music.yandex.ru").is_none());
        assert!(by_name("Yandex Music").is_none());
        let primary: Vec<_> = SERVICES[..PRIMARY_DOWNLOAD_COUNT].iter().map(|service| service.key).collect();
        assert_eq!(primary, ["qobuz", "tidal", "deezer", "amazon"]);
        for service in &SERVICES[..PRIMARY_DOWNLOAD_COUNT] {
            assert_eq!(detect(service.primary_host()).map(|s| s.key), Some(service.key));
        }
    }
}
