mod extensions;
mod library_catalog;
mod settings;

use super::*;
use crate::{fixtures, jobs::Status, store::ThemePreset};
use rune::{Env, Runtime};
use std::path::PathBuf;

fn runtime(setup: fn(&mut App)) -> Runtime<App> {
    runtime_at(setup, (1080.0, 720.0))
}

fn runtime_at(setup: fn(&mut App), size: (f32, f32)) -> Runtime<App> {
    let mut state = App::default();
    setup(&mut state);
    let env = Env { system_dark: true, scale: 1.0 };
    let mut rt = Runtime::new(state, view, size, env);
    rt.frame(0.0);
    rt
}

fn tap(rt: &mut Runtime<App>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("{key} is on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..3 {
        rt.frame(1.0 / 60.0);
    }
}

#[test]
fn the_sidebar_switches_pages() {
    let mut rt = runtime(fixtures::home);
    for (key, page, marker) in [
        ("nav-catalog", Page::Catalog, "catalog-empty"),
        ("nav-library", Page::Library, "library-list"),
        ("nav-extensions", Page::Extensions, "install-extension"),
        ("nav-settings", Page::Settings, "setting-folder"),
        ("nav-downloads", Page::Downloads, "composer"),
    ] {
        tap(&mut rt, key);
        assert_eq!(rt.state.page, page);
        assert!(rt.find(marker).is_some(), "{marker} shows on {page:?}");
    }
    tap(&mut rt, "destination");
    assert_eq!(rt.state.page, Page::Settings);
}

#[test]
fn the_sidebar_counts_installed_extensions_after_loading() {
    use crate::app::Msg;

    let loading = runtime(fixtures::loading);
    assert!(loading.find("nav-extensions/badge").is_none());

    let mut rt = runtime(fixtures::fresh);
    tap(&mut rt, "nav-extensions");
    assert_eq!(rt.state.page, Page::Extensions);
    let check_count = |rt: &Runtime<App>, expected: &str| {
        let elements = rt.elements();
        assert!(elements.iter().any(|el| el.path.contains("/nav-extensions/badge/")
            && el.text.as_deref() == Some(expected)));
        let label = format!("Extensions, {expected} installed");
        assert!(elements.iter().any(|el| el.key.as_deref() == Some("nav-extensions")
            && el.label.as_deref() == Some(&label)));
    };
    check_count(&rt, "0");

    rt.state.apply(Msg::Imported(PathBuf::from("odesli.fext"), Ok(Box::new(fixtures::installed("odesli")))));
    rt.invalidate();
    rt.frame(0.0);
    check_count(&rt, "1");

    rt.state.apply(Msg::Removed("odesli".into(), Ok(())));
    rt.invalidate();
    rt.frame(0.0);
    check_count(&rt, "0");
}

#[test]
fn pages_stay_centered_and_bounded_when_the_window_resizes() {
    let minimum = crate::MIN_WINDOW_SIZE;
    let mut rt = runtime_at(fixtures::queue, minimum);
    for size in [minimum, (1280.0, 720.0), (1920.0, 1080.0), (2560.0, 1440.0), minimum] {
        rt.set_viewport(size, 1.0);
        rt.frame(0.0);
        rt.frame(0.6);
        let ([pane_x, _, pane_width, _], _) = rt.presented("content").unwrap();
        let expected = pane_width.min(theme::PAGE_CONTENT_MAX_WIDTH + 2.0 * theme::PAGE_INSET);
        for (navigation, page, component) in [
            ("nav-downloads", "downloads-page", "composer"),
            ("nav-catalog", "catalog-page", "catalog-empty"),
            ("nav-library", "library-page", "library-list"),
            ("nav-extensions", "extensions-page", "extension-filters"),
            ("nav-settings", "settings-page", "group-downloads"),
        ] {
            tap(&mut rt, navigation);
            rt.frame(0.6);
            let ([x, _, width, height], _) = rt.presented(page).unwrap();
            let ([component_x, _, component_width, _], _) = rt.presented(component).unwrap();
            assert!((width - expected).abs() < 1.0, "{page} at {size:?}: {width}");
            assert!((x + width / 2.0 - pane_x - pane_width / 2.0).abs() < 1.0, "{page} is centered");
            assert!((height - size.1).abs() < 1.0, "{page} uses the available height");
            assert!((component_x - x - theme::PAGE_INSET).abs() < 1.0, "{component} horizontal inset");
            assert!((component_width - (expected - 2.0 * theme::PAGE_INSET)).abs() < 1.0, "{component} width");
        }
    }
}

