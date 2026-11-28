use super::*;

const MANIFEST: &str = r#"{
    "id": "odskyler", "name": "Rhythmax", "version": "0.1.0",
    "services": ["Tidal"], "hosts": ["tidal.com"],
    "executable": {"windows-x64": "odskyler.exe", "linux-x64": "odskyler"},
    "args": ["{url}", "--output", "{output}"],
    "options": [{
        "key": "quality", "label": "Quality", "default": "hires",
        "choices": [{"value": "lossless", "label": "Lossless"}, {"value": "hires", "label": "Hi-Res"}],
        "args": ["--quality", "{value}"]
    }],
    "env": {"ODSKYLER_HOME": "{data}"}
}"#;

fn fill() -> Fill<'static> {
    Fill { url: "https://tidal.com/track/1", file: "", output: "C:/Music", data: "C:/data/odskyler", browser: "C:/data/cloakbrowser" }
}

#[test]
fn every_fixture_manifest_has_a_status_source() {
    for json in crate::fixtures::MANIFESTS.iter() {
        let manifest = parse_manifest(&json.to_string()).unwrap();
        assert!(!manifest.status.is_empty() || status_url(&manifest).is_some(), "{} has a status source", manifest.id);
    }
    let spotify = crate::fixtures::manifest("spotify-web");
    let mut legacy = spotify.clone();
    legacy.id = "spotify-metadata".into();
    legacy.status.clear();
    assert_eq!(status_args(&spotify, &fill()), ["--status"]);
    assert_eq!(status_url(&legacy), status_url(&spotify));
    assert_eq!(status_url(&legacy).unwrap().host_str(), Some("open.spotify.com"));
}

#[test]
fn third_party_status_endpoints_use_https_and_can_override_the_domain() {
    let mut data: serde_json::Value = serde_json::from_str(MANIFEST).unwrap();
    data["id"] = "third-party".into();
    data["description"] = "example.org".into();
    let parse = |data: &serde_json::Value| parse_manifest(&data.to_string());
    assert_eq!(status_url(&parse(&data).unwrap()).unwrap().as_str(), "https://example.org/");
    data["status_url"] = "https://api.example.org/health".into();
    assert_eq!(status_url(&parse(&data).unwrap()).unwrap().as_str(), "https://api.example.org/health");
    for invalid in ["http://api.example.org/health", "https://user:password@example.org", "https://example.org/#health"] {
        data["status_url"] = invalid.into();
        assert!(parse(&data).is_err());
    }
}

#[test]
fn manifest_parses_with_options_and_defaults() {
    let m = parse_manifest(MANIFEST).unwrap();
    assert_eq!(m.id, "odskyler");
    assert_eq!(m.options[0].default_index(), 1);
    assert!(!m.browser);
}

#[test]
fn old_manifests_drop_excluded_services_while_retaining_other_routes() {
    let mut data = crate::fixtures::manifest_json("lucida").clone();
    data["hosts"].as_array_mut().unwrap().push("music.yandex.ru".into());
    data["hosts"].as_array_mut().unwrap().push("www.pandora.com".into());
    data["services"].as_array_mut().unwrap().push("Yandex Music".into());
    data["services"].as_array_mut().unwrap().push("Pandora".into());
    let manifest = parse_manifest(&data.to_string()).unwrap();
    assert_eq!(manifest.services, ["Amazon Music", "Qobuz", "Tidal", "Deezer"]);
    assert_eq!(manifest.hosts, ["music.amazon.com", "qobuz.com", "tidal.com", "deezer.com"]);
}

#[test]
fn command_line_fills_placeholders_and_option_args() {
    let m = parse_manifest(MANIFEST).unwrap();
    let args = command_args(&m, &fill(), &[0]);
    assert_eq!(args, ["https://tidal.com/track/1", "--output", "C:/Music", "--quality", "lossless"]);
    assert_eq!(command_env(&m, &fill()), [("ODSKYLER_HOME".to_string(), "C:/data/odskyler".to_string())]);
}

