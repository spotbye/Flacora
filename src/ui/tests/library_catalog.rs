use super::*;
use crate::{fixtures, jobs::Status};

#[test]
fn catalog_header_artwork_preserves_geometry_and_selection_when_resized() {
    let mut rt = runtime(|s| {
        fixtures::catalog_results(s);
        s.catalog.result.as_mut().unwrap().artwork = Some("https://example.com/catalog-cover.png".into());
    });
    for size in [crate::MIN_WINDOW_SIZE, (1280.0, 720.0), (2560.0, 1440.0)] {
        rt.set_viewport(size, 1.0);
        rt.frame(0.0);
        rt.frame(0.6);
        let (cover, _) = rt.presented("catalog-cover").expect("catalog header artwork");
        let (page, _) = rt.presented("catalog-result-page").unwrap();
        let (select, _) = rt.presented("catalog-select-all").unwrap();
        assert_eq!((cover[2], cover[3]), (64.0, 64.0));
        assert!(cover[0] >= page[0] && cover[0] + cover[2] <= page[0] + page[2]);
        assert!(select[0] > cover[0] + cover[2], "selection stays beside the artwork");
    }
    tap(&mut rt, "catalog-select-all");
    assert!(!rt.state.catalog.selected.is_empty());
    tap(&mut rt, "catalog-use");
    assert_eq!(rt.state.page, crate::app::Page::Downloads);
    assert!(!rt.state.url.text.is_empty());
}

#[test]
fn the_library_shows_its_recorded_source_and_leaves_older_links_generic() {
    let mut rt = runtime(|s| {
        fixtures::library_sources(s);
        s.images(std::sync::Arc::new(|| {}));
        for image in s.provider_logos.values() {
            image.wait().expect("embedded provider logo decodes");
        }
    });
    tap(&mut rt, "nav-downloads");
    tap(&mut rt, "nav-library");

    let elements = rt.elements();
    let mut source_positions = Vec::new();
    for (row, key, name) in [("entry-0", "provider-icon-tidal", "Tidal"), ("entry-3", "source-link", "Audio Source")] {
        let source = elements.iter().find(|el| el.path.contains(row) && el.key.as_deref() == Some(key)).unwrap();
        assert_eq!(source.role, "img");
        assert_eq!(source.help.as_deref(), Some(name));
        assert_eq!(source.fill, rune::Color::TRANSPARENT);
        assert!(!elements.iter().any(|el| el.path.contains(row) && el.text.as_deref() == Some(name)));
        source_positions.push(source.rect[0]);
    }
    assert_eq!(source_positions[0], source_positions[1]);
}

