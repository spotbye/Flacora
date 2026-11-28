use super::*;
use crate::fixtures;
use std::path::PathBuf;

#[test]
fn version_updates_are_informational_and_disappear_after_manual_replacement() {
    use crate::{app::Msg, extension_updates::{self, CheckState}};
    let mut rt = runtime(fixtures::extensions_page);
    let installed_before: Vec<_> = rt.state.installed.iter().map(|i| i.manifest.clone()).collect();
    let feed = extension_updates::parse(br#"{"schema_version":1,"extensions":{"cyan-dl":{"version":"1.0.3"},"odskyler":{"version":"1.0.2+same"},"unknown":{"version":"9.0.0"}}}"#).unwrap();
    rt.state.apply(Msg::ExtensionVersionsChecked(Ok(feed)));
    rt.invalidate();
    rt.frame(1.0);
    assert!(rt.find("extension-update-cyan-dl").is_some());
    assert!(rt.find("extension-update-odskyler").is_none());
    assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("extension-update-summary") && el.text.as_deref() == Some("1 update available. Download packages manually.")));
    assert_eq!(rt.state.installed.iter().map(|i| i.manifest.clone()).collect::<Vec<_>>(), installed_before);
    assert_eq!(rt.state.importing, 0);
    tap(&mut rt, "check-extension-updates");
    assert_eq!(rt.state.extension_update_check, CheckState::Checking);
    rt.state.apply(Msg::ExtensionVersionsChecked(Err("offline".into())));
    rt.invalidate();
    rt.frame(1.0);
    assert!(rt.find("extension-update-cyan-dl").is_some(), "failed checks keep last known information");
    assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("extension-update-summary") && el.text.as_deref().is_some_and(|t| t.contains("Could not check updates"))));
    let mut replacement = fixtures::installed("cyan-dl");
    replacement.manifest.version = "1.0.3".into();
    rt.state.apply(Msg::Imported(PathBuf::from("update.fext"), Ok(Box::new(replacement))));
    rt.invalidate();
    rt.frame(1.0);
    assert!(rt.find("extension-update-cyan-dl").is_none());
}

#[test]
fn update_indicators_and_refresh_stay_accessible_at_supported_widths() {
    for size in [crate::MIN_WINDOW_SIZE, (1280.0, 820.0), (1920.0, 1080.0)] {
        let mut rt = runtime_at(fixtures::extension_updates_available, size);
        assert!(rt.find("check-extension-updates").is_some());
        assert!(rt.find("check-extension-status").is_some());
        assert!(rt.find("install-extension").is_some());
        assert!(rt.find("extension-update-cyan-dl").is_some());
        let ([x, _, width, _], _) = rt.presented("ext-cyan-dl").unwrap();
        let ([ux, _, uw, _], _) = rt.presented("extension-update-cyan-dl").unwrap();
        assert!(ux >= x && ux + uw <= x + width);
        tap(&mut rt, "check-extension-updates");
        assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("extension-update-summary") && el.text.as_deref() == Some("Checking extension versions…")));
    }
}

#[test]
fn extensions_lists_local_installs_and_keeps_the_header_actions_visible() {
    let mut empty = runtime(fixtures::extensions_empty);
    assert!(empty.find("drop-zone").is_some());
    assert!(empty.find("ext-lucida").is_none());
    tap(&mut empty, "nav-downloads");
    tap(&mut empty, "go-extensions");
    assert_eq!(empty.state.page, Page::Extensions);

    let mut rt = runtime(fixtures::extensions_page);
    assert!(rt.find("drop-zone").is_none());
    for id in rt.state.installed.iter().map(|extension| &extension.manifest.id) {
        assert!(rt.find(&format!("ext-{id}")).is_some(), "{id} is listed");
        assert!(rt.find(&format!("site-{id}")).is_some(), "{id} shows its website");
    }
    for key in ["install-file", "refresh", "ext-doubledouble", "install-odskyler"] {
        assert!(rt.find(key).is_none(), "{key} is absent");
    }
    assert!(rt.find("kind-odesli").is_some() && rt.find("kind-lucida").is_some(), "each card says what the extension is");
    let install = rt.find("install-extension").unwrap();
    let check = rt.find("check-extension-status").unwrap();
    assert!(install.0 > check.0 && install.1 == check.1);
    let ([_, head_y, _, head_h], _) = rt.presented("page-head").unwrap();
    assert_eq!(rt.presented("extension-filters").unwrap().0[1], head_y + head_h + 16.0);
    rt.wheel(760.0, 520.0, -2000.0);
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.scroll_at_bottom("extension-list"), Some(true));
    assert_eq!(rt.find("install-extension"), Some(install));
}

