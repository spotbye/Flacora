use super::*;
use std::fs;
use crate::{
    fixtures,
    jobs::Status,
    runner::{JobEvent, JobHandle},
};

#[test]
fn a_tidal_link_offers_every_installed_tidal_extension() {
    let mut app = fixtures::app();
    app.url = TextEdit::new("https://tidal.com/track/441821360");
    let ids: Vec<_> = app.candidates().iter().map(|i| i.manifest.id.clone()).collect();
    assert_eq!(ids, ["flacdownloader", "odskyler"], "the established fallback order");
    assert_eq!(app.selected().unwrap().manifest.id, "flacdownloader");

    app.settings.preferred.insert("tidal".into(), "odskyler".into());
    assert_eq!(app.selected().unwrap().manifest.id, "odskyler", "the preferred extension wins");
}

#[test]
fn several_links_queue_in_order_and_unhandled_ones_are_reported() {
    let mut app = fixtures::app();
    app.url = TextEdit::new(
        "https://tidal.com/track/1\nhttps://music.amazon.com/tracks/B0FD256Q8F https://open.spotify.com/track/x https://example.org/a",
    );
    app.enqueue();
    let queued: Vec<_> = app.jobs.iter().map(|j| (j.extension.manifest.id.as_str(), j.status.clone())).collect();
    assert_eq!(queued, [("flacdownloader", Status::Queued), ("cyan-dl", Status::Queued), ("odesli", Status::Queued)]);
    assert_eq!(app.notice.as_ref().map(|note| note.message.as_str()), Some("No installed extension handles example.org."));
    assert!(app.url.text.is_empty(), "the box clears once links are queued");
}

#[test]
fn a_resolved_link_waits_for_the_user_to_choose_a_service() {
    let mut app = fixtures::app();
    app.installed.push(fixtures::installed("monochrome"));
    assert_eq!(app.selected_for("tracks.monochrome.st").map(|i| i.manifest.id.as_str()), Some("monochrome"));
    let id = fixtures::job(&mut app, "odesli", "https://open.spotify.com/track/1CPZ5BxNNd0n0nF4Orb9JS", Status::Running);
    app.jobs[0].handle = Some(JobHandle::detached());
    app.apply(Msg::Job(id, JobEvent::Line("Yandex Music: https://music.yandex.ru/track/1".into())));
    app.apply(Msg::Job(id, JobEvent::Line("Pandora: https://www.pandora.com/TR:1".into())));
    let links = r#"@flacora {"event":"links","links":{"Amazon Music":"https://music.amazon.com/tracks/B0FGGSTFL2","Deezer":"https://www.deezer.com/track/3412534581","Tidal":"https://listen.tidal.com/track/532554866","Monochrome":"https://tracks.monochrome.st/track/194491342917730304","Pandora":"https://www.pandora.com/TR:1","Yandex Music":"https://music.yandex.ru/track/1"}}"#;
    app.apply(Msg::Job(id, JobEvent::Line(links.into())));
    app.apply(Msg::Job(id, JobEvent::Exit(Ok(0))));
    let job = &app.jobs[0];
    assert_eq!(job.status, Status::Choosing, "nothing is picked for the user");
    assert!(job.links.iter().all(|(service, _)| !matches!(service.as_str(), "Yandex Music" | "Pandora")));
    assert!(job.log.iter().all(|line| !line.starts_with("Yandex Music:") && !line.starts_with("Pandora:")));
    let offered: Vec<(&str, &str)> = job.choices.iter().map(|c| (c.service.key, c.downloader.as_str())).collect();
    assert_eq!(offered, [("tidal", "flacdownloader"), ("deezer", "flacdownloader"), ("amazon", "cyan-dl"), ("monochrome", "monochrome")]);
    assert_eq!(app.active_jobs(), 1, "a job waiting for a choice still counts");

    app.choose(id, 3);
    let job = &app.jobs[0];
    assert_eq!(job.status, Status::Queued, "handed to the chosen downloader");
    assert_eq!(job.extension.manifest.id, "monochrome");
    assert_eq!(job.url, "https://tracks.monochrome.st/track/194491342917730304");
    assert_eq!(job.service.map(|s| s.key), Some("monochrome"));
    assert!(job.choices.is_empty());
    assert_eq!(job.resolved.as_ref().map(|(name, from)| (name.as_str(), from.map(|s| s.key))), Some(("Odesli", Some("spotify"))));
    assert!(app.history.iter().all(|e| e.url != "https://open.spotify.com/track/1CPZ5BxNNd0n0nF4Orb9JS"), "nothing saved yet");
}