#[test]
fn download_progress_follows_astryx_and_switches_between_reported_stages() {
    let mut rt = runtime(fixtures::queue_indeterminate);
    let track_and_fill = |rt: &Runtime<App>, fill_key: &str| {
        let elements = rt.elements();
        let track = elements.iter().find(|el| el.path.ends_with("/progress-1/track")).unwrap();
        let fill = elements.iter().find(|el| el.key.as_deref() == Some(fill_key) && el.path.contains("/progress-1/track/")).unwrap();
        assert_eq!(track.radius, 4.0);
        assert_eq!(fill.radius, 4.0);
        assert_eq!(track.target[3], 8.0);
        assert_eq!(track.fill, theme::kit(ThemePreset::Neutral, true).c.border_strong);
        (fill.loop_phase, fill.target[2], track.target[2])
    };
    let (loop_phase, segment_width, track_width) = track_and_fill(&rt, "moving-fill");
    assert!(loop_phase.is_some());
    assert!((segment_width / track_width - 0.30).abs() < 0.01);
    let event = |line: &str| crate::app::Msg::Job(1, crate::runner::JobEvent::Line(line.to_owned()));
    rt.state.apply(event(r#"@flacora {"event":"progress","value":0.5}"#));
    rt.invalidate();
    rt.frame(1.0 / 60.0);
    let (loop_phase, fill_width, track_width) = track_and_fill(&rt, "fill");
    assert!(loop_phase.is_none());
    assert!((fill_width / track_width - 0.5).abs() < 0.01);
    assert!(rt.elements().iter().any(|el| el.path.contains("/progress-1/") && el.text.as_deref() == Some("50%")));

    rt.state.apply(event(r#"@flacora {"event":"stage","text":"Writing tags"}"#));
    rt.invalidate();
    rt.frame(1.0 / 60.0);
    assert!(track_and_fill(&rt, "moving-fill").0.is_some());
    assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("progress-1")
        && el.label.as_deref() == Some("Writing Tags")));
    assert!(!rt.elements().iter().any(|el| el.path.contains("/progress-1/") && el.text.as_deref() == Some("50%")));
}

#[test]
fn destructive_buttons_use_the_danger_palette() {
    let danger = theme::kit(ThemePreset::Neutral, true).c.danger;
    for (scene, key, filled) in [
        (fixtures::queue_cancel_confirmation as fn(&mut App), "confirm-cancel-all", true),
        (fixtures::queue, "cancel-1", false),
        (fixtures::queue, "remove-4", false),
        (fixtures::library, "clear-history", true),
        (fixtures::library, "remove-entry-0", false),
        (fixtures::extensions_page, "remove-odesli", false),
        (fixtures::settings, "settings-clear-history", true),
    ] {
        let rt = runtime(scene);
        let elements = rt.elements();
        let (index, button) = elements.iter().enumerate().find(|(_, el)| el.key.as_deref() == Some(key)).unwrap();
        assert_eq!(button.fill, if filled { danger.bg } else { rune::Color::TRANSPARENT }, "{key}");
        let icon = elements.iter().find(|el| el.parent == Some(index) && el.key.as_deref() == Some("icon")).unwrap();
        assert_eq!(icon.text_color, Some(danger.fg), "{key}");
    }
}

#[test]
fn library_hover_highlights_actions_on_a_single_track() {
    let mut rt = runtime(fixtures::library_single);
    let (x, y) = rt.find("entry-0").unwrap();
    rt.pointer_move(x, y);
    for _ in 0..12 {
        rt.frame(1.0 / 60.0);
    }
    let elements = rt.elements();
    let entry = elements.iter().find(|el| el.key.as_deref() == Some("entry-0")).unwrap();
    assert_eq!(entry.hover, 0.0);
    assert_eq!(entry.fill, rune::Color::TRANSPARENT);

    let (x, y) = rt.find("remove-entry-0").unwrap();
    rt.pointer_move(x, y);
    for _ in 0..12 {
        rt.frame(1.0 / 60.0);
    }
    assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("remove-entry-0") && el.hover > 0.5));
    tap(&mut rt, "remove-entry-0");
    assert!(rt.state.history.is_empty());
}

#[test]
fn the_library_scrollbar_stays_visible_and_can_be_dragged_when_tracks_overflow() {
    let mut rt = runtime_at(fixtures::library_overflow, (1280.0, 720.0));
    rt.set_system_scroll_bars(rune::ScrollBars::Always);
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }
    let ([x, y, w, h], opacity) = rt.presented("thumb").unwrap();
    assert!(w > 0.0 && h > 0.0 && opacity > 0.5);
    let ([card_x, _, card_w, _], _) = rt.presented("library-list").unwrap();
    assert!((card_x + card_w - 1256.0).abs() < 1.0);
    assert!(x >= card_x + card_w);

    let (x, y) = (x + w / 2.0, y + h / 2.0);
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_move(x, 690.0);
    rt.frame(1.0 / 60.0);
    rt.pointer_up(x, 690.0);
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.scroll_at_bottom("library-scroll"), Some(true));

    let mut single = runtime(fixtures::library_single);
    single.set_system_scroll_bars(rune::ScrollBars::Always);
    single.frame(1.0 / 60.0);
    let (thumb, opacity) = single.presented("thumb").unwrap();
    assert!(thumb[3] == 0.0 || opacity == 0.0);
}

#[test]
fn the_library_prompts_to_update_outdated_web_metadata() {
    let mut rt = runtime(fixtures::library_outdated_metadata);
    tap(&mut rt, "lookup-0");
    tap(&mut rt, "lookup-0-tidal-web");
    assert!(rt.state.jobs.is_empty());
    assert_eq!(rt.state.page, Page::Library);
    assert!(rt.state.notice.as_ref().is_some_and(|notice| notice.message.contains("v1.0.1")));
}

