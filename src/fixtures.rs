//! Fixed data for headless scenes and tests: extension manifests, a queue and a library.

use crate::{
    app::{App, Page},
    extensions::{self, Manifest},
    jobs::{Job, Status},
    library::Entry,
    runner::JobHandle,
    services,
    store::{ExtensionStatus, Installed, Paths, Theme, ThemePreset, Tools},
};
use rune::TextEdit;
use serde_json::Value;
use std::{path::PathBuf, sync::LazyLock};

pub static MANIFESTS: LazyLock<Vec<Value>> =
    LazyLock::new(|| serde_json::from_str(include_str!("fixtures/manifests.json")).expect("fixture manifest JSON"));

pub fn manifest_json(id: &str) -> &'static Value {
    MANIFESTS
        .iter()
        .find(|json| json["id"].as_str() == Some(id))
        .expect("known extension id")
}

pub fn manifest(id: &str) -> Manifest {
    extensions::parse_manifest(&manifest_json(id).to_string()).expect("valid fixture manifest")
}

pub fn installed(id: &str) -> Installed {
    Installed { manifest: manifest(id), dir: PathBuf::from(format!("C:/Flacora/extensions/{id}")) }
}

/// A loaded app with four installed extensions, helpers, and a small library.
pub fn app() -> App {
    let installed = ["cyan-dl", "flacdownloader", "odesli", "odskyler"].map(installed).into();
    let mut app = App::default();
    app.paths = Paths { root: PathBuf::from("C:/Users/you/AppData/Local/Flacora") };
    app.loaded = true;
    app.installed = installed;
    app.tools = Some(Tools { ffmpeg: Some("9.0".into()), browser: Some("145.0.7632.159.7".into()) });
    app.history = history();
    app.settings.output = PathBuf::from("C:\\Users\\you\\Music\\Flacora");
    app
}

fn history() -> Vec<Entry> {
    let entry = |title: &str, artist: &str, album: &str, service: &str, extension: &str, quality: &str, ago: u64| Entry {
        title: title.into(),
        artist: artist.into(),
        album: album.into(),
        service: service.into(),
        extension: extension.into(),
        quality: quality.into(),
        url: "https://example.com/track".into(),
        files: vec![format!("C:\\Users\\you\\Music\\Flacora\\{artist} - {title}.flac")],
        cover: None,
        date: crate::jobs::now().saturating_sub(ago),
    };
    vec![
        entry("Golden", "HUNTR/X", "KPop Demon Hunters", "Tidal", "Rhythmax", "FLAC 24-bit / 48 kHz", 600),
        entry("Golden", "HUNTR/X", "KPop Demon Hunters", "Amazon Music", "DoubleDouble", "FLAC", 7_200),
        entry("Golden", "HUNTR/X", "KPop Demon Hunters", "Qobuz", "FlacDownloader", "FLAC CD", 90_000),
        entry("Golden", "HUNTR/X", "KPop Demon Hunters", "Deezer", "FlacDownloader", "FLAC", 3_000_000),
    ]
}

pub fn job(app: &mut App, extension: &str, url: &str, status: Status) -> u64 {
    let id = app.next_job;
    app.next_job += 1;
    let host = extensions::url_host(url).unwrap_or_default();
    let extension = app.installed.iter().find(|i| i.manifest.id == extension).cloned().unwrap_or_else(|| installed(extension));
    app.jobs.push(Job {
        id,
        url: url.into(),
        lookup_file: None,
        service: services::detect(&host),
        picks: extension.manifest.options.iter().map(|o| o.default_index()).collect(),
        extension,
        output: app.settings.output.clone(),
        layout: app.settings.layout,
        auto_tag: app.settings.auto_tag,
        staging: None,
        handle: (status == Status::Running).then(JobHandle::detached),
        cancelling: false,
        status,
        title: None,
        artist: None,
        album: None,
        quality: None,
        stage: None,
        progress: None,
        cover: None,
        log: Vec::new(),
        saved: Vec::new(),
        fallback_saved: Vec::new(),
        show_log: false,
        finished: None,
        links: Vec::new(),
        choices: Vec::new(),
        batch_service: None,
        resolved: None,
    });
    id
}

