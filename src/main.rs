//! Flacora: a Rune GUI for music downloaders that install as extensions.
//! `cargo run` opens the window; `-- --snap out.png --scene queue` renders a scene headless
//! (`--list-scenes` lists them). Headless runs show fixed data; set
//! `FLACORA_HEADLESS_LIVE=1` to load locally installed extensions instead.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod catalog;
mod extensions;
mod extension_updates;
mod fixtures;
mod astryx;
use astryx::icons;
mod jobs;
mod library;
mod output;
mod package_picker;
mod protocol;
mod runner;
mod services;
mod store;
mod track_links;
mod ui;
mod updates;
#[cfg(windows)]
mod window;

use app::App;
use rune::{AppSpec, Chord, ChordKey, ChordMod};

const MIN_WINDOW_SIZE: (f32, f32) = ui::MIN_WINDOW_SIZE;

/// Flacora's icon (1024 px PNG): the window and taskbar icon and the sidebar logo.
pub const LOGO: &[u8] = include_bytes!("../assets/appicon.png");

fn setup(s: &mut App, waker: rune::Waker) {
    s.images(waker.clone());
    s.waker = Some(waker.clone());
    let headless = rune::tooling::headless_run().is_some();
    if headless {
        // Decode embedded artwork before the first snapshot.
        for logo in s.logo.iter().chain(s.project_logos.iter().flatten()).chain(s.provider_logos.values()) {
            let _ = logo.wait();
        }
        if std::env::var("FLACORA_HEADLESS_LIVE").is_ok_and(|v| v == "1") {
            s.connect(waker);
        }
    }
}

fn poll(s: &mut App, clock: f64, tx: &mut rune::Tx) -> bool {
    let received = s.drain_with(clock, tx);
    s.tick_notifications(clock, tx) || received
}

pub(crate) fn on_key(s: &mut App, ev: &rune::KeyEvent, tx: &mut rune::Tx) -> bool {
    use app::Page;
    const MOD_1: Chord = Chord::new(ChordKey::Char('1')).with(ChordMod::Primary);
    const MOD_2: Chord = Chord::new(ChordKey::Char('2')).with(ChordMod::Primary);
    const MOD_3: Chord = Chord::new(ChordKey::Char('3')).with(ChordMod::Primary);
    const MOD_4: Chord = Chord::new(ChordKey::Char('4')).with(ChordMod::Primary);
    const MOD_5: Chord = Chord::new(ChordKey::Char('5')).with(ChordMod::Primary);
    const MOD_L: Chord = Chord::new(ChordKey::Char('l')).with(ChordMod::Primary);
    const MOD_F: Chord = Chord::new(ChordKey::Char('f')).with(ChordMod::Primary);
    const MOD_Z: Chord = Chord::new(ChordKey::Char('z')).with(ChordMod::Primary);
    const ALT_LEFT: Chord = Chord::new(ChordKey::Left).with(ChordMod::Alt);
    const ESC: Chord = Chord::new(ChordKey::Escape);

    if MOD_1.matches_tx(ev, tx) {
        s.page = Page::Downloads;
        s.dismiss_page_overlays();
        true
    } else if MOD_2.matches_tx(ev, tx) {
        s.page = Page::Catalog;
        s.dismiss_page_overlays();
        true
    } else if MOD_3.matches_tx(ev, tx) {
        s.page = Page::Library;
        s.dismiss_page_overlays();
        s.refresh_library_files();
        true
    } else if MOD_4.matches_tx(ev, tx) {
        s.page = Page::Extensions;
        s.dismiss_page_overlays();
        true
    } else if MOD_5.matches_tx(ev, tx) {
        s.page = Page::Settings;
        s.dismiss_page_overlays();
        true
    } else if MOD_L.matches_tx(ev, tx) {
        s.page = Page::Downloads;
        s.dismiss_page_overlays();
        tx.focus_when_shown("url", false);
        true
    } else if MOD_F.matches_tx(ev, tx) {
        s.dismiss_page_overlays();
        if s.page == Page::Library && !s.history.is_empty() {
            tx.focus_when_shown("search", false);
        } else if s.catalog_providers().next().is_some() {
            s.page = Page::Catalog;
            tx.focus_when_shown("catalog-input", false);
        } else if !s.history.is_empty() {
            s.page = Page::Library;
            s.refresh_library_files();
            tx.focus_when_shown("search", false);
        } else {
            s.page = Page::Downloads;
            tx.focus_when_shown("url", false);
        }
        true
    } else if MOD_Z.matches_tx(ev, tx)
        && s.undo.is_some()
        && !s.url.focused
        && !s.search.focused
        && !s.catalog.input.focused
    {
        s.undo_last();
        true
    } else if ALT_LEFT.matches_tx(ev, tx) && s.page == Page::Catalog && s.catalog.can_go_back() {
        s.catalog.back();
        true
    } else if ESC.matches_tx(ev, tx) {
        if std::mem::take(&mut s.queue_menu_open) || s.confirm_action.take().is_some() {
            return true;
        }
        if let Some((date, file)) = s.track_inspector.take() {
            if s.page == Page::Library
                && let Some(index) = s.history.iter().position(|entry| {
                    entry.date == date && entry.main_file() == Some(file.as_str())
                })
            {
                tx.focus_when_shown(format!("inspect-entry-{index}"), false);
            }
            return true;
        }
        if let Some((date, file)) = s.lookup_picker.take() {
            if s.page == Page::Library
                && let Some(index) = s.history.iter().position(|entry| {
                    entry.date == date && entry.flac_file() == Some(file.as_str())
                })
            {
                tx.focus_when_shown(format!("lookup-{index}"), false);
            }
            return true;
        }
        if s.extension_settings.take().is_some() {
            return true;
        }
        if s.page == Page::Library && std::mem::take(&mut s.library_batch_lookup_open) {
            return true;
        }
        if s.page == Page::Library && s.library_select_mode {
            s.library_select_mode = false;
            s.library_selected.clear();
            return true;
        }
        if s.page == Page::Catalog && !s.catalog.selected.is_empty() {
            s.catalog.selected.clear();
            return true;
        }
        false
    } else {
        false
    }
}