#[test]
fn the_library_queues_the_chosen_lookup_and_keeps_one_history_entry_per_track() {
    let mut rt = runtime(fixtures::library_lookups);
    assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("lookup-0") && el.label.as_deref() == Some("Lyrics & Metadata")));
    tap(&mut rt, "lookup-0");
    let elements = rt.elements();
    let card = elements.iter().find(|el| el.key.as_deref() == Some("library-list")).unwrap();
    let panel = elements.iter().find(|el| el.key.as_deref() == Some("lookup-providers-0")).unwrap();
    assert_ne!(card.fill, panel.fill, "lookup panel recesses inside library card");
    for action in ["Get Lyrics", "Tag FLAC", "Export JSON"] {
        assert!(elements.iter().any(|el| el.path.contains("/lookup-providers-0/") && el.text.as_deref() == Some(action)));
    }
    for provider in [
        "betterlyrics", "lrclib", "lrc-red", "lyricsplus", "musixmatch", "musicbrainz",
        "spotify-web", "tidal-web", "deezer-web", "qobuz-web", "amazon-web",
    ] {
        assert!(rt.find(&format!("lookup-0-{provider}")).is_some(), "{provider} is a choice");
    }
    assert!(rt.state.jobs.is_empty(), "opening the picker does not start a lookup");
    tap(&mut rt, "lookup-disclosure-0/trigger");
    assert!(rt.state.lookup_picker.is_none());
    assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("lookup-0") && el.focused));
    tap(&mut rt, "lookup-0");
    tap(&mut rt, "lookup-0-musixmatch");
    assert_eq!(rt.state.page, Page::Downloads);
    let job = &mut rt.state.jobs[0];
    assert_eq!(job.extension.manifest.id, "musixmatch");
    assert_eq!(job.status, Status::Queued);
    assert!(job.lookup_file.as_ref().is_some_and(|p| p.extension().is_some_and(|ext| ext == "flac")));
    let id = job.id;
    job.status = Status::Running;
    job.handle = Some(crate::runner::JobHandle::detached());
    let count = rt.state.history.len();
    let saved = r#"@flacora {"event":"saved","path":"C:/Music/Golden.lrc"}"#;
    rt.state.job_event(id, crate::runner::JobEvent::Line(saved.into()));
    rt.state.job_event(id, crate::runner::JobEvent::Exit(Ok(0)));
    rt.invalidate();
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.jobs[0].status, Status::Done);
    assert_eq!(rt.state.history.len(), count);
    assert!(rt.find("lookup-source-1").is_some() && rt.find("reveal-1").is_some());
}

#[test]
fn a_catalog_lookup_receives_the_library_source_url() {
    let mut rt = runtime(fixtures::library_lookups);
    tap(&mut rt, "lookup-0");
    tap(&mut rt, "lookup-0-tidal-web");
    let [job] = rt.state.jobs.as_slice() else { panic!("expected one metadata lookup") };
    assert_eq!(job.extension.manifest.id, "tidal-web");
    assert_eq!(job.url, rt.state.history[0].url);
    assert!(job.lookup_file.is_some());
}

#[test]
fn one_service_choice_routes_a_fifty_track_catalog_batch() {
    let mut rt = runtime(fixtures::catalog_batch);
    tap(&mut rt, "catalog-use");
    assert!(rt.find("batch-service-manual").is_some());
    assert!(rt.state.batch_service().is_none());
    rt.state.settings.preferred.insert("tidal".into(), "odskyler".into());
    tap(&mut rt, "batch-service-tidal");
    tap(&mut rt, "download");
    assert_eq!(rt.state.jobs.len(), 50);
    assert!(rt.state.batch_service().is_none(), "the draft choice was consumed");

    for id in 1..=50 {
        let job = &mut rt.state.jobs[(id - 1) as usize];
        assert_eq!(job.batch_service.map(|service| service.key), Some("tidal"));
        job.status = Status::Running;
        job.handle = Some(crate::runner::JobHandle::detached());
        let links = if id == 1 {
            r#"@flacora {"event":"links","links":{"Amazon Music":"https://music.amazon.com/tracks/B00GZKCOQM"}}"#
        } else {
            r#"@flacora {"event":"links","links":{"Amazon Music":"https://music.amazon.com/tracks/B00GZKCOQM","Tidal":"https://tidal.com/track/123"}}"#
        };
        rt.state.job_event(id, crate::runner::JobEvent::Line(links.into()));
        rt.state.job_event(id, crate::runner::JobEvent::Exit(Ok(0)));
    }

    let (fallback, queued) = rt.state.jobs.split_first().unwrap();
    assert_eq!(fallback.status, Status::Choosing);
    assert_eq!(fallback.choices.len(), 1);
    assert_eq!(fallback.choices[0].service.key, "amazon");
    assert!(queued.iter().all(|job| job.status == Status::Queued && job.extension.manifest.id == "odskyler"
        && job.service.is_some_and(|service| service.key == "tidal")));
    rt.invalidate();
    rt.frame(1.0 / 60.0);
    assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("choices-heading-1")
        && el.text.as_deref() == Some("Tidal is unavailable for this track. Choose a service.")));
    tap(&mut rt, "choose-1-0");
    assert_eq!(rt.state.jobs[0].service.map(|service| service.key), Some("amazon"));
}