#[test]
fn bundled_downloaders_get_the_correct_browser_environment() {
    let shared_home = ("CLOAKBROWSER_HOME".into(), "C:/data/cloakbrowser".into());
    for (id, browser) in [
        ("flacdownloader", true),
        ("doubledouble", true),
        ("lucida", true),
        ("monochrome", true),
        ("cyan-dl", false),
        ("odskyler", false),
    ] {
        let manifest = crate::fixtures::manifest(id);
        assert_eq!(manifest.browser, browser, "{id}");
        assert_eq!(command_env(&manifest, &fill()).contains(&shared_home), browser, "{id}");
    }
}

#[test]
fn placeholders_inside_a_pasted_url_stay_literal() {
    let m = parse_manifest(MANIFEST).unwrap();
    let f = Fill { url: "https://tidal.com/track/1?x={output}", ..fill() };
    assert_eq!(command_args(&m, &f, &[1])[0], "https://tidal.com/track/1?x={output}");
}

#[test]
fn hosts_match_themselves_and_subdomains_only() {
    let hosts = vec!["tidal.com".to_string()];
    assert!(host_matches(&hosts, "tidal.com"));
    assert!(host_matches(&hosts, "listen.tidal.com"));
    assert!(!host_matches(&hosts, "nottidal.com"));
    assert!(!host_matches(&hosts, "tidal.com.evil.net"));
}

#[test]
fn url_host_reads_pasted_links() {
    assert_eq!(url_host("  https://Listen.Tidal.com/track/1 ").as_deref(), Some("listen.tidal.com"));
    assert_eq!(url_host("https://music.amazon.com:443/x").as_deref(), Some("music.amazon.com"));
    assert_eq!(url_host("https://user@music.amazon.com:443/x"), None, "credentials rejected");
    assert_eq!(url_host(r"https://evil.com\@tidal.com/track/1").as_deref(), Some("evil.com"));
    assert_eq!(url_host("tidal.com/track/1"), None);
    assert_eq!(url_host("https://localhost/x"), None);
}

#[test]
fn manifests_that_escape_their_folder_or_skip_the_url_are_rejected() {
    for bad in [
        "../evil.exe",
        "/bin/sh",
        "\\\\Windows\\\\System32\\\\cmd.exe",
        "C:evil.exe",
        "C:/Windows/cmd.exe",
        "./odskyler.exe",
        "",
    ] {
        let escape = MANIFEST.replace("\"odskyler.exe\"", &format!("\"{bad}\""));
        assert!(parse_manifest(&escape).is_err(), "expected rejection for {bad:?}");
    }
    for blocked_env in ["LD_PRELOAD", "PATH", "NODE_OPTIONS", "HTTPS_PROXY", "FLACORA_HOME"] {
        let bad_env = MANIFEST.replace("\"ODSKYLER_HOME\"", &format!("\"{blocked_env}\""));
        assert!(parse_manifest(&bad_env).is_err(), "{blocked_env} must be rejected");
    }
    let bad_host = MANIFEST.replace("\"tidal.com\"", "\"com\"");
    assert!(parse_manifest(&bad_host).is_err(), "bare TLD host must be rejected");
    let empty_choices = MANIFEST.replace(
        r#"[{"value": "lossless", "label": "Lossless"}, {"value": "hires", "label": "Hi-Res"}]"#,
        "[]",
    );
    assert!(parse_manifest(&empty_choices).is_err(), "empty option choices must be rejected");
    let no_url = MANIFEST.replace("\"{url}\", ", "");
    assert!(parse_manifest(&no_url).is_err());
    assert!(valid_id("Bad Id").is_err());
}

#[test]
fn lookup_manifests_take_a_local_file_without_link_hosts() {
    let manifest = crate::fixtures::manifest("lrclib");
    assert_eq!(manifest.kind, Kind::Lyrics);
    assert!(manifest.hosts.is_empty());
    let file = "C:/Music/Album {url}/Track.flac";
    assert_eq!(command_args(&manifest, &Fill { file, ..fill() }, &[]), [file]);
    let mut invalid = crate::fixtures::manifest_json("lrclib").clone();
    invalid["args"] = serde_json::json!(["{url}"]);
    assert!(parse_manifest(&invalid.to_string()).is_err());
}