#[test]
fn batch_service_choices_apply_only_to_resolver_jobs_in_the_chosen_draft() {
    let mut app = fixtures::app();
    let tidal = crate::services::detect("tidal.com").unwrap();
    app.url = TextEdit::new("https://open.spotify.com/track/1 https://music.amazon.com/tracks/B00GZKCOQM");
    app.set_batch_service(Some(tidal));
    app.enqueue();
    assert_eq!(app.jobs[0].batch_service, Some(tidal));
    assert!(app.jobs[1].batch_service.is_none());
    assert_eq!(app.jobs[1].service.map(|service| service.key), Some("amazon"));

    app.url = TextEdit::new("https://open.spotify.com/track/2 https://open.spotify.com/track/3");
    app.set_batch_service(Some(tidal));
    app.url = TextEdit::new("https://open.spotify.com/track/4 https://open.spotify.com/track/5");
    app.enqueue();
    assert!(app.jobs[2..].iter().all(|job| job.batch_service.is_none()));
}

#[test]
fn a_batch_choice_waits_until_extension_installation_finishes() {
    let mut app = fixtures::app();
    let id = fixtures::job(&mut app, "odesli", "https://open.spotify.com/track/1", Status::Running);
    app.jobs[0].batch_service = crate::services::detect("tidal.com");
    app.importing = 1;
    app.job_event(id, JobEvent::Line(r#"@flacora {"event":"links","links":{"Tidal":"https://tidal.com/track/123"}}"#.into()));
    app.job_event(id, JobEvent::Exit(Ok(0)));
    assert_eq!(app.jobs[0].status, Status::Choosing);
    app.apply(Msg::Imported("odesli.fext".into(), Ok(Box::new(fixtures::installed("odesli")))));
    assert_eq!(app.jobs[0].status, Status::Queued);
    assert_eq!(app.jobs[0].extension.manifest.id, "flacdownloader");
}

#[test]
fn with_two_resolvers_the_preferred_one_takes_the_link() {
    let mut app = fixtures::app();
    app.installed.push(fixtures::installed("squigly"));
    let spotify = "open.spotify.com";
    let ids = |app: &App| app.resolvers_for(spotify).iter().map(|i| i.manifest.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&app), ["odesli", "squigly"], "the established fallback order");
    assert_eq!(app.resolver_for(spotify).map(|i| i.manifest.id.as_str()), Some("odesli"));
    app.set_preferred(crate::app::RESOLVER_KEY, Some("squigly".into()));
    assert_eq!(app.resolver_for(spotify).map(|i| i.manifest.id.as_str()), Some("squigly"));
    assert!(app.resolver_for("tidal.com").is_none(), "a downloader still takes Tidal links");
    app.installed.retain(|i| i.manifest.id != "squigly");
    assert_eq!(app.resolver_for(spotify).map(|i| i.manifest.id.as_str()), Some("odesli"), "a removed preference falls back");
}

#[test]
fn a_job_waiting_for_a_choice_can_be_cancelled() {
    let mut app = fixtures::app();
    let id = fixtures::job(&mut app, "odesli", "https://open.spotify.com/track/x", Status::Running);
    app.jobs[0].handle = Some(JobHandle::detached());
    app.apply(Msg::Job(id, JobEvent::Line(r#"@flacora {"event":"links","links":{"Tidal":"https://listen.tidal.com/track/1"}}"#.into())));
    app.apply(Msg::Job(id, JobEvent::Exit(Ok(0))));
    assert_eq!(app.jobs[0].status, Status::Choosing, "even a single service is offered, not taken");
    app.cancel_job(id);
    assert_eq!(app.jobs[0].status, Status::Cancelled);
    app.choose(id, 0);
    assert_eq!(app.jobs[0].status, Status::Cancelled, "a cancelled job cannot be chosen any more");
}

#[test]
fn a_resolved_link_nobody_downloads_fails_with_what_was_found() {
    let mut app = fixtures::app();
    let id = fixtures::job(&mut app, "odesli", "https://open.spotify.com/track/x", Status::Running);
    app.jobs[0].handle = Some(JobHandle::detached());
    app.apply(Msg::Job(id, JobEvent::Line(r#"@flacora {"event":"links","links":{"Bandcamp":"https://artist.bandcamp.com/track/song"}}"#.into())));
    app.apply(Msg::Job(id, JobEvent::Exit(Ok(0))));
    assert_eq!(app.jobs[0].status, Status::Failed("Found on Bandcamp, but no installed extension downloads from there".into()));
}

#[test]
fn a_service_an_extension_reports_down_is_not_routed_to_it() {
    let mut app = fixtures::app();
    app.installed.push(fixtures::installed("lucida"));
    let ids = |app: &App| app.candidates_for("tidal.com").iter().map(|i| i.manifest.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&app), ["flacdownloader", "odskyler", "lucida"]);
    app.checking.insert("lucida".into(), 1);
    app.apply(Msg::ExtensionStatus {
        id: "lucida".into(), check: 1,
        status: ExtensionStatus::Services(vec!["Qobuz".into(), "Amazon Music".into()]),
    });
    assert_eq!(ids(&app), ["flacdownloader", "odskyler"], "Lucida's Tidal is down");
    assert_eq!(app.services_of("lucida", &[]), ["Qobuz", "Amazon Music"]);
    assert!(app.candidates_for("open.qobuz.com").iter().any(|i| i.manifest.id == "lucida"));
    assert!(app.resolver_for("tidal.com").is_none(), "a downloader takes Tidal links, so no resolver");
}

#[test]
fn stale_status_results_do_not_replace_a_newer_check() {
    let mut app = fixtures::app();
    let id = "cyan-dl".to_owned();
    app.checking.insert(id.clone(), 2);
    app.apply(Msg::ExtensionStatus { id: id.clone(), check: 1, status: ExtensionStatus::Online });
    assert_eq!(app.checking.get(&id), Some(&2));
    assert!(!app.extension_status.contains_key(&id));

    app.apply(Msg::ExtensionStatus { id: id.clone(), check: 2, status: ExtensionStatus::Offline("Timed out.".into()) });
    assert!(!app.checking.contains_key(&id));
    assert!(app.candidates_for("music.amazon.com").iter().any(|i| i.manifest.id == id));

    app.apply(Msg::Removed(id.clone(), Ok(())));
    assert!(!app.extension_status.contains_key(&id));
}

#[test]
fn partial_bulk_removal_keeps_failed_extensions_installed() {
    let mut app = fixtures::app();
    app.removing.extend(["cyan-dl".into(), "odesli".into()]);
    app.apply(Msg::ExtensionsRemoved(vec![
        ("cyan-dl".into(), Ok(())),
        ("odesli".into(), Err("Could not remove odesli".into())),
    ]));
    assert!(app.removing.is_empty());
    assert!(!app.installed.iter().any(|item| item.manifest.id == "cyan-dl"));
    assert!(app.installed.iter().any(|item| item.manifest.id == "odesli"));
    assert!(app.notice.as_ref().is_some_and(|notice| notice.message.contains("Removed 1 of 2 extensions")));
}

fn running(app: &mut App) -> u64 {
    let id = fixtures::job(app, "odskyler", "https://tidal.com/track/441821360", Status::Running);
    app.jobs.last_mut().unwrap().handle = Some(JobHandle::detached());
    id
}

#[test]
fn events_fill_in_the_job_and_plain_lines_go_to_its_log() {
    let mut app = fixtures::app();
    let id = running(&mut app);
    let line = |l: &str| Msg::Job(id, JobEvent::Line(l.into()));
    app.apply(line(r#"@flacora {"event":"track","title":"Golden","artist":"HUNTR/X","album":"KPop"}"#));
    app.apply(line(r#"@flacora {"event":"quality","text":"FLAC 24-bit / 48 kHz"}"#));
    app.apply(line(r#"@flacora {"event":"stage","text":"Downloading audio"}"#));
    app.apply(line(r#"@flacora {"event":"progress","value":0.5}"#));
    app.apply(line("Manifest: HTTP 200 OK"));
    app.apply(Msg::Job(99, JobEvent::Line("from another job".into())));
    let job = &app.jobs[0];
    assert_eq!(job.title.as_deref(), Some("Golden"));
    assert_eq!(job.quality.as_deref(), Some("FLAC 24-bit / 48 kHz"));
    assert_eq!((job.stage.as_deref(), job.progress), (Some("Downloading audio"), Some(0.5)));
    assert_eq!(job.log, ["Manifest: HTTP 200 OK"]);
}

#[test]
fn a_finished_job_is_saved_to_the_library() {
    let mut app = fixtures::app();
    app.history.clear();
    let id = running(&mut app);
    app.apply(Msg::Job(id, JobEvent::Line(r#"@flacora {"event":"track","title":"Golden","artist":"HUNTR/X","album":"KPop"}"#.into())));
    app.apply(Msg::Job(id, JobEvent::Line(r#"@flacora {"event":"saved","path":"C:\\Music\\HUNTR_X - Golden.flac"}"#.into())));
    app.apply(Msg::Job(id, JobEvent::Exit(Ok(0))));
    assert_eq!(app.jobs[0].status, Status::Done);
    let entry = &app.history[0];
    assert_eq!((entry.title.as_str(), entry.service.as_str(), entry.extension.as_str()), ("Golden", "Tidal", "Rhythmax"));
    assert_eq!(entry.main_file(), Some("C:\\Music\\HUNTR_X - Golden.flac"));
}

#[test]
fn failures_explain_themselves_and_cancelled_jobs_say_so() {
    let mut app = fixtures::app();
    let id = running(&mut app);
    app.apply(Msg::Job(id, JobEvent::Line("Error: FLAC CD is not available".into())));
    app.apply(Msg::Job(id, JobEvent::Exit(Ok(1))));
    assert_eq!(app.jobs[0].status, Status::Failed("FLAC CD is not available".into()));

    let id = running(&mut app);
    app.cancel_job(id);
    app.apply(Msg::Job(id, JobEvent::Exit(Ok(1))));
    assert_eq!(app.jobs[1].status, Status::Cancelled);

    app.retry_job(id);
    assert_eq!(app.jobs[1].status, Status::Queued, "retry queues it again");
}

#[test]
fn dropped_packages_are_processed_in_order_and_update_the_installed_list() {
    let mut app = fixtures::app();
    let before = app.installed.len();
    let lucida = PathBuf::from("lucida.fext");
    let invalid = PathBuf::from("invalid.zip");
    app.install_files(&[lucida.clone(), invalid.clone()]);
    assert_eq!(app.importing, 2);
    assert_eq!(app.pending_imports.front(), Some(&invalid), "the next package waits for the first");
    app.apply(Msg::Imported(lucida, Ok(Box::new(fixtures::installed("lucida")))));
    assert_eq!(app.installed.len(), before + 1);
    assert_eq!(app.importing, 1);
    assert!(app.pending_imports.is_empty(), "the next package starts after the first reports its result");
    app.apply(Msg::Imported(invalid, Err("Invalid package".into())));
    assert_eq!(app.notice.as_ref().map(|note| note.message.as_str()), Some("invalid.zip: Invalid package"));
    assert_eq!(app.importing, 0);
}

#[test]
fn multiple_picked_packages_enter_the_same_install_queue() {
    let mut app = fixtures::app();
    let first = PathBuf::from("lucida.fext");
    let second = PathBuf::from("doubledouble.FEXT");
    app.browsing_packages = true;
    app.apply(Msg::PackagesPicked(vec![first.clone(), PathBuf::from("legacy.flacora"), second.clone()]));
    assert!(!app.browsing_packages);
    assert_eq!(app.importing, 2);
    assert_eq!(app.pending_imports.front(), Some(&second));
    assert_eq!(app.notice.as_ref().unwrap().message, "Select only .fext extension packages.");

    app.apply(Msg::Imported(first, Ok(Box::new(fixtures::installed("lucida")))));
    app.apply(Msg::Imported(second, Ok(Box::new(fixtures::installed("doubledouble")))));
    assert_eq!(app.importing, 0);
    assert!(app.installed.iter().any(|item| item.manifest.id == "lucida"));
    assert!(app.installed.iter().any(|item| item.manifest.id == "doubledouble"));
}

#[test]
fn library_search_matches_title_artist_album_and_service() {
    let mut app = fixtures::app();
    app.search = TextEdit::new("deezer");
    assert_eq!(app.filtered_history().len(), 1);
    app.search = TextEdit::new("huntr");
    assert_eq!(app.filtered_history().len(), app.history.len());
}

#[test]
fn validate_saved_files_enforces_output_containment_and_blocks_executables() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("out");
    fs::create_dir_all(&output).unwrap();
    let track = output.join("Golden.flac");
    fs::write(&track, b"fLaC").unwrap();
    let outside = dir.path().join("other.flac");
    fs::write(&outside, b"fLaC").unwrap();
    let exe = output.join("bad.exe");
    fs::write(&exe, b"MZ").unwrap();

    let ok = crate::jobs::validate_saved_files(Some(&output), &[track.to_string_lossy().into_owned()], true).unwrap();
    assert_eq!(ok.len(), 1);
    assert!(crate::jobs::validate_saved_files(Some(&output), &[outside.to_string_lossy().into_owned()], true).is_err());
    assert!(crate::jobs::validate_saved_files(Some(&output), &[exe.to_string_lossy().into_owned()], true).is_err());
    assert!(crate::jobs::validate_saved_files(None, &[track.to_string_lossy().into_owned()], true).is_err());
    assert!(crate::jobs::validate_saved_files(Some(&dir.path().join("missing-dir")), &[track.to_string_lossy().into_owned()], true).is_err());
}

#[test]
fn partial_track_events_do_not_erase_metadata_and_undo_merges_without_clobbering_new_entries() {
    let mut app = fixtures::app();
    let id = running(&mut app);
    app.apply(Msg::Job(id, JobEvent::Line(r#"@flacora {"event":"track","title":"Golden","artist":"HUNTR/X","album":"KPop"}"#.into())));
    app.apply(Msg::Job(id, JobEvent::Line(r#"@flacora {"event":"track","title":"","artist":"","album":"","cover":"https://example.com/c.jpg"}"#.into())));
    assert_eq!(app.jobs[0].title.as_deref(), Some("Golden"));
    assert_eq!(app.jobs[0].artist.as_deref(), Some("HUNTR/X"));
    assert_eq!(app.jobs[0].album.as_deref(), Some("KPop"));

    let initial_len = app.history.len();
    app.remove_entry(0);
    assert_eq!(app.history.len(), initial_len - 1);
    let new_entry = crate::library::Entry {
        title: "Newly Downloaded".into(),
        url: "https://tidal.com/track/999".into(),
        files: vec!["C:/Music/New.flac".into()],
        date: crate::jobs::now() + 10,
        ..Default::default()
    };
    app.history.insert(0, new_entry.clone());
    app.undo_last();
    assert_eq!(app.history.len(), initial_len + 1);
    assert!(app.history.iter().any(|e| e.title == "Newly Downloaded"));
}

#[test]
fn offline_downloaders_rank_behind_healthy_candidates_and_move_to_top_reorders_queue() {
    let mut app = fixtures::app();
    app.extension_status.insert("flacdownloader".into(), ExtensionStatus::Offline("Timeout".into()));
    let ids: Vec<_> = app.candidates_for("tidal.com").iter().map(|i| i.manifest.id.clone()).collect();
    assert_eq!(ids, ["odskyler", "flacdownloader"]);

    let j1 = fixtures::job(&mut app, "odskyler", "https://tidal.com/track/1", Status::Queued);
    let j2 = fixtures::job(&mut app, "odskyler", "https://tidal.com/track/2", Status::Queued);
    app.move_job_to_top(j2);
    assert_eq!(app.jobs[0].id, j2);
    assert_eq!(app.jobs[1].id, j1);
}

#[test]
fn image_cache_evicts_oldest_entries_above_the_bound_and_keeps_active_job_covers() {
    let mut app = fixtures::app();
    app.images(std::sync::Arc::new(|| {}));
    let id = running(&mut app);
    let pinned = "cover-0.png".to_owned();
    app.jobs.iter_mut().find(|j| j.id == id).unwrap().cover = Some(pinned.clone());

    for i in 0..(App::IMAGE_CACHE_LIMIT + 10) {
        app.load_image(format!("cover-{i}.png"), Vec::new());
    }
    assert_eq!(app.images.len(), App::IMAGE_CACHE_LIMIT);
    assert!(app.images.contains_key(&pinned), "active job cover stays cached");
    assert!(!app.images.contains_key("cover-1.png"), "oldest unpinned cover is evicted");
    assert!(app.images.contains_key(&format!("cover-{}.png", App::IMAGE_CACHE_LIMIT + 9)));
}

#[test]
fn job_validated_completes_downloader_and_lookup_jobs_off_the_ui_thread() {
    let mut app = fixtures::app();
    app.history.clear();
    let id = running(&mut app);
    app.jobs[0].title = Some("Golden".into());
    app.apply(Msg::JobValidated(id, Ok(vec!["C:/Music/Golden.flac".into()])));
    assert_eq!(app.jobs[0].status, Status::Done);
    assert_eq!(app.history.len(), 1);
    assert_eq!(app.history[0].main_file(), Some("C:/Music/Golden.flac"));

    let failed_id = running(&mut app);
    app.apply(Msg::JobValidated(failed_id, Err("The extension reported a missing saved file".into())));
    assert_eq!(app.jobs[1].status, Status::Failed("The extension reported a missing saved file".into()));
}

#[test]
fn completed_downloads_can_queue_a_matching_web_tagger() {
    for (enabled, version, writes_tags, expected) in [
        (false, "1.0.1", true, false),
        (true, "1.0.0", true, false),
        (true, "1.0.1", false, false),
        (true, "1.0.1", true, true),
    ] {
        let mut app = fixtures::app();
        app.history.clear();
        app.settings.auto_tag = enabled;
        let mut tidal = fixtures::installed("tidal-web");
        tidal.manifest.version = version.into();
        tidal.manifest.writes_tags = writes_tags;
        app.installed = vec![tidal, fixtures::installed("deezer-web")];
        let id = running(&mut app);
        let file = std::path::absolute("test.flac").unwrap();
        let saved = file.to_string_lossy().into_owned();
        app.apply(Msg::JobValidated(id, Ok(vec![saved.clone()])));
        assert_eq!(app.jobs.len(), if expected { 2 } else { 1 });
        assert_eq!(app.jobs[0].status, Status::Done);
        assert_eq!(app.history[0].main_file(), Some(saved.as_str()));
        if expected {
            assert_eq!(app.jobs[1].extension.manifest.id, "tidal-web");
            assert_eq!(app.jobs[1].lookup_file.as_ref(), Some(&file));
            assert_eq!(app.jobs[1].url, app.jobs[0].url);
            let tag_id = app.jobs[1].id;
            app.jobs[1].status = Status::Running;
            app.apply(Msg::TagsWritten(tag_id, Ok(library::TaggedTrack {
                file: saved, title: "Tagged Title".into(), artist: "Tagged Artist".into(),
                album: "Tagged Album".into(), cover: None,
            })));
            assert_eq!(app.jobs[1].status, Status::Done);
            assert_eq!(app.history[0].title, "Tagged Title");
            assert_eq!(app.history[0].artist, "Tagged Artist");
            assert_eq!(app.history[0].album, "Tagged Album");
        }
    }
}