fn describe(app: &mut App, id: u64, title: &str, artist: &str, album: &str, quality: &str) {
    if let Some(job) = app.jobs.iter_mut().find(|j| j.id == id) {
        job.title = Some(title.into());
        job.artist = Some(artist.into());
        job.album = Some(album.into());
        job.quality = Some(quality.into());
    }
}

// ------------------------------------------------------------------ scenes

pub fn home(s: &mut App) {
    *s = app();
}

pub fn downloads_batch_input(s: &mut App) {
    home(s);
    s.url = TextEdit::new("https://open.spotify.com/track/1CPZ5BxNNd0n0nF4Orb9JS https://open.spotify.com/track/3CPFHaVEHkkH26hAEPgMMp");
    s.set_batch_service(services::by_name("Tidal"));
}

pub fn queue(s: &mut App) {
    *s = app();
    let running = job(s, "odskyler", "https://tidal.com/track/441821360", Status::Running);
    describe(s, running, "Golden", "HUNTR/X", "KPop Demon Hunters", "FLAC 24-bit / 48 kHz");
    if let Some(job) = s.jobs.iter_mut().find(|j| j.id == running) {
        job.stage = Some("Downloading audio".into());
        job.progress = Some(0.62);
    }
    job(s, "cyan-dl", "https://music.amazon.com/tracks/B0FD256Q8F", Status::Queued);
    // A Spotify link Odesli found on Tidal, waiting for FlacDownloader.
    let resolved = job(s, "flacdownloader", "https://listen.tidal.com/track/532554866", Status::Queued);
    describe(s, resolved, "Golden", "HUNTR/X, EJAE, AUDREY NUNA, REI AMI", "", "");
    if let Some(job) = s.jobs.iter_mut().find(|j| j.id == resolved) {
        job.quality = None;
        job.album = None;
        job.resolved = Some(("Odesli".into(), services::detect("open.spotify.com")));
    }
    let done = job(s, "flacdownloader", "https://open.qobuz.com/track/341032040", Status::Done);
    describe(s, done, "Golden", "HUNTR/X", "KPop Demon Hunters", "FLAC CD");
    if let Some(job) = s.jobs.iter_mut().find(|j| j.id == done) {
        job.saved = vec!["C:\\Users\\you\\Music\\Flacora\\HUNTR_X - Golden.flac".into()];
        job.log = vec!["Tags written with cover art".into(), "Saved C:\\Users\\you\\Music\\Flacora\\HUNTR_X - Golden.flac (25579798 bytes)".into()];
    }
    let failed = job(s, "flacdownloader", "https://www.deezer.com/track/1", Status::Failed("FLAC is not available for this track".into()));
    if let Some(job) = s.jobs.iter_mut().find(|j| j.id == failed) {
        job.log = vec!["Deezer track: Unknown".into(), "Error: FLAC is not available for this track".into()];
        job.show_log = true;
    }
    s.url = TextEdit::new("https://tidal.com/track/441821360");
}

pub fn queue_actions(s: &mut App) {
    queue(s);
    s.queue_menu_open = true;
}

pub fn queue_cancel_confirmation(s: &mut App) {
    queue(s);
    s.confirm_action = Some(crate::app::ConfirmAction::CancelAll);
}

pub fn queue_indeterminate(s: &mut App) {
    queue(s);
    let job = &mut s.jobs[0];
    job.progress = None;
    job.stage = Some("Getting a download token".into());
}

