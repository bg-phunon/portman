use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const CACHE_TTL: Duration = Duration::from_secs(60 * 60 * 24);
const RELEASES_URL: &str = "https://api.github.com/repos/bg-phunon/portman/releases/latest";
const TAGS_URL: &str = "https://api.github.com/repos/bg-phunon/portman/tags?per_page=20";
const UPGRADE_COMMAND: &str = "brew update && brew upgrade bg-phunon/tap/portman";

#[derive(Debug)]
pub struct UpdateNotice {
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
}

#[derive(Debug, Deserialize)]
struct GitHubTag {
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct UpdateCache {
    checked_at: u64,
    latest_version: String,
}

pub fn spawn_update_check(current_version: &'static str) -> Option<Receiver<UpdateNotice>> {
    if std::env::var_os("PORTMAN_NO_UPDATE_CHECK").is_some() {
        return None;
    }

    let (tx, rx) = mpsc::channel();
    let current_version = current_version.to_string();

    thread::spawn(move || {
        if let Some(latest_version) = latest_version() {
            if is_newer_version(&latest_version, &current_version) {
                let notice = UpdateNotice {
                    message: format!(
                        "Update available: {latest_version}  Run: {UPGRADE_COMMAND}"
                    ),
                };
                let _ = tx.send(notice);
            }
        }
    });

    Some(rx)
}

fn latest_version() -> Option<String> {
    if let Some(cache) = load_cache() {
        if !cache_expired(cache.checked_at) {
            return Some(cache.latest_version);
        }
    }

    let version = fetch_latest_candidate_version()?;
    store_cache(&version);
    Some(version)
}

fn fetch_latest_candidate_version() -> Option<String> {
    let release_version = fetch_latest_release().map(|release| normalize_version(&release.tag_name));
    let tag_version = fetch_latest_tag_version();
    fetch_latest_candidate(release_version, tag_version)
}

fn fetch_latest_candidate(
    release_version: Option<String>,
    tag_version: Option<String>,
) -> Option<String> {
    match (release_version, tag_version) {
        (Some(release), Some(tag)) => {
            if is_newer_version(&tag, &release) {
                Some(tag)
            } else {
                Some(release)
            }
        }
        (Some(release), None) => Some(release),
        (None, Some(tag)) => Some(tag),
        (None, None) => None,
    }
}

fn fetch_latest_release() -> Option<GitHubRelease> {
    let output = github_api_get(RELEASES_URL)?;

    if !output.status.success() {
        return None;
    }

    serde_json::from_slice(&output.stdout).ok()
}

fn fetch_latest_tag_version() -> Option<String> {
    let output = github_api_get(TAGS_URL)?;

    if !output.status.success() {
        return None;
    }

    let tags: Vec<GitHubTag> = serde_json::from_slice(&output.stdout).ok()?;
    tags.into_iter()
        .map(|tag| normalize_version(&tag.name))
        .max_by_key(|version| parse_version(version))
}

fn github_api_get(url: &str) -> Option<std::process::Output> {
    Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--location",
            "--max-time",
            "2",
            "--header",
            "Accept: application/vnd.github+json",
            "--header",
            "User-Agent: portman",
            url,
        ])
        .output()
        .ok()
}

fn normalize_version(version: &str) -> String {
    version.trim().trim_start_matches('v').to_string()
}

fn cache_expired(checked_at: u64) -> bool {
    now_epoch_secs()
        .map(|now| now.saturating_sub(checked_at) > CACHE_TTL.as_secs())
        .unwrap_or(true)
}

fn now_epoch_secs() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

fn load_cache() -> Option<UpdateCache> {
    let path = cache_path()?;
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn store_cache(latest_version: &str) {
    let Some(path) = cache_path() else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    let Some(checked_at) = now_epoch_secs() else {
        return;
    };

    if fs::create_dir_all(parent).is_err() {
        return;
    }

    let cache = UpdateCache {
        checked_at,
        latest_version: latest_version.to_string(),
    };

    let Ok(json) = serde_json::to_vec(&cache) else {
        return;
    };

    let _ = fs::write(path, json);
}

fn cache_path() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("HOME") {
        return Some(
            Path::new(&home)
                .join("Library")
                .join("Caches")
                .join("portman")
                .join("update-check.json"),
        );
    }

    None
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    parse_version(latest) > parse_version(current)
}

fn parse_version(version: &str) -> (u64, u64, u64) {
    let mut parts = version
        .trim()
        .trim_start_matches('v')
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0));

    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

#[cfg(test)]
mod tests {
    use super::{fetch_latest_candidate, is_newer_version, normalize_version, parse_version};

    fn some(value: &str) -> Option<String> {
        Some(value.to_string())
    }

    #[test]
    fn normalize_version_strips_v_prefix() {
        assert_eq!(normalize_version("v0.2.0"), "0.2.0");
        assert_eq!(normalize_version("0.2.0"), "0.2.0");
    }

    #[test]
    fn parse_version_handles_missing_parts() {
        assert_eq!(parse_version("0.2"), (0, 2, 0));
        assert_eq!(parse_version("v1"), (1, 0, 0));
    }

    #[test]
    fn newer_version_comparison_is_semantic() {
        assert!(is_newer_version("0.2.0", "0.1.9"));
        assert!(is_newer_version("1.0.0", "0.9.9"));
        assert!(!is_newer_version("0.2.0", "0.2.0"));
        assert!(!is_newer_version("0.2.0", "0.3.0"));
    }

    #[test]
    fn candidate_version_prefers_newer_tag_over_release() {
        assert_eq!(
            fetch_latest_candidate(some("0.2.0"), some("0.2.1")),
            some("0.2.1")
        );
        assert_eq!(
            fetch_latest_candidate(some("0.2.1"), some("0.2.0")),
            some("0.2.1")
        );
        assert_eq!(fetch_latest_candidate(None, some("0.2.1")), some("0.2.1"));
    }
}