#[test]
fn catalog_only_extensions_are_selectable_in_catalog_and_hidden_from_library_lookups() {
    let mut rt = runtime(fixtures::catalog_with_resolver_sources);
    tap(&mut rt, "catalog-provider-ytmusic-web");
    assert_eq!(rt.state.catalog.provider.as_deref(), Some("ytmusic-web"));
    tap(&mut rt, "catalog-provider-apple-web");
    assert_eq!(rt.state.catalog.provider.as_deref(), Some("apple-web"));

    tap(&mut rt, "nav-library");
    tap(&mut rt, "lookup-0");
    assert!(rt.find("lookup-0-deezer-web").is_some());
    assert!(rt.find("lookup-0-apple-web").is_none());
    assert!(rt.find("lookup-0-ytmusic-web").is_none());
}

#[test]
fn catalog_navigation_and_provider_changes_wait_for_the_home_feed_button() {
    let directory = tempfile::tempdir().unwrap();
    let mut rt = runtime(fixtures::catalog_idle);
    rt.state.paths.root = directory.path().to_path_buf();
    for extension in &mut rt.state.installed {
        extension.dir = directory.path().join(&extension.manifest.id);
    }
    rt.state.link = Some(crate::app::Link::new(std::sync::Arc::new(|| {})));

    tap(&mut rt, "nav-downloads");
    tap(&mut rt, "nav-catalog");
    assert!(rt.find("catalog-idle").is_some());
    assert!(!rt.state.catalog.busy());

    tap(&mut rt, "catalog-provider-tidal-web");
    assert_eq!(rt.state.catalog.provider.as_deref(), Some("tidal-web"));
    assert!(rt.find("catalog-idle").is_some());
    assert!(!rt.state.catalog.busy());

    tap(&mut rt, "catalog-home");
    assert_eq!(rt.state.catalog.active_extension(), Some("tidal-web"));
    tap(&mut rt, "catalog-home");
    assert!(!rt.state.catalog.busy(), "hiding an in-flight feed cancels its request");
    assert!(rt.find("catalog-idle").is_some());
}

#[test]
fn the_home_feed_toggle_hides_and_restores_a_loaded_feed() {
    let mut rt = runtime(fixtures::catalog_feed);
    tap(&mut rt, "catalog-feed-item-0-0");
    let selected = rt.state.catalog.selected.clone();

    tap(&mut rt, "catalog-home");
    assert!(rt.find("catalog-feed").is_none());
    assert!(rt.find("catalog-idle").is_some());

    tap(&mut rt, "catalog-home");
    assert!(rt.find("catalog-feed-item-0-0").is_some());
    assert_eq!(rt.state.catalog.selected, selected);
    assert!(!rt.state.catalog.busy());

    tap(&mut rt, "catalog-home");
    tap(&mut rt, "catalog-provider-spotify-web");
    tap(&mut rt, "catalog-home");
    assert!(rt.find("catalog-feed").is_none(), "a provider change drops the previous feed");
}

#[test]
fn catalog_home_feed_scrolls_sideways_and_sends_selected_tracks_to_downloads() {
    let mut rt = runtime(fixtures::catalog_feed);
    let viewport = rt.presented("catalog-shelf-0").unwrap().0;
    let card = "catalog-feed-item-0-0";
    let first = rt.presented(card).unwrap().0;
    assert!(rt.find("catalog-feed").is_some() && rt.find("catalog-results").is_none());
    assert!(rt.presented("catalog-shelf-1").unwrap().0[1] > viewport[1] + viewport[3]);
    assert!(rt.presented("catalog-feed-item-0-8").unwrap().0[0] > viewport[0] + viewport[2]);

    tap(&mut rt, card);
    assert!(rt.state.catalog.selected.contains("https://www.deezer.com/track/3412534581"));
    tap(&mut rt, "catalog-shelf-0-next");
    for _ in 0..28 {
        rt.frame(1.0 / 60.0);
    }
    assert!(rt.presented(card).unwrap().0[0] < first[0] - 100.0);
    tap(&mut rt, "catalog-shelf-0-prev");
    for _ in 0..28 {
        rt.frame(1.0 / 60.0);
    }
    assert!((rt.presented(card).unwrap().0[0] - first[0]).abs() < 1.0);

    tap(&mut rt, "catalog-use");
    assert_eq!(rt.state.page, Page::Downloads);
    assert!(rt.state.url.text.contains("https://www.deezer.com/track/3412534581"));
}

