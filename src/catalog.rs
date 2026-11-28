//! Catalog browsing uses an installed metadata extension and keeps track selection in the UI.

use crate::{app::{App, Msg, Page}, extensions::{self, Fill}, library, protocol, runner::{self, JobEvent, JobHandle}, store::Installed};
use reqwest::{Url, blocking::Client};
use rune::{Image, TextEdit};
use serde::Deserialize;
use std::{collections::{BTreeMap, BTreeSet, VecDeque}, sync::{Arc, LazyLock}, time::Duration};

pub const MISSING_CATALOG_EXTENSION: &str = "No catalog extension found for this link. Install a compatible extension.";

const ARTWORK_WORKERS: usize = 6;
const ARTWORK_CACHE: usize = 192;
const ARTWORK_BYTES: u64 = 2 << 20;
const MAX_TRAIL: usize = 20;
const MAX_CATALOG_ITEMS: usize = 1000;
const MAX_FEED_SECTIONS: usize = 32;
const CATALOG_TIMEOUT: Duration = Duration::from_secs(45);
static ARTWORK_CLIENT: LazyLock<Option<Client>> = LazyLock::new(|| {
    Client::builder().https_only(true).timeout(Duration::from_secs(10))
        .user_agent(concat!("Flacora/", env!("CARGO_PKG_VERSION"))).build().ok()
});