#[test]
fn sidebar_footer_links_open_support_and_project_pages() {
    let mut rt = runtime_at(fixtures::home, (1280.0, 720.0));
    assert!(rt.find("sidebar-update").is_none());
    let footer_y = |key| rt.presented(key).unwrap().0[1];
    assert!(footer_y("buy-me-a-coffee") < footer_y("project-spotbye"));
    assert!(footer_y("buy-me-a-coffee") < footer_y("project-stream-hoarder"));
    let ([x, _, width, height], _) = rt.presented("buy-me-a-coffee").unwrap();
    let ([settings_x, _, settings_width, settings_height], _) = rt.presented("nav-settings").unwrap();
    assert_eq!((x, width, height), (settings_x, settings_width, settings_height));
    let status_left = |key| rt.presented(key).unwrap().0[0];
    assert_eq!(status_left("project-spotbye-status"), status_left("project-stream-hoarder-status"));
    for (key, url) in [
        ("buy-me-a-coffee", "https://afkarxyz.gumroad.com/coffee"),
        ("project-spotbye", "https://spotbye.vip/"),
        ("project-spotbye-status", "https://status.spotbye.vip/"),
        ("project-stream-hoarder", "https://streamhoarder.app/"),
        ("project-stream-hoarder-status", "https://status.streamhoarder.app/"),
    ] {
        tap(&mut rt, key);
        assert_eq!(rt.take_url_requests(), [url]);
        assert_eq!(rt.state.page, Page::Downloads);
    }
}

#[test]
fn the_sidebar_update_badge_sits_above_coffee_and_opens_github_release() {
    let mut rt = runtime(|s| {
        fixtures::settings_update_available(s);
        s.page = Page::Downloads;
    });
    let (badge, _) = rt.presented("sidebar-update").unwrap();
    let (coffee, _) = rt.presented("buy-me-a-coffee").unwrap();
    assert!(badge[1] + badge[3] <= coffee[1]);
    let release = rt.state.available_update.as_ref().unwrap();
    let expected = format!("https://github.com/spotbye/Flacora/releases/tag/v{}", release.version);
    tap(&mut rt, "sidebar-update");
    assert_eq!(rt.take_url_requests(), [expected]);
    assert_eq!(rt.state.page, Page::Downloads);

    rt.state.apply(crate::app::Msg::UpdateChecked(None));
    rt.invalidate();
    rt.frame(0.0);
    assert!(rt.find("sidebar-update").is_none());
}

#[test]
fn extension_counts_and_ffmpeg_details_stay_on_their_pages() {
    let mut rt = runtime(fixtures::home);
    assert!(!rt.elements().iter().any(|el| {
        el.path.contains("/sidebar-footer/") && el.text.as_deref().is_some_and(|text| {
            text.contains("FFmpeg") || text.ends_with(" Extensions")
        })
    }));

    tap(&mut rt, "nav-extensions");
    assert!(rt.find("extension-category-downloaders").is_some());
    tap(&mut rt, "nav-settings");
    assert!(rt.find("setting-ffmpeg").is_some());
}

#[test]
fn notifications_in_the_toast_region_can_be_dismissed_independently() {
    let mut rt = runtime_at(fixtures::notifications, (1280.0, 720.0));
    assert!(rt.find("toast-region").is_some());
    // Toasts stay clear of right-aligned card action columns.
    let ([toast_x, _, toast_w, _], _) = rt.presented("info").unwrap();
    let ([status_x, _, _, _], _) = rt.presented("online-status-lucida").unwrap();
    assert!(toast_x + toast_w < status_x, "toast ({}) should stay clear of card actions ({status_x})", toast_x + toast_w);
    tap(&mut rt, "info/top/close");
    assert!(rt.state.info.is_none());
    assert!(rt.state.notice.is_some());
    tap(&mut rt, "notice/top/close");
    assert!(rt.state.notice.is_none());
}

