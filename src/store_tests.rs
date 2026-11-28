use super::*;
use std::io::Write;

#[test]
fn status_checks_distinguish_outages_from_inconclusive_responses() {
    assert_eq!(http_status(StatusCode::OK), ExtensionStatus::Online);
    assert!(matches!(http_status(StatusCode::SERVICE_UNAVAILABLE), ExtensionStatus::Offline(_)));
    for status in [StatusCode::UNAUTHORIZED, StatusCode::FORBIDDEN, StatusCode::TOO_MANY_REQUESTS] {
        assert!(matches!(http_status(status), ExtensionStatus::Unknown(_)));
    }
    assert_eq!(reported_health("betterlyrics", &serde_json::json!({"status": "degraded"})), Some(ExtensionStatus::Degraded));
    assert!(matches!(reported_health("betterlyrics", &serde_json::json!({"status": "unhealthy"})), Some(ExtensionStatus::Offline(_))));
}

#[test]
fn musicbrainz_requires_a_recording_in_the_status_response() {
    assert_eq!(reported_health("musicbrainz", &serde_json::json!({"recordings":[{"id":"recording-id"}]})), Some(ExtensionStatus::Online));
    assert_eq!(reported_health("musicbrainz", &serde_json::json!({"recordings":[]})), None);
}

#[test]
fn musicbrainz_retries_respect_the_api_interval_and_retry_after() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let date = httpdate::fmt_http_date(now + Duration::from_secs(5));
    assert_eq!(musicbrainz_retry_delay(Some(&date), now), Some(Duration::from_secs(5)));
    assert_eq!(musicbrainz_retry_delay(Some("0"), now), Some(MUSICBRAINZ_INTERVAL));
    assert_eq!(musicbrainz_retry_delay(Some("15"), now), None);
    assert_eq!(musicbrainz_retry_delay(None, now), Some(Duration::from_secs(3)));
}