/// Odesli found a Spotify link on Tidal, Deezer, and Amazon Music; the user picks.
pub fn choose(s: &mut App) {
    *s = app();
    let id = job(s, "odesli", "https://open.spotify.com/track/1CPZ5BxNNd0n0nF4Orb9JS", Status::Running);
    describe(s, id, "Golden", "HUNTR/X, EJAE, AUDREY NUNA, REI AMI", "", "");
    if let Some(job) = s.jobs.iter_mut().find(|j| j.id == id) {
        job.quality = None;
        job.album = None;
        job.handle = Some(JobHandle::detached());
    }
    let links = r#"@flacora {"event":"links","links":{"Tidal":"https://listen.tidal.com/track/532554866","Deezer":"https://www.deezer.com/track/3412534581","Amazon Music":"https://music.amazon.com/tracks/B0FGGSTFL2","Pandora":"https://www.pandora.com/TR:1","Spotify":"https://open.spotify.com/track/1CPZ5BxNNd0n0nF4Orb9JS"}}"#;
    s.apply(crate::app::Msg::Job(id, crate::runner::JobEvent::Line(links.into())));
    s.apply(crate::app::Msg::Job(id, crate::runner::JobEvent::Exit(Ok(0))));
}

pub fn light(s: &mut App) {
    queue(s);
    s.settings.theme = Theme::Light;
}

/// A Spotify link without Odesli installed: Flacora offers to install it.
pub fn missing(s: &mut App) {
    *s = app();
    s.installed.retain(|i| i.manifest.id != "odesli");
    s.url = TextEdit::new("https://open.spotify.com/track/1CPZ5BxNNd0n0nF4Orb9JS");
}

/// A Spotify link with Odesli installed.
pub fn spotify(s: &mut App) {
    *s = app();
    s.url = TextEdit::new("https://open.spotify.com/track/1CPZ5BxNNd0n0nF4Orb9JS");
}

/// A Spotify link with both Odesli and Squigly installed: the user picks the resolver.
pub fn resolvers(s: &mut App) {
    spotify(s);
    s.installed.push(installed("squigly"));
    s.installed.sort_by_key(|i| i.manifest.name.to_lowercase());
}

pub fn fresh(s: &mut App) {
    *s = app();
    s.installed.clear();
    s.history.clear();
}

pub fn library(s: &mut App) {
    *s = app();
    s.page = Page::Library;
}

pub fn library_search(s: &mut App) {
    library(s);
    s.search = TextEdit::new("Deezer");
}

pub fn library_single(s: &mut App) {
    library(s);
    s.history.truncate(1);
}

pub fn library_overflow(s: &mut App) {
    library(s);
    s.history = s.history.iter().cycle().take(16).cloned().collect();
}

pub fn library_lookups(s: &mut App) {
    library(s);
    s.history[0].url = "https://tidal.com/track/441821360".into();
    s.installed.extend([
        "betterlyrics", "lrclib", "lrc-red", "lyricsplus", "musixmatch", "musicbrainz",
        "spotify-web", "tidal-web", "deezer-web", "qobuz-web", "amazon-web",
    ].map(installed));
}

pub fn library_outdated_metadata(s: &mut App) {
    library_lookups(s);
    let web = s.installed.iter_mut().find(|i| i.manifest.id == "tidal-web").unwrap();
    web.manifest.version = "1.0.0".into();
    web.manifest.writes_tags = false;
}

pub fn library_sources(s: &mut App) {
    library_lookups(s);
    let legacy = &mut s.history[3];
    legacy.service.clear();
    legacy.url = "https://open.spotify.com/track/1CPZ5BxNNd0n0nF4Orb9JS".into();
}

pub fn library_batch(s: &mut App) {
    library_lookups(s);
    s.history[0].files = vec!["C:/Users/you/Music/Flacora/HUNTR_X - Golden (Tidal).flac".into()];
    s.history[1].files = vec!["C:/Users/you/Music/Flacora/HUNTR_X - Golden (Missing).flac".into()];
    s.library_files.insert(
        s.history[0].files[0].clone(),
        crate::library::SidecarInfo {
            exists: true,
            sidecars: vec![("LRC".into(), "C:/Users/you/Music/Flacora/HUNTR_X - Golden (Tidal).lrc".into())],
        },
    );
    s.library_files.insert(
        s.history[1].files[0].clone(),
        crate::library::SidecarInfo { exists: false, sidecars: Vec::new() },
    );
    s.library_select_mode = true;
    s.library_selected.insert(App::entry_selection_key(&s.history[0]));
    s.library_selected.insert(App::entry_selection_key(&s.history[2]));
}