#[test]
fn remove_all_requires_confirmation_and_removes_packages_across_filters() {
    let root = tempfile::tempdir().unwrap();
    let mut rt = runtime_at(fixtures::extensions_page, crate::MIN_WINDOW_SIZE);
    rt.state.paths.root = root.path().to_path_buf();
    rt.state.link = Some(crate::app::Link::new(std::sync::Arc::new(|| {})));
    let ids: Vec<_> = rt.state.installed.iter().map(|item| item.manifest.id.clone()).collect();
    for id in &ids {
        std::fs::create_dir_all(rt.state.paths.extension(id)).unwrap();
    }
    let data = rt.state.paths.data("cyan-dl");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::write(data.join("settings"), "keep").unwrap();
    let history = rt.state.history.clone();

    tap(&mut rt, "ext-filter-downloaders");
    tap(&mut rt, "remove-all-extensions");
    let count = format!("This will uninstall {} installed extensions.", ids.len());
    assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("remove-all-extensions-counts")
        && el.text.as_deref().is_some_and(|detail| detail.starts_with(&count))));
    rt.key(rune::KeyEvent { key: rune::Key::Enter, mods: rune::Mods::default() });
    rt.frame(1.0 / 60.0);
    assert!(rt.state.confirm_action.is_none());
    assert!(ids.iter().all(|id| rt.state.paths.extension(id).is_dir()));

    tap(&mut rt, "remove-all-extensions");
    tap(&mut rt, "confirm-remove-all-extensions");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !rt.state.removing.is_empty() && std::time::Instant::now() < deadline {
        rt.update(|s, tx| { s.drain_with(0.0, tx); });
        rt.frame(1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(rt.state.installed.is_empty() && rt.state.removing.is_empty());
    assert!(rt.find("drop-zone").is_some() && rt.find("remove-all-extensions").is_none());
    assert!(rt.state.extension_filter.is_none());
    assert!(ids.iter().all(|id| !rt.state.paths.extension(id).exists()));
    assert_eq!(std::fs::read_to_string(data.join("settings")).unwrap(), "keep");
    assert_eq!(rt.state.history, history);
}

#[test]
fn remove_all_explains_which_work_needs_to_finish() {
    let mut rt = runtime(fixtures::extensions_page);
    fixtures::job(&mut rt.state, "cyan-dl", "https://music.amazon.com/tracks/B00GZKCOQM", Status::Queued);
    tap(&mut rt, "remove-all-extensions");
    assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("remove-all-extensions-counts")
        && el.text.as_deref().is_some_and(|detail| detail.contains("Wait for active jobs"))));
    tap(&mut rt, "confirm-remove-all-extensions");
    assert_eq!(rt.state.confirm_action, Some(crate::app::ConfirmAction::RemoveAllExtensions));
    assert!(rt.state.removing.is_empty());
}

#[test]
fn old_lucida_status_only_shows_current_service_badges() {
    let mut rt = runtime(fixtures::extensions_page);
    tap(&mut rt, "ext-filter-downloaders");
    let elements = rt.elements();
    assert!(elements.iter().any(|el| el.path.contains("services-lucida/") && el.text.as_deref() == Some("Amazon Music")));
    assert!(elements.iter().all(|el| el.text.as_deref() != Some("Yandex Music")));
}

