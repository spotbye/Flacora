use reqwest::Url;

/// Recognizes collection URLs without rejecting short links or unknown provider routes.
pub fn is_collection(url: &Url) -> bool {
    let (Some(host), Some(segments)) = (url.host_str(), url.path_segments()) else { return false };
    let mut segments = segments.filter(|part| !part.is_empty());
    let Some(first) = segments.next() else { return false };

    if host == "open.spotify.com" {
        let mut kind = Some(first);
        while let Some(part) = kind {
            if part.starts_with("intl-") || part == "embed" {
                kind = segments.next();
            } else if part == "user" {
                let _ = segments.next();
                kind = segments.next();
            } else {
                break;
            }
        }
        collection_kind(kind)
    } else if host_is(host, "tidal.com") {
        collection_kind(if first == "browse" { segments.next() } else { Some(first) })
    } else if host_is(host, "deezer.com") {
        collection_kind(if locale(first) { segments.next() } else { Some(first) })
    } else if matches!(host, "music.apple.com" | "itunes.apple.com" | "geo.music.apple.com") {
        match if locale(first) { segments.next() } else { Some(first) } {
            Some("album") => !url.query_pairs().any(|(key, id)| key == "i" && !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit())),
            kind => collection_kind(kind),
        }
    } else if crate::services::AMAZON_HOSTS.contains(&host) {
        match first {
            "album" | "albums" => amazon_track_id(url).is_none(),
            "user-playlists" => true,
            _ => collection_kind(Some(first)),
        }
    } else if host_is(host, "youtube.com") {
        matches!(first, "playlist" | "channel" | "user" | "c") || first.starts_with('@')
    } else {
        host_is(host, "bandcamp.com") && first == "album"
    }
}

fn amazon_track_id(url: &Url) -> Option<String> {
    url.query_pairs().find_map(|(key, id)| {
        (key == "trackAsin" && id.len() == 10 && id.bytes().all(|b| b.is_ascii_alphanumeric())).then(|| id.into_owned())
    })
}

fn host_is(host: &str, domain: &str) -> bool {
    host == domain || host.strip_suffix(domain).is_some_and(|prefix| prefix.ends_with('.'))
}

fn locale(part: &str) -> bool {
    part.len() == 2 && part.bytes().all(|b| b.is_ascii_alphabetic())
}

fn collection_kind(kind: Option<&str>) -> bool {
    matches!(kind, Some("album" | "albums" | "playlist" | "playlists" | "artist" | "artists"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_collection_routes() {
        for link in [
            "https://open.spotify.com/album/7aJuG4TFXa2hmE4z1yxc3n",
            "https://open.spotify.com/intl-id/playlist/37i9dQZEVXbNG2KDcFcKOF",
            "https://open.spotify.com/intl-en/embed/album/7aJuG4TFXa2hmE4z1yxc3n",
            "https://open.spotify.com/user/spotify/playlist/37i9dQZEVXbNG2KDcFcKOF",
            "https://open.spotify.com/artist/06HL4z0CvFAxyc27GXpf02",
            "https://tidal.com/browse/album/441821356",
            "https://www.deezer.com/en/artist/330311461",
            "https://music.amazon.com/albums/B0FD256Q8D?trackAsin=bad",
            "https://music.amazon.fr/albums/B0FD256Q8D",
            "https://music.amazon.com.au/user-playlists/example",
            "https://music.apple.com/us/album/golden/1820264137",
            "https://music.youtube.com/playlist?list=OLAK5uy_example",
            "https://a.bandcamp.com/album/example",
        ] {
            assert!(is_collection(&Url::parse(link).unwrap()), "{link}");
        }
    }

    #[test]
    fn leaves_tracks_and_short_links_available() {
        for link in [
            "https://open.spotify.com/track/1CPZ5BxNNd0n0nF4Orb9JS",
            "https://tidal.com/browse/track/441821360",
            "https://www.deezer.com/en/track/3412534581",
            "https://music.amazon.com/albums/B0FD256Q8D?trackAsin=B0FD256Q8F",
            "https://geo.music.apple.com/us/album/golden/1820264137?i=1820264150",
            "https://music.youtube.com/watch?v=9_bTl2vvYQg&list=OLAK5uy_example",
            "https://a.bandcamp.com/track/example",
            "https://spotify.link/abcdef",
            "https://deezer.page.link/abcdef",
            "https://song.link/s/1CPZ5BxNNd0n0nF4Orb9JS",
            "https://squigly.link/f99fhc",
        ] {
            assert!(!is_collection(&Url::parse(link).unwrap()), "{link}");
        }
    }
}
