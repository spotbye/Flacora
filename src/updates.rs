//! Startup update checks against Flacora's public GitHub Releases.

use reqwest::{StatusCode, Url, blocking::Client, header};
use semver::Version;
use serde::Deserialize;
use std::io::Read;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const LATEST_RELEASE: &str = "https://api.github.com/repos/spotbye/Flacora/releases/latest";
const RELEASE_PAGE: &str = "https://github.com/spotbye/Flacora/releases/tag";
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    pub url: Url,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
}

pub fn check() -> Option<Release> {
    check_result().unwrap_or_default()
}

pub fn check_result() -> Result<Option<Release>, String> {
    let client = crate::store::client()?;
    fetch(&client, LATEST_RELEASE)
}

fn fetch(client: &Client, endpoint: &str) -> Result<Option<Release>, String> {
    let response = client.get(endpoint)
        .header(header::ACCEPT, "application/vnd.github+json")
        .send()
        .map_err(|_| "Could not reach GitHub. Try again.".to_owned())?;
    let status = response.status();
    if status == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(format!("GitHub returned HTTP {}. Try again later.", status.as_u16()));
    }
    let mut bytes = Vec::new();
    response.take(MAX_RESPONSE_BYTES + 1).read_to_end(&mut bytes)
        .map_err(|_| "Could not read the GitHub release.".to_owned())?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err("GitHub's release response was too large.".to_owned());
    }
    let release: GitHubRelease = serde_json::from_slice(&bytes)
        .map_err(|_| "GitHub returned invalid release information.".to_owned())?;
    from_tag(&release.tag_name)
}

pub fn from_tag(tag: &str) -> Result<Option<Release>, String> {
    let tag = tag.trim();
    let version = Version::parse(tag.strip_prefix(['v', 'V']).unwrap_or(tag))
        .map_err(|_| "GitHub's release tag is not a valid version.".to_owned())?;
    let installed = Version::parse(CURRENT_VERSION).expect("Cargo package version");
    if !version.cmp_precedence(&installed).is_gt() {
        return Ok(None);
    }
    let mut url = Url::parse(RELEASE_PAGE).expect("GitHub release URL");
    url.path_segments_mut().expect("GitHub release path").push(tag);
    Ok(Some(Release { version, url }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::{BufRead, BufReader, Write}, net::TcpListener, thread, time::Duration};

    #[test]
    fn compares_release_tag_versions_and_ignores_build_metadata() {
        let installed = Version::parse(CURRENT_VERSION).unwrap();
        let newer = Version::new(installed.major, installed.minor + 10, 0);
        assert_eq!(from_tag("0.0.0"), Ok(None));
        assert_eq!(from_tag(&format!("v{CURRENT_VERSION}+build")), Ok(None));
        assert_eq!(from_tag(&newer.to_string()), Ok(Some(Release {
            version: newer.clone(),
            url: Url::parse(&format!("{RELEASE_PAGE}/{newer}")).unwrap(),
        })));
        assert!(from_tag("latest").is_err());
    }

    #[test]
    fn reads_github_style_release_responses() {
        let installed = Version::parse(CURRENT_VERSION).unwrap();
        let tag = format!("v{}.{}.0", installed.major, installed.minor + 1);
        for (status, body, expected) in [
            ("200 OK", serde_json::json!({ "tag_name": tag }).to_string(), from_tag(&tag)),
            ("404 Not Found", "{}".into(), Ok(None)),
            ("429 Too Many Requests", "{}".into(), Err("GitHub returned HTTP 429. Try again later.".into())),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                socket.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                let mut reader = BufReader::new(&socket);
                let mut line = String::new();
                assert!(reader.read_line(&mut line).unwrap() > 0);
                assert!(line.starts_with("GET /releases/latest "));
                loop {
                    line.clear();
                    if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                        break;
                    }
                }
                write!(socket, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            });
            let client = Client::builder().no_proxy().timeout(Duration::from_secs(2)).build().unwrap();
            assert_eq!(fetch(&client, &format!("http://{address}/releases/latest")), expected);
            server.join().unwrap();
        }
    }
}