#[test]
fn status_toasts_use_their_theme_colors_and_expire_independently() {
    let mut rt = runtime(fixtures::notifications);
    rt.poll = Some(crate::poll);
    rt.frame(1.0 / 60.0);
    let k = theme::kit(ThemePreset::Neutral, true);
    let elements = rt.elements();
    for (key, label, fill) in [("info", "Success", k.c.success.bg), ("notice", "Error", k.c.danger.bg)] {
        let toast = elements.iter().find(|el| el.key.as_deref() == Some(key)).unwrap();
        assert_eq!(toast.label.as_deref(), Some(label));
        assert_eq!(toast.fill, fill);
    }

    rt.frame(2.4);
    assert!(rt.find("info").is_some());
    let before_exit = rt.presented("info").unwrap().0;
    rt.frame(0.2);
    assert!(rt.find("info").is_none());
    assert!(rt.find("notice").is_some());
    let exiting = rt.elements().into_iter().find(|el| el.key.as_deref() == Some("info")).unwrap();
    assert!(exiting.exiting && exiting.opacity > 0.0);
    assert_eq!(&exiting.rect[..2], &before_exit[..2]);
    rt.frame(0.08);
    let fading = rt.elements().into_iter().find(|el| el.key.as_deref() == Some("info")).unwrap();
    assert!(fading.opacity < exiting.opacity);

    rt.frame(1.25);
    assert!(rt.find("notice").is_some());
    rt.frame(0.1);
    assert!(rt.find("notice").is_none());
    assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("notice") && el.exiting));
    rt.frame(0.5);
    assert!(rt.elements().iter().all(|el| el.key.as_deref() != Some("notice")));
}

#[test]
fn hovering_a_toast_pauses_auto_dismiss_until_pointer_leaves() {
    let mut rt = runtime(fixtures::notifications);
    rt.poll = Some(crate::poll);
    rt.frame(1.0 / 60.0);

    let (x, y) = rt.find("info").unwrap();
    rt.pointer_move(x, y);
    rt.frame(1.0 / 60.0);
    assert!(rt.state.info_hovered);

    // Waiting well past both lifetimes keeps both toasts alive while hovered.
    rt.frame(5.0);
    assert!(rt.find("info").is_some());
    assert!(rt.find("notice").is_some());

    // Moving straight onto the second toast keeps hover active without clobbering.
    let (nx, ny) = rt.find("notice").unwrap();
    rt.pointer_move(nx, ny);
    rt.frame(1.0 / 60.0);
    assert!(rt.state.notice_hovered && !rt.state.info_hovered);
    rt.frame(5.0);
    assert!(rt.find("info").is_some());
    assert!(rt.find("notice").is_some());

    // Leaving resumes the timer and gives at least 1s before dismissal.
    rt.pointer_move(10.0, 10.0);
    rt.frame(1.0 / 60.0);
    assert!(!rt.state.info_hovered && !rt.state.notice_hovered);
    rt.frame(0.5);
    assert!(rt.find("info").is_some());
    rt.frame(3.0);
    assert!(rt.find("info").is_none());
}

#[test]
fn a_link_without_an_installed_extension_opens_the_install_controls() {
    let mut rt = runtime(fixtures::missing);
    assert!(rt.find("install-odesli").is_none());
    tap(&mut rt, "open-extensions");
    assert_eq!(rt.state.page, Page::Extensions);
    assert!(rt.find("install-extension").is_some());
}

#[test]
fn a_spotify_link_goes_through_the_resolver() {
    let mut rt = runtime(fixtures::spotify);
    assert!(rt.find("resolver").is_some(), "the route names Odesli");
    tap(&mut rt, "download");
    assert_eq!(rt.state.jobs.last().map(|j| j.extension.manifest.id.as_str()), Some("odesli"));
}