#[test]
fn catalog_selection_prepares_tracks_without_enqueuing_a_collection() {
    let mut rt = runtime(fixtures::catalog_link);
    tap(&mut rt, "download");
    assert_eq!(rt.state.page, Page::Catalog);
    assert!(rt.state.jobs.is_empty());
    assert!(rt.state.url.text.is_empty());

    let mut rt = runtime(fixtures::catalog_results);
    assert!(rt.find("catalog-item-1").is_some());
    assert!(rt.find("catalog-artwork-0").is_some() && rt.find("catalog-artwork-2").is_some());
    tap(&mut rt, "catalog-item-0");
    tap(&mut rt, "catalog-use");
    assert_eq!(rt.state.page, Page::Downloads);
    assert_eq!(rt.state.url.text, "https://www.deezer.com/track/3412534581");
    assert!(rt.state.jobs.is_empty());
    tap(&mut rt, "nav-catalog");
    tap(&mut rt, "catalog-provider-spotify-web");
    assert!(rt.state.catalog.result.is_none() && rt.state.catalog.selected.is_empty());
}

#[test]
fn catalog_scrollbar_reaches_the_centered_load_more_button() {
    let mut rt = runtime_at(fixtures::catalog_overflow, (1280.0, 720.0));
    rt.set_system_scroll_bars(rune::ScrollBars::Always);
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }
    let ([bar_x, bar_y, bar_w, bar_h], opacity) = rt.presented("thumb").unwrap();
    assert!(bar_w > 0.0 && bar_h > 0.0 && opacity > 0.5);
    let ([controls_x, _, controls_w, _], _) = rt.presented("catalog-controls").unwrap();
    assert!(bar_x >= controls_x + controls_w);

    let (x, y) = (bar_x + bar_w / 2.0, bar_y + bar_h / 2.0);
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_move(x, 690.0);
    rt.frame(1.0 / 60.0);
    rt.pointer_up(x, 690.0);
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.scroll_at_bottom("catalog-results"), Some(true));

    let ([more_x, _, more_w, _], _) = rt.presented("catalog-more").unwrap();
    assert!(more_w < controls_w / 4.0);
    assert!((more_x + more_w / 2.0 - (controls_x + controls_w / 2.0)).abs() < 1.0);
    tap(&mut rt, "catalog-item-11");
    assert!(rt.state.catalog.selected.contains("https://www.deezer.com/track/12"));
    assert!(rt.find("catalog-more").is_some());
}

#[test]
fn catalog_clear_search_has_padding_and_refocuses_the_empty_field() {
    let mut rt = runtime_at(fixtures::catalog_results, crate::MIN_WINDOW_SIZE);
    assert_clear_button_inset(&rt, "catalog-input", "catalog-clear-search");

    tap(&mut rt, "catalog-clear-search");
    assert!(rt.state.catalog.input.text.is_empty());
    assert!(rt.state.catalog.input.focused);
    assert!(rt.find("catalog-clear-search").is_none());
    assert!(rt.find("catalog-results").is_some());
}

#[test]
fn library_clear_search_refocuses_the_field_and_shows_all_entries() {
    let mut rt = runtime_at(fixtures::library_search, crate::MIN_WINDOW_SIZE);
    assert_eq!(rt.state.filtered_history().len(), 1);
    assert_clear_button_inset(&rt, "search", "library-clear-search");

    tap(&mut rt, "library-clear-search");
    assert!(rt.state.search.text.is_empty() && rt.state.search.focused);
    assert!(rt.find("library-clear-search").is_none());
    assert_eq!(rt.state.filtered_history().len(), rt.state.history.len());
}