pub fn lookup_picker(s: &mut App) {
    library_lookups(s);
    let track = &s.history[0];
    s.lookup_picker = track.flac_file().map(|file| (track.date, file.to_owned()));
}

pub fn catalog_results(s: &mut App) {
    *s = app();
    s.page = Page::Catalog;
    s.installed.extend(["deezer-web", "spotify-web", "tidal-web", "qobuz-web", "amazon-web"].map(installed));
    s.catalog.provider = Some("deezer-web".into());
    s.catalog.input = TextEdit::new("Golden");
    s.catalog.result = Some(crate::catalog::ResultPage {
        title: "Results for Golden".into(),
        note: String::new(),
        next: Some("50".into()),
        artwork: None,
        sections: Vec::new(),
        items: vec![
            crate::catalog::Item {
                kind: crate::catalog::Kind::Track,
                title: "Golden".into(),
                detail: "HUNTR/X".into(),
                url: "https://www.deezer.com/track/3412534581".into(),
                artwork: Some("https://example.com/track-art.jpg".into()),
            },
            crate::catalog::Item {
                kind: crate::catalog::Kind::Album,
                title: "KPop Demon Hunters".into(),
                detail: "Soundtrack from the Netflix Film".into(),
                url: "https://www.deezer.com/album/771853201".into(),
                artwork: Some("https://example.com/album-art.jpg".into()),
            },
            crate::catalog::Item {
                kind: crate::catalog::Kind::Artist,
                title: "KPop Demon Hunters Cast".into(),
                detail: String::new(),
                url: "https://www.deezer.com/artist/330311461".into(),
                artwork: None,
            },
        ],
    });
}

pub fn catalog_idle(s: &mut App) {
    *s = app();
    s.page = Page::Catalog;
    s.installed.extend(["deezer-web", "tidal-web"].map(installed));
    s.catalog.select_provider("deezer-web".into());
}

pub fn catalog_with_resolver_sources(s: &mut App) {
    catalog_idle(s);
    s.installed.extend(["apple-web", "ytmusic-web"].map(installed));
    s.catalog.select_provider("apple-web".into());
}

pub fn catalog_batch(s: &mut App) {
    catalog_results(s);
    s.installed.push(installed("squigly"));
    s.catalog.select_provider("spotify-web".into());
    let items: Vec<_> = (1..=50).map(|track| crate::catalog::Item {
        kind: crate::catalog::Kind::Track,
        title: format!("Sample Track {track}"),
        detail: "Sample Artist".into(),
        url: format!("https://open.spotify.com/track/{track:022}"),
        artwork: None,
    }).collect();
    s.catalog.selected = items.iter().map(|item| item.url.clone()).collect();
    s.catalog.result = Some(crate::catalog::ResultPage {
        title: "Sample Playlist".into(),
        note: String::new(),
        next: None,
        artwork: None,
        sections: Vec::new(),
        items,
    });
}

pub fn catalog_link(s: &mut App) {
    *s = app();
    s.installed.push(installed("deezer-web"));
    s.url = TextEdit::new("https://www.deezer.com/us/album/771853201");
}

pub fn catalog_feed(s: &mut App) {
    catalog_overflow(s);
    s.catalog.input = TextEdit::default();
    let page = s.catalog.result.as_mut().unwrap();
    page.title = "Deezer Charts".into();
    page.next = None;
    page.sections = [(crate::catalog::Kind::Track, "Top Tracks"), (crate::catalog::Kind::Album, "Top Albums"),
        (crate::catalog::Kind::Artist, "Top Artists")].into_iter().map(|(kind, title)| crate::catalog::FeedSection {
        title: title.into(),
        urls: page.items.iter().filter(|item| item.kind == kind).map(|item| item.url.clone()).collect(),
    }).collect();
}