#[test]
fn download_layout_and_automatic_tagging_default_for_old_settings_and_persist() {
    let (_guard, paths) = temp_root("layout");
    fs::write(paths.settings(), r#"{"output":"test-downloads"}"#).unwrap();
    let mut settings = load_settings(&paths);
    assert_eq!(settings.layout, crate::output::Layout::default());
    assert!(!settings.auto_tag);
    settings.layout = crate::output::Layout { filename: crate::output::Filename::TitleArtist, folders: crate::output::Folders::ArtistAlbum };
    settings.auto_tag = true;
    save_settings(&paths, &settings).unwrap();
    assert_eq!(load_settings(&paths), settings);
}

#[test]
fn theme_presets_persist_independently_of_existing_color_modes() {
    let (_guard, paths) = temp_root("theme");
    for json in [r#"{"theme":"dark"}"#, r#"{"theme":"dark","theme_preset":"unknown"}"#] {
        fs::write(paths.settings(), json).unwrap();
        let settings = load_settings(&paths);
        assert_eq!(settings.theme, Theme::Dark);
        assert_eq!(settings.theme_preset, ThemePreset::Neutral);
    }
    let mut settings = load_settings(&paths);
    for preset in ThemePreset::ALL {
        settings.theme_preset = preset;
        save_settings(&paths, &settings).unwrap();
        assert_eq!(load_settings(&paths), settings);
    }
}

/// A fresh Flacora data folder that cleans itself up on drop.
fn temp_root(name: &str) -> (tempfile::TempDir, Paths) {
    let dir = tempfile::Builder::new().prefix(&format!("flacora-test-{name}-")).tempdir().unwrap();
    let paths = Paths { root: dir.path().to_path_buf() };
    (dir, paths)
}

/// Writes a package with the given manifest version and, optionally, the executable.
fn package(dir: &Path, version: &str, with_exe: bool) -> PathBuf {
    let mut manifest = crate::fixtures::manifest_json("odskyler").clone();
    manifest["version"] = version.into();
    package_with_manifest(dir, &format!("odskyler-{version}"), &manifest.to_string(), with_exe)
}

fn package_with_manifest(dir: &Path, name: &str, manifest: &str, with_exe: bool) -> PathBuf {
    let exe = crate::extensions::parse_manifest(manifest).unwrap().executable[crate::extensions::platform()].clone();
    let path = dir.join(format!("{name}.{PACKAGE}"));
    let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("extension.json", options).unwrap();
    zip.write_all(manifest.as_bytes()).unwrap();
    if with_exe {
        zip.start_file(exe.as_str(), options).unwrap();
        zip.write_all(b"not really a program").unwrap();
    }
    zip.finish().unwrap();
    path
}

#[test]
fn web_package_replaces_its_legacy_metadata_install_only_when_idle() {
    let (_guard, paths) = temp_root("web-rename");
    let web = crate::fixtures::manifest_json("spotify-web").to_string();
    let legacy = web.replace("spotify-web", "spotify-metadata");
    let old_package = package_with_manifest(&paths.root, "old-spotify", &legacy, true);
    install_file(&paths, &old_package, |_| true).unwrap();
    let saved_data = paths.data("spotify-metadata");
    fs::create_dir_all(&saved_data).unwrap();
    fs::write(saved_data.join("marker"), b"keep").unwrap();

    let new_package = package_with_manifest(&paths.root, "new-spotify", &web, true);
    assert!(install_file(&paths, &new_package, |id| id != "spotify-metadata").unwrap_err().contains("in use"));
    let broken = package_with_manifest(&paths.root, "broken-spotify", &web, false);
    assert!(install_file(&paths, &broken, |_| true).unwrap_err().contains("missing its executable"));
    assert_eq!(load_installed(&paths)[0].manifest.id, "spotify-metadata");
    install_file(&paths, &new_package, |_| true).unwrap();
    let installed = load_installed(&paths);
    assert_eq!(installed.len(), 1);
    assert_eq!(installed[0].manifest.id, "spotify-web");
    assert!(saved_data.join("marker").is_file());
    assert_eq!(fs::read_dir(paths.extensions()).unwrap().count(), 1, "superseded backup cleaned up");
    assert!(install_file(&paths, &old_package, |_| true).unwrap_err().contains("spotify-web"));
}

#[test]
fn universal_packages_install_and_select_the_current_platform_executable() {
    let (_guard, paths) = temp_root("universal");
    let platforms = ["windows-x64", "linux-x64", "linux-arm64", "macos-x64", "macos-arm64"];
    let mut manifest = crate::fixtures::manifest_json("odskyler").clone();
    let executables: serde_json::Map<String, serde_json::Value> = platforms.iter().map(|platform| {
        let name = if *platform == "windows-x64" { "odskyler.exe" } else { "odskyler" };
        ((*platform).to_owned(), format!("bin/{platform}/{name}").into())
    }).collect();
    manifest["executable"] = executables.clone().into();
    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    archive.start_file("extension.json", zip::write::SimpleFileOptions::default()).unwrap();
    archive.write_all(manifest.to_string().as_bytes()).unwrap();
    for platform in platforms {
        archive.start_file(executables[platform].as_str().unwrap(), zip::write::SimpleFileOptions::default().unix_permissions(0o755)).unwrap();
        archive.write_all(platform.as_bytes()).unwrap();
    }
    let mut payload = archive.finish().unwrap().into_inner();
    let nonce = [0; 12];
    Aes256Gcm::new(FEXT_KEY.into()).encrypt_in_place(&Nonce::from(nonce), FEXT_HEADER, &mut payload).unwrap();
    let mut envelope = FEXT_HEADER.to_vec();
    envelope.extend_from_slice(&nonce);
    envelope.extend_from_slice(&payload);
    let package = paths.root.join("odskyler-1.0.1.fext");
    fs::write(&package, envelope).unwrap();
    let installed = install_file(&paths, &package, |_| true).unwrap();
    let platform = crate::extensions::platform();
    assert_eq!(fs::read(installed.executable().unwrap()).unwrap(), platform.as_bytes());
    for relative in installed.manifest.executable.values() {
        assert!(installed.dir.join(relative).is_file());
    }
    let reloaded = load_installed(&paths);
    assert_eq!(reloaded[0].executable(), installed.executable());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_ne!(fs::metadata(installed.executable().unwrap()).unwrap().permissions().mode() & 0o111, 0);
    }
}