#[test]
fn downloads_clear_links_refocuses_the_field_and_resets_the_batch_draft() {
    let mut rt = runtime_at(fixtures::downloads_batch_input, crate::MIN_WINDOW_SIZE);
    assert_eq!(rt.state.batch_service().map(|service| service.name), Some("Tidal"));
    assert_clear_button_inset(&rt, "url", "downloads-clear-links");
    let ([badge_x, _, badge_width, _], _) = rt.presented("batch-summary").unwrap();
    let ([clear_x, _, _, _], _) = rt.presented("downloads-clear-links").unwrap();
    assert!(clear_x - badge_x - badge_width >= 6.0);

    tap(&mut rt, "downloads-clear-links");
    assert!(rt.state.url.text.is_empty() && rt.state.url.focused);
    assert!(rt.state.batch_destination.is_none());
    assert!(rt.find("downloads-clear-links").is_none() && rt.find("batch-summary").is_none());
}

fn assert_clear_button_inset(rt: &Runtime<App>, field: &str, button: &str) {
    let ([field_x, _, field_width, _], _) = rt.presented(field).unwrap();
    let ([clear_x, _, clear_width, _], _) = rt.presented(button).unwrap();
    assert!(field_x + field_width - clear_x - clear_width >= 6.0);
}

#[test]
fn catalog_provider_selection_is_single_choice_and_supports_arrow_keys() {
    let mut rt = runtime(fixtures::catalog_results);
    tap(&mut rt, "catalog-provider-spotify-web");
    assert_eq!(rt.state.catalog.provider.as_deref(), Some("spotify-web"));

    rt.key(rune::KeyEvent { key: rune::Key::Right, mods: rune::Mods::default() });
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.catalog.provider.as_deref(), Some("tidal-web"));

    let tree = rt.accessibility_json();
    fn checked_providers(node: &serde_json::Value) -> usize {
        let checked = node["key"].as_str().is_some_and(|key| key.starts_with("catalog-provider-"))
            && node["states"].as_array().is_some_and(|states| states.iter().any(|state| state == "checked"));
        usize::from(checked) + node["children"].as_array().into_iter().flatten().map(checked_providers).sum::<usize>()
    }
    assert_eq!(checked_providers(&tree), 1);
}

#[test]
fn catalog_reports_when_no_installed_extension_supports_a_link() {
    let mut rt = runtime(|s| {
        fixtures::catalog_missing_extension(s);
        s.catalog.error = None;
    });
    tap(&mut rt, "catalog-submit");
    assert!(rt.elements().iter().any(|el| el.key.as_deref() == Some("catalog-error")
        && el.text.as_deref() == Some(crate::catalog::MISSING_CATALOG_EXTENSION)));
    assert!(rt.state.catalog.result.is_none(), "stale results clear when opening an unsupported URL");
    assert!(rt.find("catalog-open-extensions").is_some());
    tap(&mut rt, "catalog-open-extensions");
    assert_eq!(rt.state.page, Page::Extensions);
}

#[test]
fn fresh_downloads_hides_composer_and_extension_cards_omit_the_catalog_badge() {
    let fresh = runtime(fixtures::fresh);
    assert!(fresh.find("composer").is_none(), "fresh onboarding hides disabled composer");
    assert!(fresh.find("go-extensions").is_some());

    let mut opts = runtime(fixtures::extension_options);
    opts.state.installed.push(fixtures::installed("deezer-web"));
    opts.invalidate();
    opts.frame(0.0);
    tap(&mut opts, "ext-filter-metadata");
    assert!(opts.find("ext-deezer-web").is_some());
    assert!(!opts.elements().iter().any(|el| el.text.as_deref() == Some("Catalog") && el.path.contains("/ext-deezer-web/")));
    assert!(opts.find("catalog-badge-deezer-web").is_none());
    tap(&mut opts, "ext-filter-all");
    let elements = opts.elements();
    let open_gear = elements.iter().find(|el| el.key.as_deref() == Some("configure-odskyler")).unwrap();
    let closed_gear = elements.iter().find(|el| el.key.as_deref() == Some("configure-cyan-dl")).unwrap();
    assert_ne!(open_gear.fill, closed_gear.fill, "expanded settings gear has an active fill");
}