#[test]
fn extension_cards_are_alphabetical_within_each_category() {
    let mut rt = runtime(fixtures::extensions_page);
    tap(&mut rt, "nav-downloads");
    tap(&mut rt, "nav-extensions");

    let order = [
        "extension-category-downloaders", "ext-cyan-dl", "ext-flacdownloader", "ext-lucida", "ext-monochrome", "ext-odskyler",
        "extension-category-resolvers", "ext-odesli", "ext-squigly",
        "extension-category-metadata", "ext-musicbrainz",
        "extension-category-lyrics", "ext-betterlyrics", "ext-lrclib", "ext-lyricsplus", "ext-musixmatch",
    ];
    let positions = order.map(|key| rt.presented(key).unwrap_or_else(|| panic!("{key} is listed")).0[1]);
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]), "{positions:?}");

    let partial = runtime(fixtures::extension_options);
    assert!(partial.find("extension-category-metadata").is_none());
    assert!(partial.find("extension-category-lyrics").is_none());
}

#[test]
fn extension_settings_use_manifest_choices_and_update_the_composer() {
    let mut rt = runtime(|s| {
        fixtures::extension_options(s);
        s.extension_settings = None;
    });
    assert!(rt.find("configure-odesli").is_none());
    assert!(rt.find("extension-options-odskyler").is_none());
    tap(&mut rt, "configure-odskyler");
    for _ in 0..30 { rt.frame(1.0 / 60.0); }
    tap(&mut rt, "setting-odskyler-quality/segment1");
    assert_eq!(rt.state.settings.options.get("odskyler/quality").map(String::as_str), Some("hires"));

    tap(&mut rt, "configure-doubledouble");
    assert!(rt.find("extension-options-odskyler").is_none());
    for _ in 0..30 { rt.frame(1.0 / 60.0); }
    tap(&mut rt, "setting-doubledouble-region/segment1");
    assert_eq!(rt.state.settings.options.get("doubledouble/region").map(String::as_str), Some("eu"));

    tap(&mut rt, "nav-downloads");
    assert!(rt.find("option-quality").is_some());
    let quality = rt.state.selected().unwrap().manifest.options.iter().find(|o| o.key == "quality").unwrap();
    assert_eq!(rt.state.pick_index("odskyler", quality), 1);
}

#[test]
fn extension_settings_fold_without_moving_the_card_header() {
    let mut rt = runtime_at(|s| {
        fixtures::extension_options(s);
        s.extension_settings = None;
    }, (1280.0, 720.0));
    let height = |rt: &Runtime<App>| rt.presented("ext-cyan-dl").unwrap().0[3];
    let next_top = |rt: &Runtime<App>| rt.presented("ext-doubledouble").unwrap().0[1];
    let trigger = rt.presented("configure-cyan-dl").unwrap().0;
    let closed = height(&rt);
    let next_closed = next_top(&rt);
    let settle = |rt: &mut Runtime<App>| {
        for _ in 0..45 { rt.frame(1.0 / 60.0); }
    };

    tap(&mut rt, "configure-cyan-dl");
    let opening = height(&rt);
    assert_eq!(rt.presented("configure-cyan-dl").unwrap().0, trigger);
    assert!((next_top(&rt) - next_closed - (opening - closed)).abs() < 1.0);
    settle(&mut rt);
    let opened = height(&rt);
    assert!(closed < opening && opening < opened, "heights while opening: {closed}, {opening}, {opened}");

    tap(&mut rt, "configure-cyan-dl");
    let closing = height(&rt);
    assert!(closed < closing && closing < opened, "height while closing: {closing}");
    assert_eq!(rt.presented("configure-cyan-dl").unwrap().0, trigger);
    settle(&mut rt);
    assert_eq!(height(&rt), closed);
    assert_eq!(next_top(&rt), next_closed);
    assert!(rt.find("setting-cyan-dl-quality").is_none());
}

#[test]
fn the_empty_extension_drop_zone_highlights_and_yields_to_the_install_button() {
    let mut rt = runtime(fixtures::extensions_empty);
    let (empty, _) = rt.presented("drop-zone").unwrap();
    assert_eq!(empty[3], 224.0);
    let content = rt.presented("extensions-page").unwrap().0;
    assert!((empty[0] + empty[2] / 2.0 - content[0] - content[2] / 2.0).abs() < 1.0);

    let package = PathBuf::from("C:/Downloads/odesli.fext");
    let at = Some((900.0, 620.0));
    rt.drag_files(vec![package.clone()], at);
    rt.frame(1.0 / 60.0);
    assert!(rt.state.drop_over);
    assert!(rt.find("drop-overlay").is_none());
    assert!(rt.elements().iter().any(|element| element.text.as_deref() == Some("Drop to Install")));

    rt.drop_files(vec![package.clone()], at);
    rt.frame(1.0 / 60.0);
    assert!(rt.elements().iter().any(|element| element.text.as_deref() == Some("Installing Packages…")));

    rt.state.apply(crate::app::Msg::Imported(package, Ok(Box::new(fixtures::installed("odesli")))));
    rt.invalidate();
    rt.frame(1.0 / 60.0);
    assert!(rt.find("drop-zone").is_none());
    assert!(rt.find("install-extension").is_some());
    assert!(rt.find("ext-odesli").is_some());
}

