//! Persistence for update checker.

use super::UpdateNotification;
use crate::framework::cache_path;
use semver::Version;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, SystemTimeError};
use thiserror::Error;
use toml_edit::{Document, TomlError};

/// All the ways in which persistence can fail.
#[derive(Debug, Error)]
pub(crate) enum Error {
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// TOML parse error.
    #[error("Persistence parse error: {0}")]
    Toml(#[from] TomlError),

    /// SemVer parse error.
    #[error("SemVer parse error: {0}")]
    SemVer(#[from] semver::Error),

    /// Type error for `last_check`.
    #[error("`last_check` is invalid")]
    LastCheck,

    /// Type error for `last_update`.
    #[error("`last_update` is invalid")]
    LastUpdate,
}

#[derive(Debug)]
pub(crate) struct Persist {
    /// Original path to TOML file.
    doc_path: PathBuf,

    /// TOML document.
    doc: Document,

    /// Last time the version was checked.
    last_check: SystemTime,

    /// Last known version.
    last_version: Version,

    /// Current application version.
    current_version: Version,

    /// Update release notes.
    release_notes: String,

    /// Update URL.
    update_url: String,
}

impl Persist {
    /// Create a new update checker persistence.
    pub(crate) fn new() -> Result<Self, Error> {
        let mut doc_path = cache_path();
        doc_path.push("updates.toml");

        let doc: Document = match fs::read_to_string(&doc_path) {
            Ok(data) => data.parse()?,
            Err(_) => {
                let mut doc = Document::new();
                let last_version = Version::parse(env!("CARGO_PKG_VERSION"))?;

                doc["last_check"] = toml_edit::value(0.0);
                doc["last_version"] = toml_edit::value(last_version.to_string());
                doc["release_notes"] = toml_edit::value("");
                doc["update_url"] = toml_edit::value("");

                doc
            }
        };

        let last_check = doc["last_check"].as_float().ok_or(Error::LastCheck)?;
        let last_check = SystemTime::UNIX_EPOCH + Duration::from_secs_f64(last_check);

        let last_version = doc["last_version"]
            .as_str()
            .ok_or(Error::LastUpdate)?
            .parse()?;

        let current_version = Version::parse(env!("CARGO_PKG_VERSION"))?;

        let release_notes = doc["release_notes"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let update_url = doc["update_url"].as_str().unwrap_or_default().to_string();

        Ok(Self {
            doc_path,
            doc,
            last_check,
            last_version,
            current_version,
            release_notes,
            update_url,
        })
    }

    /// Create TOML file from this Persist.
    ///
    /// The Config remembers the original TOML path, and this method rewrites that file. The config
    /// file is created if it does not exist, along with all intermediate directories in the path.
    pub(crate) fn write_toml(&self) -> Result<(), Error> {
        let toml = self.doc.to_string();
        if let Some(parent) = self.doc_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let result = fs::write(&self.doc_path, toml)?;

        Ok(result)
    }

    pub(crate) fn last_check(&self) -> Result<Duration, SystemTimeError> {
        self.last_check.elapsed()
    }

    pub(crate) fn update_last_check(&mut self) -> Result<(), SystemTimeError> {
        self.last_check = SystemTime::now();
        let last_check = self
            .last_check
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs_f64();

        self.doc["last_check"] = toml_edit::value(last_check);

        Ok(())
    }

    pub(crate) fn last_version(&self) -> &Version {
        &self.last_version
    }

    pub(crate) fn update_last_version(&mut self, last_version: Version) {
        self.last_version = last_version;
        self.doc["last_version"] = toml_edit::value(self.last_version.to_string());
    }

    pub(crate) fn current_version(&self) -> &Version {
        &self.current_version
    }

    pub(crate) fn update_release_notes(&mut self, release_notes: String) {
        self.release_notes = release_notes;
        self.doc["release_notes"] = toml_edit::value(&self.release_notes);
    }

    pub(crate) fn update_url(&mut self, update_url: String) {
        self.update_url = update_url;
        self.doc["update_url"] = toml_edit::value(&self.update_url);
    }

    pub(crate) fn get_update_notification(&self) -> UpdateNotification {
        UpdateNotification {
            version: self.last_version.clone(),
            release_notes: self.release_notes.clone(),
            update_url: self.update_url.clone(),
        }
    }
}