#[test]
fn a_resolved_link_offers_the_services_it_was_found_on() {
    let mut rt = runtime(fixtures::choose);
    let id = rt.state.jobs[0].id;
    assert_eq!(rt.state.jobs[0].status, Status::Choosing);
    for i in 0..3 {
        assert!(rt.find(&format!("choose-{id}-{i}")).is_some(), "choice {i} is offered");
    }
    assert!(rt.find(&format!("choose-{id}-3")).is_none(), "only downloadable results are offered");
    assert!(rt.elements().iter().all(|el| !el.text.as_deref().is_some_and(|text| text.contains("Pandora"))));
    tap(&mut rt, &format!("choose-{id}-1"));
    let job = &rt.state.jobs[0];
    assert_eq!((job.status.clone(), job.service.map(|s| s.key)), (Status::Queued, Some("deezer")));
}

#[test]
fn library_inspector_missing_actions_and_catalog_in_library_badge_work() {
    let mut rt = runtime(|s| {
        fixtures::library(s);
        s.history[0].files = vec!["C:/Music/Present.flac".into()];
        s.history[1].files = vec!["C:/Music/Missing.flac".into()];
        s.library_files.insert("C:/Music/Present.flac".into(), crate::library::SidecarInfo {
            exists: true,
            sidecars: Vec::new(),
        });
        s.library_files.insert("C:/Music/Missing.flac".into(), crate::library::SidecarInfo {
            exists: false,
            sidecars: Vec::new(),
        });
    });
    assert!(rt.find("inspect-entry-0").is_some());
    tap(&mut rt, "inspect-entry-0");
    assert!(rt.find("track-inspector-0").is_some());
    assert!(rt.find("redownload-entry-1").is_some());
    assert!(rt.find("remove-missing").is_some());
    tap(&mut rt, "remove-missing");
    assert_eq!(rt.state.history.len(), 3);
    assert!(rt.state.undo.is_some());

    let mut cat = runtime(|s| {
        fixtures::catalog_results(s);
        s.history[0].url = "https://www.deezer.com/track/3412534581".into();
    });
    assert!(cat.find("catalog-in-library-0").is_some());
    assert!(cat.find("catalog-quick-0").is_some());
    tap(&mut cat, "catalog-quick-0");
    assert_eq!(cat.state.page, Page::Catalog, "quick queue keeps user in Catalog");
    assert_eq!(cat.state.jobs.len(), 1);
    assert_eq!(cat.state.jobs[0].url, "https://www.deezer.com/track/3412534581");
}

#[test]
fn library_batch_selection_status_filters_and_export_work() {
    let dir = tempfile::tempdir().unwrap();
    let mut rt = runtime(fixtures::library_batch);
    assert_eq!(rt.state.library_selected.len(), 2);
    assert!(rt.find("library-batch-bar").is_some());
    assert!(rt.find("library-status-lyrics").is_some());
    assert!(rt.find("library-status-missing").is_some());

    // Filter to Has Lyrics shows only the track with an .lrc sidecar.
    tap(&mut rt, "library-status-lyrics");
    assert_eq!(rt.state.filtered_history().len(), 1);
    tap(&mut rt, "library-status-missing");
    assert_eq!(rt.state.filtered_history().len(), 1);
    tap(&mut rt, "library-status-all");
    assert_eq!(rt.state.filtered_history().len(), 4);

    // Export saves selected tracks via the Save dialog.
    let export_path = dir.path().join("selected.csv");
    tap(&mut rt, "library-export");
    assert_eq!(rt.file_dialog().unwrap().0, rune::FileDialogKind::Save);
    rt.resolve_file_dialog(Some(export_path.clone())).unwrap();
    rt.frame(1.0 / 60.0);
    assert!(export_path.is_file());
    assert!(rt.state.info.as_ref().is_some_and(|n| n.message.contains("Exported 2 tracks")));

    // Batch lookup queues all selected non-missing FLACs and switches to Downloads.
    tap(&mut rt, "library-batch-lookup");
    assert!(rt.find("batch-lookup-lrclib").is_some());
    tap(&mut rt, "batch-lookup-lrclib");
    assert_eq!(rt.state.page, Page::Downloads);
    assert_eq!(rt.state.jobs.len(), 2);
    assert!(!rt.state.library_select_mode);

    // Batch remove supports Undo.
    let mut rt2 = runtime(fixtures::library_batch);
    tap(&mut rt2, "library-batch-remove");
    assert_eq!(rt2.state.history.len(), 2);
    assert!(rt2.state.undo.is_some());
    rt2.state.undo_last();
    assert_eq!(rt2.state.history.len(), 4);
}