fn main() {
    rune::prewarm_gpu();
    let _ = rune::set_app_icon(LOGO);
    let app = rune::App::new(AppSpec::<App> {
        min_size: Some(MIN_WINDOW_SIZE),
        fonts: ui::FONTS,
        families: ("Figtree", "ui-monospace"),
        setup: Some(setup),
        poll: Some(poll),
        on_key: Some(on_key),
        scenes: &[
            ("home", fixtures::home),
            ("downloads-batch-input", fixtures::downloads_batch_input),
            ("queue", fixtures::queue),
            ("queue-actions", fixtures::queue_actions),
            ("queue-cancel-confirmation", fixtures::queue_cancel_confirmation),
            ("queue-indeterminate", fixtures::queue_indeterminate),
            ("choose", fixtures::choose),
            ("resolvers", fixtures::resolvers),
            ("light", fixtures::light),
            ("missing", fixtures::missing),
            ("spotify", fixtures::spotify),
            ("fresh", fixtures::fresh),
            ("library", fixtures::library),
            ("library-search", fixtures::library_search),
            ("library-sources", fixtures::library_sources),
            ("library-single", fixtures::library_single),
            ("library-overflow", fixtures::library_overflow),
            ("library-batch", fixtures::library_batch),
            ("lookup-picker", fixtures::lookup_picker),
            ("library-outdated-metadata", fixtures::library_outdated_metadata),
            ("catalog", fixtures::catalog_results),
            ("catalog-feed", fixtures::catalog_feed),
            ("catalog-feed-hidden", fixtures::catalog_feed_hidden),
            ("catalog-idle", fixtures::catalog_idle),
            ("catalog-resolver-sources", fixtures::catalog_with_resolver_sources),
            ("catalog-batch", fixtures::catalog_batch),
            ("catalog-empty", fixtures::catalog_empty),
            ("catalog-link", fixtures::catalog_link),
            ("catalog-overflow", fixtures::catalog_overflow),
            ("catalog-missing-extension", fixtures::catalog_missing_extension),
            ("lookup-queue", fixtures::lookup_queue),
            ("extensions", fixtures::extensions_page),
            ("extension-badges", fixtures::extension_badges),
            ("extension-status", fixtures::extension_status),
            ("extension-options", fixtures::extension_options),
            ("extension-updates", fixtures::extension_updates_available),
            ("extension-updates-light", fixtures::extension_updates_light),
            ("extension-updates-current", fixtures::extension_updates_current),
            ("extension-updates-failed", fixtures::extension_updates_failed),
            ("extension-updates-checking", fixtures::extension_updates_checking),
            ("extensions-empty", fixtures::extensions_empty),
            ("extensions-remove-confirmation", fixtures::extensions_remove_confirmation),
            ("extensions-drop", fixtures::extensions_dropping),
            ("extensions-installing", fixtures::extensions_installing),
            ("imported", fixtures::imported),
            ("notifications", fixtures::notifications),
            ("settings", fixtures::settings),
            ("settings-matcha", fixtures::settings_matcha),
            ("settings-update-available", fixtures::settings_update_available),
            ("output-settings", fixtures::output_settings),
            ("settings-auto-tag-available", fixtures::auto_tag_available_settings),
            ("settings-auto-tag", fixtures::auto_tag_settings),
            ("settings-without-ffmpeg", fixtures::settings_without_ffmpeg),
            ("settings-without-browser", fixtures::settings_without_browser),
            ("drop", fixtures::dropping),
            ("loading", fixtures::loading),
        ],
        ..AppSpec::new("Flacora", MIN_WINDOW_SIZE, App::default, ui::view)
    });
    let app_handle = app.handle();
    let ready_handle = app.handle();
    let app = app
        .on_window(move |handle| {
            #[cfg(windows)]
            window::center(handle);
            let handle = handle.clone();
            app_handle.update_if(move |s, _| {
                s.window = Some(handle);
                false
            });
        })
        .on_ready(move |_| {
            ready_handle.update_if(|s, _| {
                if s.link.is_none()
                    && let Some(waker) = s.waker.clone()
                {
                    s.connect(waker);
                }
                false
            });
        });
    let args = ["--scroll-bars", "always"].map(str::to_owned).into_iter().chain(std::env::args().skip(1));
    app.args(args).run();
}