pub fn catalog_feed_hidden(s: &mut App) {
    catalog_feed(s);
    s.toggle_catalog_home();
}

pub fn catalog_overflow(s: &mut App) {
    catalog_results(s);
    let result = s.catalog.result.as_mut().unwrap();
    result.items.extend((4..=12).map(|number| crate::catalog::Item {
        kind: crate::catalog::Kind::Track,
        title: format!("Sample Track {number}"),
        detail: "Sample Artist".into(),
        url: format!("https://www.deezer.com/track/{number}"),
        artwork: None,
    }));
}

pub fn catalog_missing_extension(s: &mut App) {
    catalog_overflow(s);
    s.installed.retain(|extension| extension.manifest.id != "spotify-web");
    s.catalog.input = TextEdit::new("https://open.spotify.com/artist/06HL4z0CvFAxyc27GXpf02");
    s.submit_catalog();
}

pub fn lookup_queue(s: &mut App) {
    library_lookups(s);
    s.enqueue_lookup(0, "musixmatch");
    let job = &mut s.jobs[0];
    job.status = Status::Done;
    job.saved.push(job.lookup_file.as_ref().unwrap().with_extension("lrc").to_string_lossy().into_owned());
}

/// Installed extensions across all categories, including Lucida's live services.
pub fn extensions_page(s: &mut App) {
    *s = app();
    s.page = Page::Extensions;
    s.installed.extend(["lucida", "monochrome", "squigly", "betterlyrics", "lrclib", "lyricsplus", "musixmatch", "musicbrainz"].map(installed));
    let legacy_status = r#"@flacora {"event":"services","services":["Qobuz","Amazon Music","Yandex Music"]}"#;
    let Some(crate::protocol::Event::Services(services)) = crate::protocol::parse(legacy_status) else {
        unreachable!("valid status fixture");
    };
    s.extension_status.insert("lucida".into(), ExtensionStatus::Services(services));
}