#[test]
fn both_extension_install_controls_can_browse_and_import_a_local_package() {
    for (scene, key) in [(fixtures::extensions_empty as fn(&mut App), "drop-zone"), (fixtures::extensions_page, "install-extension")] {
        let mut rt = runtime(scene);
        tap(&mut rt, key);
        let (kind, options) = rt.file_dialog().expect("the install control requests a file picker");
        assert_eq!(kind, rune::FileDialogKind::Open);
        assert_eq!(options.filters, [("Extension Packages (*.fext)".into(), vec!["fext".into()])]);

        rt.resolve_file_dialog(None).unwrap();
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.importing, 0);

        tap(&mut rt, key);
        rt.resolve_file_dialog(Some(PathBuf::from("C:/Downloads/legacy.zip"))).unwrap();
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.importing, 0);
        assert_eq!(rt.state.notice.as_ref().unwrap().message, "Select only .fext extension packages.");

        tap(&mut rt, key);
        rt.resolve_file_dialog(Some(PathBuf::from("C:/Downloads/doubledouble.fext"))).unwrap();
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.importing, 1);
    }
}

#[test]
fn populated_extensions_shows_a_drop_overlay_only_while_dragging_packages() {
    let mut rt = runtime(fixtures::extensions_page);
    let at = Some((120.0, 400.0));
    rt.drag_files(vec![PathBuf::from("C:/Downloads/song.flac")], at);
    rt.frame(1.0 / 60.0);
    assert!(rt.find("drop-overlay").is_none());

    let package = PathBuf::from("C:/Downloads/doubledouble.fext");
    rt.drag_files(vec![package.clone()], at);
    rt.frame(1.0 / 60.0);
    assert!(rt.find("drop-overlay").is_some());
    rt.drop_files(vec![package], at);
    rt.frame(1.0 / 60.0);
    assert!(rt.find("drop-overlay").is_none());
    assert_eq!(rt.state.importing, 1);
    assert!(rt.elements().iter().any(|element| element.text.as_deref() == Some("Installing Packages…")));
}

