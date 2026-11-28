use super::*;
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    thread,
    time::Duration,
};

#[test]
fn compares_semantic_versions_without_false_updates() {
    let versions =
        parse(br#"{"schema_version":1,"extensions":{"odskyler":{"version":"1.0.10+release"}}}"#)
            .unwrap();
    for (installed, expected) in [
        ("1.0.9", true),
        ("1.0.10+local", false),
        ("1.0.11", false),
        ("invalid", false),
    ] {
        assert_eq!(newer(&versions, "odskyler", installed).is_some(), expected);
    }
    assert!(newer(&versions, "unknown", "1.0.0").is_none());
    let pre =
        parse(br#"{"schema_version":1,"extensions":{"odskyler":{"version":"2.0.0-beta.2"}}}"#)
            .unwrap();
    assert!(newer(&pre, "odskyler", "2.0.0-beta.1").is_some());
    assert!(newer(&pre, "odskyler", "2.0.0").is_none());
}

#[test]
fn rejects_bad_feeds_and_reads_the_complete_bundled_feed() {
    for bytes in [
        b"{}".as_slice(),
        b"<html>error</html>",
        br#"{"schema_version":2,"extensions":{}}"#,
        br#"{"schema_version":1,"extensions":{"odskyler":{"version":"bad"}}}"#,
        br#"{"schema_version":1,"extensions":{"../bad":{"version":"1.0.0"}}}"#,
    ] {
        assert!(parse(bytes).is_err());
    }
    let versions = parse(include_bytes!("../fixtures/extension-updates.json")).unwrap();
    assert_eq!(versions.len(), 21);

    let mixed = parse(
        br#"{"schema_version":1,"extensions":{"odskyler":{"version":"v1.2.3"},"broken":{"version":"not-semver"}}}"#,
    )
    .unwrap();
    assert_eq!(mixed.get("odskyler"), Some(&Version::new(1, 2, 3)));
    assert!(!mixed.contains_key("broken"));
}

#[test]
fn fetch_handles_http_errors_bad_json_and_size_limits() {
    for (status, body, success) in [
        (
            "200 OK",
            r#"{"schema_version":1,"extensions":{"odskyler":{"version":"1.0.2"}}}"#.to_owned(),
            true,
        ),
        ("404 Not Found", "{}".into(), false),
        ("429 Too Many Requests", "{}".into(), false),
        ("200 OK", "not JSON".into(), false),
        ("200 OK", " ".repeat(MAX_BYTES as usize + 1), false),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut reader = BufReader::new(&socket);
            let mut line = String::new();
            assert!(reader.read_line(&mut line).unwrap() > 0);
            assert!(line.starts_with("GET /versions "));
            loop {
                line.clear();
                if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                    break;
                }
            }
            let _ = write!(
                socket,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        });
        let client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        assert_eq!(
            fetch(&client, &format!("http://{address}/versions")).is_ok(),
            success
        );
        server.join().unwrap();
    }
}

#[test]
fn connection_timeout_is_a_failed_check() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let client = Client::builder()
        .no_proxy()
        .timeout(Duration::from_millis(100))
        .build()
        .unwrap();
    assert!(fetch(&client, &format!("http://{address}/versions")).is_err());
}

#[test]
#[ignore = "requires the public Gist; run explicitly when verifying the live feed"]
fn live_gist_uses_the_production_fetcher() {
    let versions = check().expect("live Gist is readable");
    assert!(!versions.is_empty());
    assert!(versions.contains_key("odskyler"));
    println!("Live Gist: {} extension versions", versions.len());
}