/// Newer versions are fixture data; the public Gist is never modified.
pub fn extension_updates_available(s: &mut App) {
    extensions_page(s);
    let versions = crate::extension_updates::parse(br#"{"schema_version":1,"extensions":{"cyan-dl":{"version":"1.0.3"},"flacdownloader":{"version":"1.1.0"},"odskyler":{"version":"1.0.2"}}}"#).unwrap();
    s.apply(crate::app::Msg::ExtensionVersionsChecked(Ok(versions)));
}

pub fn extension_updates_light(s: &mut App) {
    extension_updates_available(s);
    s.settings.theme = Theme::Light;
}

pub fn extension_updates_current(s: &mut App) {
    extensions_page(s);
    let versions = crate::extension_updates::parse(include_bytes!("fixtures/extension-updates.json")).unwrap();
    s.apply(crate::app::Msg::ExtensionVersionsChecked(Ok(versions)));
}

pub fn extension_updates_failed(s: &mut App) {
    extension_updates_available(s);
    s.apply(crate::app::Msg::ExtensionVersionsChecked(Err("offline fixture".into())));
}

pub fn extension_updates_checking(s: &mut App) {
    extensions_page(s);
    s.check_extension_updates();
}

pub fn extensions_empty(s: &mut App) {
    fresh(s);
    s.page = Page::Extensions;
}

pub fn extensions_remove_confirmation(s: &mut App) {
    extensions_page(s);
    s.confirm_action = Some(crate::app::ConfirmAction::RemoveAllExtensions);
}

pub fn extensions_dropping(s: &mut App) {
    extensions_page(s);
    s.drop_over = true;
}

pub fn extensions_installing(s: &mut App) {
    extensions_page(s);
    s.importing = 1;
}

pub fn extension_badges(s: &mut App) {
    *s = app();
    s.page = Page::Extensions;
    s.installed = ["betterlyrics", "flacdownloader", "monochrome", "musicbrainz", "odesli"].map(installed).into();
}

pub fn extension_status(s: &mut App) {
    *s = app();
    s.page = Page::Extensions;
    s.installed = ["cyan-dl", "lucida", "odesli", "musicbrainz", "betterlyrics", "musixmatch"].map(installed).into();
    s.extension_status.extend([
        ("cyan-dl".into(), ExtensionStatus::Online),
        ("lucida".into(), ExtensionStatus::Services(Vec::new())),
        ("odesli".into(), ExtensionStatus::Unknown("The status endpoint returned HTTP 403.".into())),
        ("musicbrainz".into(), ExtensionStatus::Online),
        ("betterlyrics".into(), ExtensionStatus::Degraded),
        ("musixmatch".into(), ExtensionStatus::Services(vec!["Musixmatch".into()])),
    ]);
}

pub fn extension_options(s: &mut App) {
    *s = app();
    s.page = Page::Extensions;
    s.installed = ["cyan-dl", "doubledouble", "odskyler", "odesli"].map(installed).into();
    s.extension_settings = Some("odskyler".into());
    s.url = TextEdit::new("https://tidal.com/track/441821360");
}

pub fn settings(s: &mut App) {
    *s = app();
    s.page = Page::Settings;
    s.installed.extend(["lucida", "doubledouble", "squigly"].map(installed));
}

pub fn settings_matcha(s: &mut App) {
    settings(s);
    s.settings.theme = Theme::Light;
    s.settings.theme_preset = ThemePreset::Matcha;
}

pub fn settings_update_available(s: &mut App) {
    settings(s);
    let current = semver::Version::parse(crate::updates::CURRENT_VERSION).unwrap();
    let release = semver::Version::new(current.major, current.minor + 1, 0);
    s.available_update = crate::updates::from_tag(&format!("v{release}")).unwrap();
}

pub fn output_settings(s: &mut App) {
    settings(s);
    s.settings.layout = crate::output::Layout { filename: crate::output::Filename::TitleArtist, folders: crate::output::Folders::ArtistAlbum };
}

pub fn auto_tag_available_settings(s: &mut App) {
    settings(s);
    s.installed.push(installed("tidal-web"));
}

pub fn auto_tag_settings(s: &mut App) {
    auto_tag_available_settings(s);
    s.settings.auto_tag = true;
}

pub fn settings_without_ffmpeg(s: &mut App) {
    settings(s);
    if let Some(tools) = &mut s.tools {
        tools.ffmpeg = None;
    }
}

pub fn settings_without_browser(s: &mut App) {
    settings(s);
    if let Some(tools) = &mut s.tools {
        tools.browser = None;
    }
}

pub fn dropping(s: &mut App) {
    queue(s);
    s.drop_over = true;
}

pub fn imported(s: &mut App) {
    extensions_page(s);
    let extension = installed("doubledouble");
    let manifest = &extension.manifest;
    let message = format!("Installed {} {} from {}-{}-windows-x64.fext",
        manifest.name, manifest.version, manifest.id, manifest.version);
    s.installed.push(extension);
    s.installed.sort_by_key(|i| i.manifest.name.to_lowercase());
    s.extension_status.insert("doubledouble".into(), ExtensionStatus::Services(vec!["Amazon Music".into()]));
    s.success(message);
}

pub fn catalog_empty(s: &mut App) {
    *s = app();
    s.page = Page::Catalog;
}

pub fn notifications(s: &mut App) {
    imported(s);
    s.error("broken.fext: This is not a Flacora extension package");
}

pub fn loading(s: &mut App) {
    let mut app = App::default();
    app.paths = Paths { root: PathBuf::from("C:/Users/you/AppData/Local/Flacora") };
    *s = app;
}