#[test]
fn a_package_file_installs_and_a_newer_one_replaces_it() {
    let (_guard, paths) = temp_root("install");
    let first = install_file(&paths, &package(&paths.root, "0.1.0", true), |_| true).unwrap();
    assert_eq!(first.manifest.id, "odskyler");
    assert!(first.executable().unwrap().is_file());

    let update = package(&paths.root, "0.2.0", true);
    let nonce = [0; 12];
    let mut payload = fs::read(&update).unwrap();
    Aes256Gcm::new(FEXT_KEY.into())
        .encrypt_in_place(&Nonce::from(nonce), FEXT_HEADER, &mut payload)
        .unwrap();
    let mut envelope = FEXT_HEADER.to_vec();
    envelope.extend_from_slice(&nonce);
    envelope.extend_from_slice(&payload);
    fs::write(&update, &envelope).unwrap();
    install_file(&paths, &update, |_| true).unwrap();
    let installed = load_installed(&paths);
    assert_eq!(installed.len(), 1, "staging folders are not read as extensions");
    assert_eq!(installed[0].manifest.version, "0.2.0");

    let last = envelope.last_mut().unwrap();
    *last ^= 1;
    fs::write(&update, envelope).unwrap();
    let error = install_file(&paths, &update, |_| true).unwrap_err();
    assert!(error.contains("Could not decrypt this package"), "{error}");
    assert_eq!(load_installed(&paths)[0].manifest.version, "0.2.0");
    assert_eq!(fs::read_dir(paths.extensions()).unwrap().count(), 1);
}

#[test]
fn web_packages_below_the_tag_writer_minimum_are_refused() {
    let (_guard, paths) = temp_root("tag-writer-version");
    let mut manifest = crate::fixtures::manifest_json("tidal-web").clone();
    for version in ["1.0.0", "1.0.1-rc.1"] {
        manifest["version"] = version.into();
        manifest["writes_tags"] = false.into();
        let old = package_with_manifest(&paths.root, version, &manifest.to_string(), true);
        assert!(install_file(&paths, &old, |_| true).unwrap_err().contains("v1.0.1"));
        assert!(load_installed(&paths).is_empty());
    }
    manifest["version"] = "1.0.1".into();
    manifest["writes_tags"] = true.into();
    let current = package_with_manifest(&paths.root, "1.0.1", &manifest.to_string(), true);
    let installed = install_file(&paths, &current, |_| true).unwrap();
    assert!(installed.manifest.can_write_tags());
}

#[test]
fn installed_browser_ignores_staging_and_incomplete_folders() {
    let (_guard, paths) = temp_root("browser");
    let staging = paths.browser().join("chromium-123.4.tmp").join(CLOAKBROWSER_EXECUTABLE);
    fs::create_dir_all(staging.parent().unwrap()).unwrap();
    fs::write(&staging, b"test browser").unwrap();
    let installed = paths.browser().join("chromium-123.4");
    fs::create_dir_all(&installed).unwrap();
    assert_eq!(installed_browser(&paths.browser()), None);

    let executable = installed.join(CLOAKBROWSER_EXECUTABLE);
    fs::create_dir_all(executable.parent().unwrap()).unwrap();
    fs::write(executable, b"test browser").unwrap();
    assert_eq!(installed_browser(&paths.browser()), Some("123.4".into()));
}

#[test]
fn ffmpeg_reports_its_version_without_build_labels_or_copyright() {
    assert_eq!(ffmpeg_version("ffmpeg version 7.1.1 Copyright (c) the FFmpeg developers"), Some("7.1.1"));
    assert_eq!(ffmpeg_version("ffmpeg version 9.0-essentials_build-www.gyan.dev\nconfiguration: ..."), Some("9.0"));
    assert_eq!(ffmpeg_version("ffmpeg version N-118123-gabc123 Copyright (c)"), Some("N-118123-gabc123"));
}

#[test]
fn a_package_without_its_executable_leaves_the_installed_one_alone() {
    let (_guard, paths) = temp_root("broken");
    install_file(&paths, &package(&paths.root, "0.1.0", true), |_| true).unwrap();
    let error = install_file(&paths, &package(&paths.root, "0.2.0", false), |_| true).unwrap_err();
    assert!(error.contains("missing its executable"), "{error}");
    assert_eq!(load_installed(&paths)[0].manifest.version, "0.1.0");
    assert_eq!(fs::read_dir(paths.extensions()).unwrap().count(), 1, "nothing left behind");
}