#[test]
fn with_two_resolvers_the_composer_lets_you_pick_one() {
    let mut rt = runtime(fixtures::resolvers);
    assert!(rt.find("resolver-pick").is_some() && rt.find("resolver").is_none());
    tap(&mut rt, "resolver-pick/segment1");
    assert_eq!(rt.state.settings.preferred.get(crate::app::RESOLVER_KEY).map(String::as_str), Some("squigly"));
    tap(&mut rt, "download");
    assert_eq!(rt.state.jobs.last().map(|j| j.extension.manifest.id.as_str()), Some("squigly"));
}

#[test]
fn picking_an_extension_is_remembered_for_the_service() {
    let mut rt = runtime(fixtures::queue);
    assert_eq!(rt.state.selected().unwrap().manifest.id, "flacdownloader");
    assert!(rt.find("option-quality").is_none(), "FlacDownloader has no quality option");
    tap(&mut rt, "extension-pick/segment1");
    assert_eq!(rt.state.settings.preferred.get("tidal").map(String::as_str), Some("odskyler"));
    rt.frame(1.0 / 60.0);
    assert!(rt.find("option-quality").is_some(), "Rhythmax shows its quality option");
}

#[test]
fn queue_cards_offer_the_actions_their_state_allows() {
    let mut rt = runtime(fixtures::queue);
    assert!(rt.find("cancel-1").is_some() && rt.find("progress-1").is_some(), "running");
    assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("progress-1") && el.label.as_deref() == Some("Downloading Audio")));
    assert!(rt.find("cancel-2").is_some() && rt.find("cancel-3").is_some(), "queued");
    assert!(rt.find("play-4").is_some() && rt.find("reveal-4").is_some(), "done");
    assert!(rt.find("retry-5").is_some() && rt.find("log-view-5").is_some(), "failed, details open");
    tap(&mut rt, "cancel-2");
    assert_eq!(rt.state.jobs[1].status, Status::Cancelled);
    tap(&mut rt, "queue-more");
    tap(&mut rt, "queue-actions-item1");
    let left: Vec<u64> = rt.state.jobs.iter().map(|j| j.id).collect();
    assert_eq!(left, [1, 3], "the running and the waiting job stay");
}

#[test]
fn the_download_error_banner_retries_the_failed_job() {
    let mut rt = runtime_at(fixtures::queue, (1080.0, 1024.0));
    let elements = rt.elements();
    assert!(elements.iter().any(|el| el.key.as_deref() == Some("job-error-5") && el.role == "alert"));
    assert!(elements.iter().any(|el| el.path.contains("/job-error-5/") && el.text.as_deref() == Some("Download Failed")));
    tap(&mut rt, "retry-5");
    assert_eq!(rt.state.jobs.iter().find(|job| job.id == 5).unwrap().status, Status::Queued);
    assert!(rt.find("job-error-5").is_none());
}

#[test]
fn dropping_a_package_anywhere_installs_it_and_other_files_are_refused() {
    let mut rt = runtime(fixtures::queue);
    let package = PathBuf::from("C:/Downloads/lucida-0.2.0-windows-x64.flacora");
    rt.drag_files(vec![package.clone()], Some((600.0, 400.0)));
    rt.frame(1.0 / 60.0);
    assert!(rt.state.drop_over && rt.find("drop-overlay").is_some());
    rt.drop_files(vec![package], Some((600.0, 400.0)));
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.importing, 1);
    assert!(!rt.state.drop_over);

    rt.drop_files(vec![PathBuf::from("C:/Music/Golden.flac")], Some((600.0, 400.0)));
    rt.frame(1.0 / 60.0);
    let notice = &rt.state.notice.as_ref().expect("rejected file shows a notice").message;
    assert!(notice.contains("Golden.flac is not an extension"), "{notice}");
    assert!(notice.contains("Drop a .fext file"), "{notice}");
}