#[test]
fn extension_cards_show_colored_kind_tiles_and_only_music_service_badges() {
    let mut rt = runtime(|s| {
        fixtures::extension_badges(s);
        s.images(std::sync::Arc::new(|| {}));
        for image in s.provider_logos.values() {
            image.wait().expect("embedded provider logo decodes");
        }
    });
    tap(&mut rt, "nav-downloads");
    tap(&mut rt, "nav-extensions");

    for key in [
        "kind-betterlyrics/lucide-mic-vocal",
        "kind-flacdownloader/lucide-cloud-download",
        "kind-musicbrainz/lucide-tag",
        "kind-odesli/lucide-share-2",
        "services-flacdownloader/provider-icon-qobuz",
        "services-flacdownloader/provider-icon-tidal",
        "services-flacdownloader/provider-icon-deezer",
        "services-monochrome/provider-icon-tidal",
        "services-odesli/provider-icon-spotify",
        "services-odesli/provider-icon-youtube-music",
        "services-odesli/provider-icon-amazon",
        "services-odesli/provider-icon-apple",
    ] {
        assert!(rt.find(key).is_some(), "{key} is present");
    }
    assert_eq!(fixtures::manifest("monochrome").services, ["Tidal"]);
    for id in ["betterlyrics", "flacdownloader", "musicbrainz", "odesli"] {
        assert!(rt.find(&format!("kind-label-{id}")).is_none(), "section header already shows the category");
    }
    for id in ["betterlyrics", "musicbrainz"] {
        assert!(rt.find(&format!("services-{id}")).is_none());
    }
    let elements = rt.elements();
    for logo in elements.iter().filter(|el| el.key.as_deref().is_some_and(|key| key.starts_with("provider-icon-"))) {
        let badge = &elements[logo.parent.expect("provider icon has a wrapper")];
        assert_eq!(badge.fill, rune::Color::TRANSPARENT);
    }
    for (id, service) in [("flacdownloader", "qobuz"), ("odesli", "spotify")] {
        let group = format!("/services-{id}/");
        let key = format!("provider-icon-{service}");
        let logo = elements.iter().find(|el| el.path.contains(&group) && el.key.as_deref() == Some(&key)).unwrap();
        let (website, _) = rt.presented(&format!("site-{id}")).unwrap();
        assert_eq!(logo.rect[0], website[0]);
    }
    let mut fills = Vec::new();
    for (id, label) in [("betterlyrics", "Lyrics"), ("flacdownloader", "Downloader"), ("musicbrainz", "Metadata"), ("odesli", "Link Resolver")] {
        let key = format!("kind-{id}");
        let tile = elements.iter().find(|el| el.key.as_deref() == Some(&key)).expect("kind tile exists");
        assert_eq!(tile.label.as_deref(), Some(label));
        assert!(!fills.contains(&tile.fill), "{label} has a distinct tile color");
        fills.push(tile.fill);
        assert!(tile.rect[0] < rt.presented(&format!("site-{id}")).unwrap().0[0], "{label} is on the left");
    }
    let ([status_x, status_y, status_w, status_h], _) = rt.presented("online-status-odesli").unwrap();
    let ([remove_x, remove_y, _, remove_h], _) = rt.presented("remove-odesli").unwrap();
    assert!((remove_x - status_x - status_w - 8.0).abs() < 1.0);
    assert!((status_y + status_h / 2.0 - remove_y - remove_h / 2.0).abs() < 1.0);
    assert!(status_x > rt.presented("site-odesli").unwrap().0[0]);
}

#[test]
fn extension_status_refresh_checks_every_installed_category() {
    let mut rt = runtime(fixtures::extension_status);
    let elements = rt.elements();
    for (id, label) in [("cyan-dl", "Online"), ("lucida", "Offline"), ("odesli", "Status Unknown"),
        ("musicbrainz", "Online"), ("betterlyrics", "Degraded")]
    {
        let key = format!("online-status-{id}");
        assert!(elements.iter().any(|el| el.key.as_deref() == Some(&key) && el.label.as_deref() == Some(label)));
    }

    // Unknown legacy extensions without a domain finish without issuing a network request.
    for installed in &mut rt.state.installed {
        installed.manifest.id = format!("test-{}", installed.manifest.id);
        installed.manifest.status.clear();
        installed.manifest.description.clear();
    }
    rt.state.link = Some(crate::app::Link::new(std::sync::Arc::new(|| {})));
    rt.frame(0.0);
    tap(&mut rt, "check-extension-status");
    assert!(rt.state.checking.len() <= 4, "status checks are concurrency-bounded");
    assert_eq!(rt.state.checking.len() + rt.state.status_queue.len(), rt.state.installed.len());
    assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("online-status-test-betterlyrics")
        && el.label.as_deref() == Some("Checking…")));
}

#[test]
fn extension_filter_moves_remaining_rows_to_the_top_in_one_motion() {
    let mut rt = runtime(fixtures::extension_badges);
    let top = |rt: &Runtime<App>| rt.presented("extension-category-metadata").unwrap().0[1];
    let list_top = rt.presented("extension-list").unwrap().0[1];
    let mut positions = vec![top(&rt)];
    tap(&mut rt, "ext-filter-metadata");
    positions.push(top(&rt));
    for _ in 0..36 {
        rt.frame(1.0 / 60.0);
        positions.push(top(&rt));
    }
    assert!((positions[5] - list_top) < (positions[0] - list_top) * 0.36, "{positions:?}");
    assert!(positions.windows(2).all(|pair| pair[1] <= pair[0] + 0.5), "{positions:?}");
    assert!(positions[18..].windows(2).all(|pair| pair[0] - pair[1] < 3.0), "{positions:?}");
    assert!((positions.last().unwrap() - list_top).abs() < 1.0, "{positions:?}");
}

