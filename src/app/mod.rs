//! App state, messages from background threads, and user actions.
//! The download queue lives in `jobs`.

mod actions;
mod notifications;

pub use notifications::{Notification, NotificationKind, UndoAction};

use crate::{
    extensions,
    extension_updates::{self, CheckState, Versions},
    jobs::Job,
    library::{self, Entry},
    runner::JobEvent,
    services,
    store::{self, ExtensionStatus, Installed, Paths, Settings, Theme, Tools},
    updates,
};
use rune::{Image, ImageCache, TextEdit, Tx, Waker};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender, channel},
    },
    thread,
};

/// Key in `settings.preferred` of the resolver to use when several are installed.
pub const RESOLVER_KEY: &str = "resolver";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Downloads,
    Catalog,
    Library,
    Extensions,
    Settings,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum QueueFilter {
    #[default]
    All,
    Active,
    Choosing,
    Failed,
    Done,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum LibraryStatusFilter {
    #[default]
    All,
    HasLyrics,
    Missing,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum LibrarySort {
    #[default]
    Newest,
    Title,
    Artist,
}

impl LibrarySort {
    pub const ALL: [LibrarySort; 3] = [LibrarySort::Newest, LibrarySort::Title, LibrarySort::Artist];

    pub fn label(self) -> &'static str {
        match self {
            LibrarySort::Newest => "Newest",
            LibrarySort::Title => "Title",
            LibrarySort::Artist => "Artist",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfirmAction {
    CancelAll,
    RemoveExtension(String),
    RemoveAllExtensions,
}

/// Coalesces rapid background disk saves without losing the latest mutation.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum SaveState {
    #[default]
    Idle,
    Saving,
    Dirty,
}

pub enum Msg {
    Loaded { installed: Vec<Installed>, settings: Settings, history: Vec<Entry> },
    PackagesPicked(Vec<PathBuf>),
    /// A package file was installed (or refused).
    Imported(PathBuf, Result<Box<Installed>, String>),
    Removed(String, Result<(), String>),
    ExtensionsRemoved(Vec<(String, Result<(), String>)>),
    Tools(Tools),
    UpdateChecked(Option<updates::Release>),
    ManualUpdateChecked(Result<Option<updates::Release>, String>),
    ExtensionVersionsChecked(Result<Versions, String>),
    ExtensionStatus { id: String, check: u64, status: ExtensionStatus },
    SettingsSaved(Result<(), String>),
    HistorySaved(Result<(), String>),
    Job(u64, JobEvent),
    JobStaging(u64, tempfile::TempDir),
    Catalog(u64, JobEvent),
    CatalogArtwork(String, Option<Vec<u8>>),
    DownloadPlaced(u64, Result<Vec<String>, String>),
    JobValidated(u64, Result<Vec<String>, String>),
    TagsWritten(u64, Result<library::TaggedTrack, String>),
    /// Cover art for a job: the cached file and its bytes.
    JobCover(u64, PathBuf, Vec<u8>),
    /// Bytes of a cached cover, for the history (`None` when the file could not be read).
    Image(String, Option<Vec<u8>>),
    LibraryFiles(u64, BTreeMap<String, library::SidecarInfo>),
    TrackSpecs(String, Option<library::TrackSpecs>),
    LibraryExported(Result<(PathBuf, usize), String>),
    Error(String),
}

/// Channel from background threads to `poll`, and the waker that runs it.
pub struct Link {
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    waker: Waker,
}

impl Link {
    pub fn new(waker: Waker) -> Link {
        let (tx, rx) = channel();
        Link { tx, rx, waker }
    }

    pub fn sender(&self) -> impl Fn(Msg) + Send + Sync + Clone + 'static {
        let tx = Arc::new(Mutex::new(self.tx.clone()));
        let waker = self.waker.clone();
        move |msg| {
            if let Ok(tx) = tx.lock() {
                let _ = tx.send(msg);
            }
            waker();
        }
    }
}

pub struct App {
    pub paths: Paths,
    pub page: Page,
    pub url: TextEdit,
    /// The destination chosen for an exact Downloads draft.
    pub(crate) batch_destination: Option<(String, &'static services::Service)>,
    pub search: TextEdit,
    pub catalog: crate::catalog::Catalog,
    pub loaded: bool,
    pub settings: Settings,
    pub installed: Vec<Installed>,
    pub removing: BTreeSet<String>,
    pub jobs: Vec<Job>,
    pub next_job: u64,
    pub queue_paused: bool,
    pub queue_filter: QueueFilter,
    pub queue_menu_open: bool,
    pub history: Vec<Entry>,
    pub library_sort: LibrarySort,
    pub library_service: Option<String>,
    pub library_status_filter: LibraryStatusFilter,
    pub library_select_mode: bool,
    pub library_selected: BTreeSet<(u64, String)>,
    pub library_batch_lookup_open: bool,
    pub library_files: BTreeMap<String, library::SidecarInfo>,
    pub(crate) library_scan_serial: u64,
    pub lookup_picker: Option<(u64, String)>,
    pub track_inspector: Option<(u64, String)>,
    pub track_specs: BTreeMap<String, Option<library::TrackSpecs>>,
    pub extension_settings: Option<String>,
    pub extension_filter: Option<extensions::Kind>,
    pub(crate) settings_save: SaveState,
    pub(crate) history_save: SaveState,
    /// Decoded covers by cached file path.
    pub images: BTreeMap<String, Image>,
    pub(crate) image_order: VecDeque<String>,
    pub image_cache: Option<ImageCache>,
    pub tools: Option<Tools>,
    pub available_update: Option<updates::Release>,
    pub checking_app_update: bool,
    pub extension_versions: Versions,
    pub extension_update_check: CheckState,
    pub extension_update_error: Option<String>,
    pub extension_status: BTreeMap<String, ExtensionStatus>,
    /// Extensions whose status check is running.
    pub checking: BTreeMap<String, u64>,
    pub(crate) status_queue: VecDeque<String>,
    pub(crate) status_serial: u64,
    pub(crate) cover_loading: BTreeSet<String>,
    /// Flacora's own icon, shown in the sidebar.
    pub logo: Option<Image>,
    pub project_logos: Option<[Image; 2]>,
    pub provider_logos: BTreeMap<&'static str, Image>,
    pub notice: Option<Notification>,
    pub info: Option<Notification>,
    pub notice_hovered: bool,
    pub info_hovered: bool,
    pub undo: Option<UndoAction>,
    pub confirm_action: Option<ConfirmAction>,
    /// Package files being installed.
    pub importing: usize,
    pub(crate) window: Option<rune::WindowHandle>,
    pub(crate) browsing_packages: bool,
    pub(crate) pending_imports: VecDeque<PathBuf>,
    pub(crate) pending_checks: BTreeSet<String>,
    /// Files are being dragged over the window.
    pub drop_over: bool,
    pub(crate) waker: Option<Waker>,
    pub link: Option<Link>,
}

impl Default for App {
    fn default() -> Self {
        App {
            paths: Paths::detect(),
            page: Page::Downloads,
            url: TextEdit::default(),
            batch_destination: None,
            search: TextEdit::default(),
            catalog: crate::catalog::Catalog::default(),
            loaded: false,
            settings: Settings::default(),
            installed: Vec::new(),
            removing: BTreeSet::new(),
            jobs: Vec::new(),
            next_job: 1,
            queue_paused: false,
            queue_filter: QueueFilter::All,
            queue_menu_open: false,
            history: Vec::new(),
            library_sort: LibrarySort::Newest,
            library_service: None,
            library_status_filter: LibraryStatusFilter::All,
            library_select_mode: false,
            library_selected: BTreeSet::new(),
            library_batch_lookup_open: false,
            library_files: BTreeMap::new(),
            library_scan_serial: 0,
            lookup_picker: None,
            track_inspector: None,
            track_specs: BTreeMap::new(),
            extension_settings: None,
            extension_filter: None,
            settings_save: SaveState::Idle,
            history_save: SaveState::Idle,
            images: BTreeMap::new(),
            image_order: VecDeque::new(),
            image_cache: None,
            tools: None,
            available_update: None,
            checking_app_update: false,
            extension_versions: Versions::new(),
            extension_update_check: CheckState::Idle,
            extension_update_error: None,
            extension_status: BTreeMap::new(),
            checking: BTreeMap::new(),
            status_queue: VecDeque::new(),
            status_serial: 0,
            cover_loading: BTreeSet::new(),
            logo: None,
            project_logos: None,
            provider_logos: BTreeMap::new(),
            notice: None,
            info: None,
            notice_hovered: false,
            info_hovered: false,
            undo: None,
            confirm_action: None,
            importing: 0,
            window: None,
            browsing_packages: false,
            pending_imports: VecDeque::new(),
            pending_checks: BTreeSet::new(),
            drop_over: false,
            waker: None,
            link: None,
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if self.link.is_none() {
            return;
        }
        if self.settings_save != SaveState::Idle {
            let _ = store::save_settings(&self.paths, &self.settings);
        }
        if self.history_save != SaveState::Idle {
            let _ = library::save(&self.paths, &self.history);
        }
    }
}

impl App {
    /// Sets up image decoding for covers and embedded artwork.
    pub fn images(&mut self, waker: Waker) {
        let cache = ImageCache::new(waker);
        self.logo = Some(cache.load(crate::LOGO));
        self.project_logos = Some(crate::ui::project_logos(&cache));
        self.provider_logos = services::LOGOS.into_iter().map(|(service, bytes)| (service, cache.load(bytes))).collect();
        self.image_cache = Some(cache);
    }

    /// Connects background work to the window and starts loading.
    pub fn connect(&mut self, waker: Waker) {
        if self.image_cache.is_none() {
            self.images(waker.clone());
        }
        self.link = Some(Link::new(waker));
        let paths = self.paths.clone();
        self.background(move || {
            store::cleanup_stale_temps(&paths);
            Msg::Loaded {
                installed: store::load_installed(&paths),
                settings: store::load_settings(&paths),
                history: library::load(&paths),
            }
        });
        self.refresh_tools();
        self.background(|| Msg::UpdateChecked(updates::check()));
        self.check_extension_updates();
    }

    pub fn check_extension_updates(&mut self) {
        if self.extension_update_check == CheckState::Checking { return; }
        self.extension_update_check = CheckState::Checking;
        self.background(|| Msg::ExtensionVersionsChecked(extension_updates::check()));
    }

    pub fn background(&self, work: impl FnOnce() -> Msg + Send + 'static) {
        if let Some(link) = &self.link {
            let send = link.sender();
            thread::spawn(move || send(work()));
        }
    }

    pub fn background_opt(&self, work: impl FnOnce() -> Option<Msg> + Send + 'static) {
        if let Some(link) = &self.link {
            let send = link.sender();
            thread::spawn(move || {
                if let Some(msg) = work() {
                    send(msg);
                }
            });
        }
    }

    /// Applies every message that arrived; true when something changed.
    /// Startup hydration in the first second lands with `tx.instant()` so launch stays still.
    pub fn drain_with(&mut self, clock: f64, tx: &mut Tx) -> bool {
        let mut messages = Vec::new();
        if let Some(link) = &self.link {
            while let Ok(msg) = link.rx.try_recv() {
                messages.push(msg);
            }
        }
        let changed = !messages.is_empty();
        let mut startup_only = changed && clock < 1.0;
        for msg in messages {
            if !matches!(
                msg,
                Msg::Loaded { .. }
                    | Msg::Tools(_)
                    | Msg::UpdateChecked(_)
                    | Msg::ExtensionVersionsChecked(_)
                    | Msg::LibraryFiles(_, _)
                    | Msg::Image(_, _)
                    | Msg::ExtensionStatus { .. }
            ) {
                startup_only = false;
            }
            self.apply(msg);
        }
        if startup_only {
            tx.instant();
        }
        changed
    }

    pub fn apply(&mut self, msg: Msg) {
        match msg {
            Msg::Loaded { installed, settings, history } => {
                self.installed = installed;
                self.settings = settings;
                self.history = history;
                self.loaded = true;
                self.load_history_covers();
                self.refresh_library_files();
                if self.importing == 0 {
                    self.check_all();
                } else {
                    self.pending_checks.extend(self.installed.iter().map(|i| i.manifest.id.clone()));
                    self.start_import();
                }
            }
            Msg::PackagesPicked(mut files) => {
                self.browsing_packages = false;
                let count = files.len();
                files.retain(|file| file.extension().is_some_and(|ext| ext.eq_ignore_ascii_case(store::PACKAGE)));
                if files.len() != count {
                    self.warn(format!("Select only .{} extension packages.", store::PACKAGE));
                }
                if !files.is_empty() {
                    self.install_files(&files);
                }
            }
            Msg::Imported(file, result) => {
                self.importing = self.importing.saturating_sub(1);
                let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                match result {
                    Ok(installed) => {
                        self.success(format!("Installed {} {} from {name}", installed.manifest.name, installed.manifest.version));
                        self.add_installed(*installed);
                    }
                    Err(e) => self.error(format!("{name}: {e}")),
                }
                if self.importing == 0 {
                    for id in std::mem::take(&mut self.pending_checks) {
                        self.check_status(&id);
                    }
                    self.apply_waiting_batch_services();
                    self.pump();
                } else {
                    self.start_import();
                }
            }
            Msg::Removed(id, result) => {
                self.removing.remove(&id);
                match result {
                    Ok(()) => {
                        let name = self
                            .installed
                            .iter()
                            .find(|i| i.manifest.id == id)
                            .map(|i| i.manifest.name.clone())
                            .unwrap_or_else(|| id.clone());
                        self.forget_extension(&id);
                        self.success(format!("Removed {name}"));
                    }
                    Err(e) => self.error(e),
                }
            }
            Msg::ExtensionsRemoved(results) => {
                let total = results.len();
                let mut removed = 0;
                let mut first_error = None;
                for (id, result) in results {
                    self.removing.remove(&id);
                    match result {
                        Ok(()) => {
                            self.forget_extension(&id);
                            removed += 1;
                        }
                        Err(error) => { first_error.get_or_insert(error); }
                    }
                }
                if let Some(error) = first_error {
                    self.error(format!("Removed {removed} of {total} extensions. {error}"));
                } else {
                    self.success(format!("Removed {removed} extension{}", if removed == 1 { "" } else { "s" }));
                }
            }
            Msg::Tools(tools) => self.tools = Some(tools),
            Msg::UpdateChecked(release) => self.available_update = release,
            Msg::ManualUpdateChecked(result) => {
                self.checking_app_update = false;
                match result {
                    Ok(Some(release)) => {
                        self.success(format!("Flacora v{} is available.", release.version));
                        self.available_update = Some(release);
                    }
                    Ok(None) => {
                        self.available_update = None;
                        self.success(format!("Flacora v{} is up to date.", updates::CURRENT_VERSION));
                    }
                    Err(error) => self.warn(error),
                }
            }
            Msg::ExtensionVersionsChecked(result) => match result {
                Ok(versions) => {
                    self.extension_versions = versions;
                    self.extension_update_check = CheckState::Ready;
                    self.extension_update_error = None;
                }
                Err(error) => {
                    self.extension_update_check = CheckState::Failed;
                    self.extension_update_error = Some(error);
                }
            },
            Msg::ExtensionStatus { id, check, status } => {
                if self.checking.get(&id) == Some(&check) {
                    self.checking.remove(&id);
                    self.extension_status.insert(id, status);
                    self.pump_status_checks();
                    self.pump();
                }
            }
            Msg::SettingsSaved(result) => {
                if let Err(error) = result {
                    self.error(format!("Could not save settings: {error}"));
                }
                if self.settings_save.finish() {
                    self.save_settings();
                }
            }
            Msg::HistorySaved(result) => {
                if let Err(error) = result {
                    self.error(format!("Could not save the history: {error}"));
                }
                if self.history_save.finish() {
                    self.save_history();
                }
            }
            Msg::Job(id, event) => self.job_event(id, event),
            Msg::JobStaging(id, staging) => {
                if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id && j.is_running()) {
                    job.staging = Some(staging);
                }
            }
            Msg::Catalog(id, event) => self.catalog.event(id, event),
            Msg::CatalogArtwork(url, bytes) => self.catalog_artwork_loaded(url, bytes),
            Msg::DownloadPlaced(id, result) => self.download_placed(id, result),
            Msg::JobValidated(id, result) => self.job_validated(id, result),
            Msg::TagsWritten(id, result) => self.tags_written(id, result),
            Msg::JobCover(id, path, bytes) => self.job_cover(id, path, bytes),
            Msg::Image(path, bytes) => {
                self.cover_loading.remove(&path);
                if let Some(bytes) = bytes {
                    self.load_image(path, bytes);
                }
            }
            Msg::LibraryFiles(serial, files) => {
                if serial == self.library_scan_serial {
                    self.library_files = files;
                }
            }
            Msg::TrackSpecs(path, specs) => {
                self.track_specs.insert(path, specs);
            }
            Msg::LibraryExported(result) => match result {
                Ok((path, count)) => {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string());
                    self.success(format!(
                        "Exported {count} track{} to {name}",
                        if count == 1 { "" } else { "s" }
                    ));
                }
                Err(error) => self.error(format!("Could not export library: {error}")),
            },
            Msg::Error(e) if e.is_empty() => {}
            Msg::Error(e) => self.error(e),
        }
    }

    fn add_installed(&mut self, installed: Installed) {
        let id = installed.manifest.id.clone();
        let alias = extensions::legacy_web_id(&id);
        self.installed.retain(|i| i.manifest.id != id && alias != Some(i.manifest.id.as_str()));
        if alias.is_some() && self.catalog.provider.as_deref() == alias {
            self.catalog.select_provider(id.clone());
        }
        self.installed.push(installed);
        self.installed.sort_by_key(|i| i.manifest.name.to_lowercase());
        if let Some(alias) = alias {
            self.forget_status(alias);
        }
        self.forget_status(&id);
        self.pending_checks.insert(id);
    }

    fn forget_extension(&mut self, id: &str) {
        self.installed.retain(|i| i.manifest.id != id);
        self.forget_status(id);
        self.catalog.clear_for_removed(id);
        if self.extension_settings.as_deref() == Some(id) {
            self.extension_settings = None;
        }
        if let Some(kind) = self.extension_filter
            && !self.installed.iter().any(|i| i.manifest.kind == kind)
        {
            self.extension_filter = None;
        }
    }

    fn forget_status(&mut self, id: &str) {
        self.checking.remove(id);
        self.extension_status.remove(id);
        self.pending_checks.remove(id);
        self.status_queue.retain(|queued| queued != id);
    }

    /// Refreshes the online status of every installed extension.
    pub fn check_all(&mut self) {
        if self.importing > 0 {
            return;
        }
        let ids: Vec<_> = self.installed.iter().map(|i| i.manifest.id.clone()).collect();
        for id in ids {
            self.check_status(&id);
        }
    }

    /// Refreshes the online status of one installed extension.
    pub fn check_extension(&mut self, id: &str) {
        if self.importing > 0 {
            return;
        }
        self.check_status(id);
    }

    fn check_status(&mut self, id: &str) {
        if self.link.is_none()
            || self.checking.contains_key(id)
            || self.removing.contains(id)
            || self.status_queue.iter().any(|queued| queued == id)
        {
            return;
        }
        if !self.installed.iter().any(|i| i.manifest.id == id) {
            return;
        }
        self.status_queue.push_back(id.to_owned());
        self.pump_status_checks();
    }

    pub fn pump_status_checks(&mut self) {
        const MAX_CONCURRENT_CHECKS: usize = 4;
        if self.link.is_none() || self.importing > 0 {
            return;
        }
        let mut browser_busy = self.jobs.iter().any(|j| j.is_running() && j.extension.manifest.browser)
            || self.installed.iter().any(|i| {
                i.manifest.browser && !i.manifest.status.is_empty() && self.checking.contains_key(&i.manifest.id)
            });
        let mut scanned = 0;
        let total = self.status_queue.len();
        while self.checking.len() < MAX_CONCURRENT_CHECKS && scanned < total {
            let Some(id) = self.status_queue.pop_front() else { break };
            scanned += 1;
            if self.checking.contains_key(&id) || self.removing.contains(&id) {
                continue;
            }
            let Some(installed) = self.installed.iter().find(|i| i.manifest.id == id).cloned() else {
                continue;
            };
            let needs_browser = installed.manifest.browser && !installed.manifest.status.is_empty();
            if needs_browser && browser_busy {
                self.status_queue.push_back(id);
                continue;
            }
            if needs_browser {
                browser_busy = true;
            }
            self.status_serial += 1;
            let check = self.status_serial;
            self.checking.insert(id.clone(), check);
            let paths = self.paths.clone();
            self.background(move || Msg::ExtensionStatus {
                id,
                check,
                status: store::check_status(&paths, &installed),
            });
        }
    }

    pub fn is_checking(&self, id: &str) -> bool {
        self.checking.contains_key(id) || self.status_queue.iter().any(|queued| queued == id)
    }

    pub fn any_checking(&self) -> bool {
        !self.checking.is_empty() || !self.status_queue.is_empty()
    }

    pub fn status_command_running(&self, installed: &Installed) -> bool {
        !installed.manifest.status.is_empty() && self.is_checking(&installed.manifest.id)
    }

    /// The services to show for an extension: what works now when it checked, else what it declares.
    pub fn services_of<'a>(&'a self, id: &str, declared: &'a [String]) -> &'a [String] {
        match self.extension_status.get(id) {
            Some(ExtensionStatus::Services(services)) => services,
            _ => declared,
        }
    }

    pub const IMAGE_CACHE_LIMIT: usize = 192;

    pub fn load_image(&mut self, path: String, bytes: Vec<u8>) {
        let Some(cache) = &self.image_cache else { return };
        if self.images.contains_key(&path) {
            return;
        }
        self.images.insert(path.clone(), cache.load(bytes));
        self.image_order.push_back(path);
        self.evict_images();
    }

    fn evict_images(&mut self) {
        let mut attempts = self.image_order.len();
        while self.images.len() > Self::IMAGE_CACHE_LIMIT && attempts > 0 {
            attempts -= 1;
            let Some(oldest) = self.image_order.pop_front() else { break };
            if self.jobs.iter().any(|job| job.cover.as_deref() == Some(oldest.as_str())) {
                self.image_order.push_back(oldest);
                continue;
            }
            self.images.remove(&oldest);
        }
    }

    pub fn image(&self, path: Option<&str>) -> Option<&Image> {
        self.images.get(path?)
    }

    pub(crate) fn load_history_covers(&mut self) {
        const INITIAL_COVERS: usize = 64;
        let Some(link) = &self.link else { return };
        let mut seen = BTreeSet::new();
        let covers: Vec<String> = self
            .history
            .iter()
            .filter_map(|e| e.cover.as_ref())
            .filter(|path| !self.images.contains_key(*path) && seen.insert((*path).clone()))
            .take(INITIAL_COVERS)
            .cloned()
            .collect();
        if covers.is_empty() {
            return;
        }
        for path in &covers {
            self.cover_loading.insert(path.clone());
        }
        let send = link.sender();
        let paths = self.paths.clone();
        thread::spawn(move || {
            for path in covers {
                let bytes = library::read_cover(&paths, &path);
                send(Msg::Image(path, bytes));
            }
        });
    }

    pub fn request_history_cover(&mut self, path: &str) {
        if self.link.is_none() || self.images.contains_key(path) || !self.cover_loading.insert(path.to_owned()) {
            return;
        }
        let paths = self.paths.clone();
        let path = path.to_owned();
        self.background(move || {
            let bytes = library::read_cover(&paths, &path);
            Msg::Image(path, bytes)
        });
    }

    pub fn dark(&self, system_dark: bool) -> bool {
        match self.settings.theme {
            Theme::System => system_dark,
            Theme::Light => false,
            Theme::Dark => true,
        }
    }
}

#[cfg(test)]
mod tests;