use crate::track_links;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Track,
    Album,
    Playlist,
    Artist,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Track => "Track",
            Self::Album => "Album",
            Self::Playlist => "Playlist",
            Self::Artist => "Artist",
        }
    }

    pub fn plural(self) -> &'static str {
        match self {
            Self::Track => "Tracks",
            Self::Album => "Albums",
            Self::Playlist => "Playlists",
            Self::Artist => "Artists",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Item {
    pub kind: Kind,
    pub title: String,
    #[serde(default)]
    pub detail: String,
    pub url: String,
    pub artwork: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ResultPage {
    pub title: String,
    #[serde(default)]
    pub note: String,
    pub items: Vec<Item>,
    pub next: Option<String>,
    pub artwork: Option<String>,
    #[serde(default)]
    pub sections: Vec<FeedSection>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FeedSection {
    pub title: String,
    pub urls: Vec<String>,
}

impl ResultPage {
    fn validate_sections(&mut self) {
        let items: BTreeSet<_> = self.items.iter().map(|item| item.url.as_str()).collect();
        self.sections.truncate(MAX_FEED_SECTIONS);
        for section in &mut self.sections {
            let mut seen = BTreeSet::new();
            section.urls.retain(|url| items.contains(url.as_str()) && seen.insert(url.clone()));
        }
        self.sections.retain(|section| !section.title.trim().is_empty() && !section.urls.is_empty());
    }

    fn append(&mut self, mut page: Self) {
        let mut seen: BTreeSet<_> = self.items.iter().map(|item| item.url.clone()).collect();
        let room = MAX_CATALOG_ITEMS.saturating_sub(self.items.len());
        self.items.extend(page.items.drain(..).filter(|item| seen.insert(item.url.clone())).take(room));
        self.next = if self.items.len() >= MAX_CATALOG_ITEMS { None } else { page.next };
        self.note = page.note;
        for section in page.sections {
            if let Some(current) = self.sections.iter_mut().find(|current| current.title == section.title) {
                current.urls.extend(section.urls);
            } else {
                self.sections.push(section);
            }
        }
        self.validate_sections();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apple_and_youtube_music_collections_go_to_catalog_and_tracks_go_to_resolvers() {
        for catalog in [
            "https://music.apple.com/id/new", "https://music.apple.com/us/album/hey-jude/1441133100",
            "https://music.youtube.com/explore", "https://music.youtube.com/browse/MPREb_EB1GQowDeAh",
            "https://music.youtube.com/playlist?list=OLAK5uy_example",
        ] {
            assert!(is_catalog_url(catalog), "{catalog}");
        }
        for track in [
            "https://music.apple.com/us/album/hey-jude/1441133100?i=1441133277",
            "https://music.apple.com/us/song/hey-jude/1441133277",
            "https://music.youtube.com/watch?v=KQetemT1sWc&list=OLAK5uy_example",
        ] {
            assert!(!is_catalog_url(track), "{track}");
        }
    }

    #[test]
    fn an_appended_feed_page_extends_its_shelf_without_duplicate_cards() {
        let feed = |ids: &[&str]| {
            let urls: Vec<_> = ids.iter().map(|id| format!("https://www.qobuz.com/playlist/{id}")).collect();
            ResultPage {
                title: "Qobuz Home".into(),
                items: urls.iter().map(|url| Item {
                    kind: Kind::Playlist, title: "Playlist".into(), detail: String::new(), url: url.clone(), artwork: None,
                }).collect(),
                sections: vec![FeedSection { title: "Featured Playlists".into(), urls }],
                note: String::new(),
                next: Some("50".into()),
                artwork: None,
            }
        };
        let mut first = feed(&["1", "2"]);
        let mut next = feed(&["2", "3"]);
        next.next = Some("100".into());
        first.append(next);
        assert_eq!(first.sections.len(), 1);
        assert_eq!(first.items.len(), 3);
        assert!(first.sections[0].urls.iter().map(String::as_str).eq(first.items.iter().map(|item| item.url.as_str())));
        assert_eq!(first.next.as_deref(), Some("100"));
    }
}

#[derive(Clone)]
enum Query {
    Open(String),
    Search(String),
    Home,
}

#[derive(Clone)]
struct Request {
    provider: String,
    query: Query,
}

struct OwnedProcess(JobHandle);

impl Drop for OwnedProcess {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

struct Pending {
    id: u64,
    request: Request,
    cursor: Option<String>,
    append: bool,
    process: OwnedProcess,
    result: Option<ResultPage>,
    error: Option<String>,
}

#[derive(Default)]
pub struct Catalog {
    pub input: TextEdit,
    pub provider: Option<String>,
    pub result: Option<ResultPage>,
    pub selected: BTreeSet<String>,
    pub error: Option<String>,
    request: Option<Request>,
    last_failed: Option<(Request, Option<String>)>,
    trail: Vec<(Request, ResultPage, BTreeSet<String>)>,
    hidden_home: Option<(Option<Request>, ResultPage, BTreeSet<String>)>,
    pending: Option<Pending>,
    serial: u64,
    artwork: BTreeMap<String, Option<Image>>,
    artwork_queue: VecDeque<String>,
    artwork_cached: VecDeque<String>,
    artwork_loading: usize,
}

fn valid_url(value: &str) -> Option<Url> {
    let url = Url::parse(value).ok()?;
    (url.scheme() == "https" && url.username().is_empty() && url.password().is_none() && url.port().is_none())
        .then_some(url)
}

pub fn is_catalog_url(value: &str) -> bool {
    let Some(url) = valid_url(value) else { return false };
    if track_links::is_collection(&url) {
        return true;
    }
    let Some(host) = url.host_str() else { return false };
    if host == "music.youtube.com" {
        return matches!(url.path().trim_end_matches('/'), "" | "/home" | "/explore" | "/search")
            || url.path().starts_with("/browse/");
    }
    if matches!(host, "music.apple.com" | "geo.music.apple.com" | "itunes.apple.com") {
        let mut parts = url.path_segments().into_iter().flatten().filter(|part| !part.is_empty());
        let first = parts.next();
        let route = if first.is_some_and(|part| part.len() == 2 && part.bytes().all(|b| b.is_ascii_alphabetic())) {
            parts.next()
        } else {
            first
        };
        return matches!(route, None | Some("browse" | "home" | "new" | "search"));
    }
    let browse_host = ["open.spotify.com", "tidal.com", "deezer.com", "qobuz.com"]
        .into_iter()
        .chain(crate::services::AMAZON_HOSTS.iter().copied())
        .any(|domain| host == domain || host.strip_suffix(domain).is_some_and(|part| part.ends_with('.')));
    if browse_host && (matches!(url.path().trim_end_matches('/'), "" | "/browse" | "/home")
        || url.path_segments().into_iter().flatten().any(|part| matches!(part, "search" | "user-playlists")))
    {
        return true;
    }
    if !(host == "qobuz.com" || host.ends_with(".qobuz.com")) {
        return false;
    }
    let has_track = url.fragment().is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()));
    !has_track && url.path_segments().into_iter().flatten()
        .any(|part| matches!(part, "album" | "playlist" | "playlists" | "artist" | "interpreter"))
}

impl Catalog {
    pub fn select_provider(&mut self, id: String) {
        self.cancel();
        self.clear_artwork_queue();
        self.clear_failed_artwork();
        self.provider = Some(id);
        self.result = None;
        self.request = None;
        self.last_failed = None;
        self.trail.clear();
        self.hidden_home = None;
        self.selected.clear();
        self.error = None;
    }

    pub fn clear_for_removed(&mut self, id: &str) {
        if self.active_extension() == Some(id) {
            self.cancel();
        }
        if self.provider.as_deref() == Some(id) {
            self.clear_artwork_queue();
            self.clear_failed_artwork();
            self.provider = None;
            self.result = None;
            self.request = None;
            self.last_failed = None;
            self.hidden_home = None;
            self.selected.clear();
            self.error = None;
        }
        self.trail.retain(|(req, _, _)| req.provider != id);
        if self.last_failed.as_ref().is_some_and(|(req, _)| req.provider == id) {
            self.last_failed = None;
        }
    }

    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }

    pub fn showing_home(&self) -> bool {
        self.result.as_ref().is_some_and(|page| !page.sections.is_empty())
            || self.request.as_ref().is_some_and(|request| match &request.query {
                Query::Home => true,
                Query::Open(url) => valid_url(url).is_some_and(|url| matches!(url.path().trim_end_matches('/'), "" | "/browse" | "/home")),
                Query::Search(_) => false,
            })
    }

    pub fn home_feed_active(&self) -> bool {
        match &self.pending {
            Some(pending) => matches!(pending.request.query, Query::Home) || (pending.append && self.showing_home()),
            None => self.result.is_some() && self.showing_home(),
        }
    }

    pub fn can_go_back(&self) -> bool {
        !self.trail.is_empty()
    }

    pub fn can_retry(&self) -> bool {
        self.last_failed.is_some() && !self.busy()
    }

    pub fn running_extension(&self, id: &str) -> bool {
        self.active_extension() == Some(id)
    }

    pub fn active_extension(&self) -> Option<&str> {
        self.pending.as_ref().map(|pending| pending.request.provider.as_str())
    }

    pub fn cancel(&mut self) {
        if let Some(pending) = self.pending.take() {
            pending.process.0.cancel();
        }
    }

    pub fn back(&mut self) {
        self.cancel();
        self.clear_artwork_queue();
        self.clear_failed_artwork();
        if let Some((request, result, selected)) = self.trail.pop() {
            self.provider = Some(request.provider.clone());
            self.request = Some(request);
            self.result = Some(result);
            self.selected = selected;
            self.last_failed = None;
            self.error = None;
        }
    }

    pub fn artwork(&self, url: &str) -> Option<&Image> {
        self.artwork.get(url)?.as_ref().filter(|image| image.error().is_none())
    }

    fn clear_artwork_queue(&mut self) {
        for url in self.artwork_queue.drain(..) {
            self.artwork.remove(&url);
        }
    }

    fn clear_failed_artwork(&mut self) {
        let queued: BTreeSet<&str> = self.artwork_queue.iter().map(String::as_str).collect();
        self.artwork.retain(|url, img| img.is_some() || queued.contains(url.as_str()));
        self.artwork_cached.retain(|url| self.artwork.contains_key(url));
    }

    pub fn event(&mut self, id: u64, event: JobEvent) {
        let Some(pending) = self.pending.as_mut().filter(|p| p.id == id) else { return };
        match event {
            JobEvent::Line(line) => {
                if let Some(json) = line.strip_prefix(protocol::PREFIX)
                    && let Ok(mut data) = serde_json::from_str::<serde_json::Value>(json)
                    && data["event"] == "catalog"
                    && let Ok(mut page) = serde_json::from_value::<ResultPage>(data["page"].take())
                {
                    let mut seen = BTreeSet::new();
                    page.items.retain(|item| {
                        !item.title.trim().is_empty()
                            && valid_url(&item.url).is_some()
                            && seen.insert(item.url.clone())
                    });
                    page.items.truncate(250);
                    page.validate_sections();
                    pending.result = Some(page);
                } else if let Some(error) = protocol::error_line(&line) {
                    pending.error = Some(error);
                }
            }
            JobEvent::Exit(exit) => {
                let pending = self.pending.take().unwrap();
                if let (Ok(0), Some(next)) = (&exit, pending.result) {
                    if pending.append && let Some(page) = &mut self.result {
                        page.append(next);
                    } else {
                        self.result = Some(next);
                    }
                    self.request = Some(pending.request);
                    self.last_failed = None;
                } else {
                    self.last_failed = Some((pending.request, pending.cursor));
                    self.error = Some(pending.error.or_else(|| exit.err()).unwrap_or_else(|| "The extension returned no catalog results.".into()));
                }
            }
        }
    }
}

impl App {
    pub fn catalog_providers(&self) -> impl Iterator<Item = &Installed> {
        self.installed.iter().filter(|i| i.manifest.catalog && !self.removing.contains(&i.manifest.id))
    }

    pub fn chosen_catalog_provider(&self) -> Option<&Installed> {
        if let Some(id) = &self.catalog.provider
            && let Some(provider) = self.catalog_providers().find(|i| i.manifest.id == *id)
        {
            return Some(provider);
        }
        let mut providers = self.catalog_providers();
        let provider = providers.next()?;
        providers.next().is_none().then_some(provider)
    }

    fn catalog_provider_for(&self, url: &str) -> Result<String, &'static str> {
        let missing = MISSING_CATALOG_EXTENSION;
        let url = valid_url(url).ok_or(missing)?;
        let host = url.host_str().ok_or(missing)?;
        if crate::services::is_excluded_host(host) {
            return Err(missing);
        }
        let mut matches = self.catalog_providers().filter(|p| extensions::host_matches(&p.manifest.hosts, host));
        let first = matches.next().ok_or(missing)?;
        if matches.next().is_none() {
            Ok(first.manifest.id.clone())
        } else {
            self.chosen_catalog_provider()
                .filter(|p| extensions::host_matches(&p.manifest.hosts, host))
                .map(|p| p.manifest.id.clone())
                .ok_or("Choose a catalog extension that supports this link.")
        }
    }

    pub fn submit_catalog(&mut self) {
        let input = self.catalog.input.text.trim().to_owned();
        if input.is_empty() {
            return;
        }
        if input.contains("://") {
            self.open_catalog(input, false);
        } else if let Some(provider) = self.chosen_catalog_provider().map(|p| p.manifest.id.clone()) {
            self.catalog.trail.clear();
            self.start_catalog(Request { provider, query: Query::Search(input) }, None);
        } else {
            self.catalog.error = Some("Choose a catalog extension first.".into());
        }
    }

    pub fn open_catalog(&mut self, url: String, nested: bool) {
        self.page = Page::Catalog;
        if !nested {
            self.catalog.input = TextEdit::new(&url);
        }
        let provider = match self.catalog_provider_for(&url) {
            Ok(provider) => provider,
            Err(error) => {
                if !nested {
                    self.catalog.result = None;
                    self.catalog.selected.clear();
                    self.catalog.trail.clear();
                }
                self.catalog.error = Some(error.into());
                return;
            }
        };
        if self.wait_for_import() {
            return;
        }
        let Some(installed) = self.catalog_providers().find(|p| p.manifest.id == provider) else {
            self.catalog.error = Some("The catalog extension is unavailable.".into());
            return;
        };
        if installed.executable().is_none() {
            self.catalog.error = Some("The catalog extension has no build for this computer.".into());
            return;
        }
        if nested {
            if let (Some(previous), Some(result)) = (self.catalog.request.take(), self.catalog.result.take()) {
                if self.catalog.trail.len() >= MAX_TRAIL {
                    self.catalog.trail.remove(0);
                }
                self.catalog.trail.push((previous, result, std::mem::take(&mut self.catalog.selected)));
            }
        } else {
            self.catalog.trail.clear();
        }
        self.start_catalog(Request { provider, query: Query::Open(url) }, None);
    }

    pub fn toggle_catalog_home(&mut self) {
        if self.catalog.home_feed_active() {
            self.catalog.cancel();
            self.catalog.clear_artwork_queue();
            let request = self.catalog.request.take();
            let selected = std::mem::take(&mut self.catalog.selected);
            self.catalog.hidden_home = self.catalog.result.take().map(|page| (request, page, selected));
            self.catalog.trail.clear();
            self.catalog.error = None;
            self.catalog.last_failed = None;
            return;
        }
        let Some(provider) = self.chosen_catalog_provider().map(|p| p.manifest.id.clone()) else {
            self.catalog.error = Some("Choose a catalog extension first.".into());
            return;
        };

        if self.catalog.provider.as_deref() == Some(provider.as_str())
            && let Some((request, page, selected)) = self.catalog.hidden_home.take()
        {
            self.catalog.request = request;
            self.catalog.result = Some(page);
            self.catalog.selected = selected;
            self.catalog.error = None;
            return;
        }

        self.catalog.trail.clear();
        self.start_catalog(Request { provider, query: Query::Home }, None);
    }

    pub fn catalog_more(&mut self) {
        if !self.catalog.busy()
            && let Some(request) = self.catalog.request.clone()
            && let Some(next) = self.catalog.result.as_ref().and_then(|result| result.next.clone())
        {
            self.start_catalog(request, Some(next));
        }
    }

    fn start_catalog(&mut self, request: Request, cursor: Option<String>) {
        if self.wait_for_import() {
            return;
        }
        let Some(installed) = self.catalog_providers().find(|p| p.manifest.id == request.provider).cloned() else {
            self.catalog.error = Some("The catalog extension is unavailable.".into());
            return;
        };
        let Some(program) = installed.executable() else {
            self.catalog.error = Some("The catalog extension has no build for this computer.".into());
            return;
        };
        self.catalog.cancel();
        self.catalog.error = None;
        self.catalog.provider = Some(request.provider.clone());
        let cursor = cursor.filter(|c| !c.is_empty() && !c.chars().any(char::is_control));
        let append = cursor.is_some();
        if !append {
            self.catalog.clear_artwork_queue();
            self.catalog.clear_failed_artwork();
            self.catalog.result = None;
            self.catalog.hidden_home = None;
            self.catalog.selected.clear();
        }
        let Some(link) = &self.link else { return };
        let mut args = match &request.query {
            Query::Open(url) => vec!["--browse".into(), url.clone()],
            Query::Search(query) => vec!["--search".into(), query.clone()],
            Query::Home => vec!["--home".into()],
        };
        if let Some(cursor_val) = &cursor {
            args.extend(["--cursor".into(), cursor_val.clone()]);
        }
        let cwd = self.paths.data(&request.provider);
        let browser = self.paths.browser();
        let fill = Fill { file: "", url: "", output: "", data: &cwd.to_string_lossy(), browser: &browser.to_string_lossy() };
        let env = extensions::command_env(&installed.manifest, &fill);
        self.catalog.serial += 1;
        let id = self.catalog.serial;
        let send = link.sender();
        let process = JobHandle::detached();
        self.catalog.pending = Some(Pending {
            id,
            request,
            cursor,
            append,
            process: OwnedProcess(process.clone()),
            result: None,
            error: None,
        });
        let spec = runner::Spec { program, args, env, cwd };
        std::thread::spawn(move || {
            let timed = process.clone();
            let timeout_send = send.clone();
            let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
            let event_send = Arc::new(move |event| {
                if matches!(event, JobEvent::Exit(_)) {
                    let _ = done_tx.send(());
                }
                send(Msg::Catalog(id, event));
            });
            if let Err(error) = runner::spawn_into(process, spec, event_send.clone()) {
                event_send(JobEvent::Exit(Err(error)));
                return;
            }
            if done_rx.recv_timeout(CATALOG_TIMEOUT).is_err() && !timed.is_cancelled() {
                timeout_send(Msg::Catalog(id, JobEvent::Line("Error: The catalog request timed out.".into())));
                timed.cancel();
            }
        });
    }

    pub fn retry_catalog(&mut self) {
        if let Some((request, cursor)) = self.catalog.last_failed.take() {
            self.start_catalog(request, cursor);
        }
    }

    pub fn prepare_catalog_tracks(&mut self) {
        let Some(result) = &self.catalog.result else { return };
        let existing: BTreeSet<String> = self.url.text.split_whitespace().map(str::to_owned).collect();
        let urls: Vec<_> = result
            .items
            .iter()
            .filter(|item| item.kind == Kind::Track && self.catalog.selected.contains(&item.url))
            .map(|item| item.url.as_str())
            .filter(|url| !existing.contains(*url))
            .collect();
        if !urls.is_empty() {
            self.batch_destination = None;
            let draft = self.url.text.trim();
            let selected = urls.join(" ");
            self.url = TextEdit::new(&if draft.is_empty() { selected } else { format!("{draft} {selected}") });
        }
        if !self.catalog.selected.is_empty() {
            self.catalog.selected.clear();
            self.page = Page::Downloads;
        }
    }

    pub fn request_catalog_artwork(&mut self, url: &str) {
        if self.link.is_none() || ARTWORK_CLIENT.is_none() || self.catalog.artwork.contains_key(url)
            || valid_url(url).is_none()
        {
            return;
        }
        self.catalog.artwork.insert(url.to_owned(), None);
        self.catalog.artwork_queue.push_back(url.to_owned());
        self.pump_catalog_artwork();
    }

    fn pump_catalog_artwork(&mut self) {
        let Some(client) = ARTWORK_CLIENT.as_ref() else { return };
        while self.catalog.artwork_loading < ARTWORK_WORKERS
            && let Some(url) = self.catalog.artwork_queue.pop_front()
        {
            self.catalog.artwork_loading += 1;
            let client = client.clone();
            self.background(move || {
                let bytes = library::fetch_image(&client, &url, ARTWORK_BYTES);
                Msg::CatalogArtwork(url, bytes)
            });
        }
    }

    pub fn catalog_artwork_loaded(&mut self, url: String, bytes: Option<Vec<u8>>) {
        self.catalog.artwork_loading = self.catalog.artwork_loading.saturating_sub(1);
        let image = bytes.and_then(|bytes| self.image_cache.as_ref().map(|cache| cache.load(bytes)));
        self.catalog.artwork.insert(url.clone(), image);
        self.catalog.artwork_cached.push_back(url);
        if self.catalog.artwork_cached.len() > ARTWORK_CACHE
            && let Some(url) = self.catalog.artwork_cached.pop_front()
        {
            self.catalog.artwork.remove(&url);
        }
        if self.page == Page::Catalog {
            self.pump_catalog_artwork();
        } else {
            self.catalog.clear_artwork_queue();
        }
    }
}