#[test]
fn a_dropped_package_cannot_replace_an_extension_in_use() {
    let (_guard, paths) = temp_root("in-use");
    install_file(&paths, &package(&paths.root, "0.1.0", true), |_| true).unwrap();
    let error = install_file(&paths, &package(&paths.root, "0.2.0", true), |id| id != "odskyler").unwrap_err();
    assert!(error.contains("is in use"), "{error}");
    assert_eq!(load_installed(&paths)[0].manifest.version, "0.1.0");
}

#[test]
fn a_file_that_is_not_a_package_is_refused() {
    let (_guard, paths) = temp_root("junk");
    let junk = paths.root.join("song.fext");
    fs::write(&junk, b"just some bytes").unwrap();
    assert_eq!(install_file(&paths, &junk, |_| true).unwrap_err(), "This is not a Flacora extension package");
}

#[test]
fn packages_with_symlinks_or_excessive_size_are_rejected() {
    let (_guard, paths) = temp_root("zip-security");
    let symlink_pkg = paths.root.join("symlink.fext");
    {
        let mut zip = zip::ZipWriter::new(File::create(&symlink_pkg).unwrap());
        zip.add_symlink("odskyler.exe", "/bin/sh", zip::write::SimpleFileOptions::default()).unwrap();
        zip.finish().unwrap();
    }
    let error = install_file(&paths, &symlink_pkg, |_| true).unwrap_err();
    assert!(error.contains("symbolic link"), "{error}");

    let big_pkg = paths.root.join("big.fext");
    {
        let mut zip = zip::ZipWriter::new(File::create(&big_pkg).unwrap());
        zip.start_file("payload.bin", zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(&[0u8; 64]).unwrap();
        zip.finish().unwrap();
    }
    let target = paths.root.join("out");
    let error = unpack_with_limits(&big_pkg, &target, 10, 32).unwrap_err();
    assert!(error.contains("maximum allowed size"), "{error}");

    let many_pkg = paths.root.join("many.fext");
    {
        let mut zip = zip::ZipWriter::new(File::create(&many_pkg).unwrap());
        for i in 0..5 {
            zip.start_file(format!("f{i}.txt"), zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(b"x").unwrap();
        }
        zip.finish().unwrap();
    }
    let error = unpack_with_limits(&many_pkg, &paths.root.join("out2"), 3, 1024).unwrap_err();
    assert!(error.contains("too many files"), "{error}");

    let reserved_pkg = paths.root.join("reserved.fext");
    {
        let mut zip = zip::ZipWriter::new(File::create(&reserved_pkg).unwrap());
        zip.start_file("CON.txt", zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(b"x").unwrap();
        zip.finish().unwrap();
    }
    let error = unpack_with_limits(&reserved_pkg, &paths.root.join("out3"), 10, 1024).unwrap_err();
    assert!(error.contains("unsafe path"), "{error}");
}

#[test]
fn settings_clamp_concurrency_and_ignore_empty_output() {
    let (_guard, paths) = temp_root("concurrency");
    fs::write(paths.settings(), r#"{"output":"   ","max_concurrent_jobs":99}"#).unwrap();
    let settings = load_settings(&paths);
    assert_eq!(settings.output, default_output());
    assert_eq!(settings.max_concurrent_jobs, 4);

    let stale = paths.extensions().join(".import-abandoned");
    fs::create_dir_all(&stale).unwrap();
    cleanup_stale_temps(&paths);
    assert!(!stale.exists());
}

#[test]
fn installed_browser_prefers_newest_version() {
    let (_guard, paths) = temp_root("browser-order");
    for version in ["140.0.1.0", "145.0.7632.159.7", "142.0.5.0"] {
        let exe = paths.browser().join(format!("chromium-{version}")).join(CLOAKBROWSER_EXECUTABLE);
        fs::create_dir_all(exe.parent().unwrap()).unwrap();
        fs::write(exe, b"bin").unwrap();
    }
    assert_eq!(installed_browser(&paths.browser()), Some("145.0.7632.159.7".into()));
}
