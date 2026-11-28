//! Informational extension versions from a public Gist. Packages remain manual installs.

use reqwest::blocking::Client;
use semver::Version;
use serde::Deserialize;
use std::{collections::BTreeMap, io::Read};

pub const FEED_URL: &str = "https://gist.githubusercontent.com/afkarxyz/62556be30c357c1e8cf02e8c546ea059/raw/extension-updates.json";
const MAX_BYTES: u64 = 128 * 1024;
pub type Versions = BTreeMap<String, Version>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CheckState {
    #[default]
    Idle,
    Checking,
    Ready,
    Failed,
}

#[derive(Deserialize)]
struct Feed {
    schema_version: u32,
    extensions: BTreeMap<String, Entry>,
}

#[derive(Deserialize)]
struct Entry {
    version: String,
}

pub fn check() -> Result<Versions, String> {
    fetch(&crate::store::client()?, FEED_URL)
}

fn fetch(client: &Client, endpoint: &str) -> Result<Versions, String> {
    let response = client
        .get(endpoint)
        .header(reqwest::header::CACHE_CONTROL, "no-cache")
        .send()
        .map_err(|_| "Could not reach the extension version feed.".to_owned())?;
    if !response.status().is_success() {
        return Err(format!(
            "The extension version feed returned HTTP {}.",
            response.status().as_u16()
        ));
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read extension versions.".to_owned())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("The extension version feed was too large.".into());
    }
    parse(&bytes)
}

pub fn parse(bytes: &[u8]) -> Result<Versions, String> {
    let feed: Feed = serde_json::from_slice(bytes)
        .map_err(|_| "The extension version feed is invalid.".to_owned())?;
    if feed.schema_version != 1 {
        return Err("The extension version feed uses an unsupported format.".into());
    }
    let total = feed.extensions.len();
    let mut versions = Versions::new();
    let mut first_error = None;
    for (id, entry) in feed.extensions {
        if let Err(err) = crate::extensions::valid_id(&id) {
            first_error.get_or_insert(err);
            continue;
        }
        let raw = entry.version.trim();
        let trimmed = raw.strip_prefix(['v', 'V']).unwrap_or(raw);
        match Version::parse(trimmed) {
            Ok(version) => {
                versions.insert(id, version);
            }
            Err(_) => {
                first_error.get_or_insert_with(|| format!("Invalid version for extension {id}."));
            }
        }
    }
    if total > 0 && versions.is_empty()
        && let Some(error) = first_error
    {
        return Err(error);
    }
    Ok(versions)
}

pub fn newer<'a>(versions: &'a Versions, id: &str, installed: &str) -> Option<&'a Version> {
    let latest = versions.get(id)?;
    let raw = installed.trim();
    let installed = Version::parse(raw.strip_prefix(['v', 'V']).unwrap_or(raw)).ok()?;
    latest.cmp_precedence(&installed).is_gt().then_some(latest)
}

#[cfg(test)]
mod tests;