#[test]
fn changing_the_extension_filter_scrolls_to_the_top_immediately() {
    let mut rt = runtime_at(fixtures::extensions_page, (1080.0, 520.0));
    let top = |rt: &Runtime<App>| rt.presented("extension-category-downloaders").unwrap().0[1];
    let list_top = rt.presented("extension-list").unwrap().0[1];
    rt.wheel(760.0, 410.0, -120.0);
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }
    let scrolled = top(&rt);
    assert!(scrolled < list_top - 60.0, "{scrolled} vs {list_top}");

    tap(&mut rt, "ext-filter-downloaders");
    assert!(top(&rt) > scrolled + 3.0 && top(&rt) < list_top - 3.0);
    for _ in 0..36 {
        rt.frame(1.0 / 60.0);
    }
    assert!((top(&rt) - list_top).abs() < 1.0);
}

#[test]
fn extensions_can_be_filtered_by_category_and_rechecked_individually() {
    let mut rt = runtime(fixtures::extension_status);
    assert!(rt.find("status-detail-odesli").is_some(), "unknown status reason displays inline");
    assert!(rt.find("ext-filter-lyrics").is_some());
    tap(&mut rt, "ext-filter-lyrics");
    assert!(rt.find("ext-betterlyrics").is_some());
    assert!(rt.find("ext-cyan-dl").is_none());
    tap(&mut rt, "ext-filter-all");
    assert!(rt.find("ext-cyan-dl").is_some());

    // Clicking one extension's status badge re-checks only that extension.
    for installed in &mut rt.state.installed {
        installed.manifest.id = format!("one-{}", installed.manifest.id);
        installed.manifest.status.clear();
        installed.manifest.description.clear();
    }
    rt.state.link = Some(crate::app::Link::new(std::sync::Arc::new(|| {})));
    rt.invalidate();
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }
    tap(&mut rt, "online-status-one-cyan-dl");
    assert_eq!(rt.state.checking.len(), 1);
    assert!(rt.state.checking.contains_key("one-cyan-dl"));
}

#[test]
fn extensions_opens_rentry_in_both_states() {
    for scene in [fixtures::extensions_empty as fn(&mut App), fixtures::extensions_page] {
        let mut rt = runtime(scene);
        assert!(rt.elements().iter().any(|el| {
            el.key.as_deref() == Some("download-extensions") && el.text.as_deref() == Some("Get Extensions on Rentry")
        }));
        tap(&mut rt, "download-extensions");
        assert_eq!(rt.take_url_requests(), ["https://rentry.org/flacora"]);
    }
}

#[test]
fn extension_card_remove_can_be_cancelled_inline_and_exposes_data_folder_button() {
    let mut rt = runtime(fixtures::extensions_page);
    assert!(rt.find("data-cyan-dl").is_some());
    tap(&mut rt, "remove-cyan-dl");
    assert_eq!(rt.state.confirm_action, Some(crate::app::ConfirmAction::RemoveExtension("cyan-dl".into())));
    assert!(rt.find("cancel-remove-cyan-dl").is_some());
    tap(&mut rt, "cancel-remove-cyan-dl");
    assert!(rt.state.confirm_action.is_none());
}

#[test]
fn an_available_update_stays_in_the_sidebar_on_settings() {
    let rt = runtime(fixtures::settings_update_available);
    assert!(rt.find("sidebar-update").is_some());
    assert!(rt.find("check-for-updates").is_none());
    assert!(rt.find("app-update").is_none());
    assert!(rt.find("view-update-release").is_none());
}

#[test]
fn extension_website_is_left_aligned_and_opens_as_a_link() {
    let mut rt = runtime(fixtures::extension_badges);
    let elements = rt.elements();
    let title = elements.iter()
        .find(|el| el.path.contains("ext-odesli/") && el.text.as_deref() == Some("Odesli"))
        .unwrap();
    let website = elements.iter().find(|el| el.key.as_deref() == Some("site-odesli")).unwrap();
    assert_eq!(website.rect[0], title.rect[0]);
    assert_eq!(website.fill, rune::Color::TRANSPARENT);
    tap(&mut rt, "site-odesli");
    assert_eq!(rt.take_url_requests(), ["https://odesli.co"]);
}

