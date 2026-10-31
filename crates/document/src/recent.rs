//! Local, bounded recent-document preferences. No artwork is stored here.
use crate::PaintError;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NONCE: AtomicU64 = AtomicU64::new(0);
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recent {
    pub paths: Vec<PathBuf>,
    /// UTC seconds when each path was last opened; parallel to `paths`.
    /// Missing from legacy recent lists and then filled with zero.
    #[serde(default)]
    pub opened_unix_seconds: Vec<u64>,
}
impl Recent {
    pub fn load(path: &Path) -> Result<Self, PaintError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let mut data = Vec::new();
        File::open(path)?.take(131073).read_to_end(&mut data)?;
        if data.len() > 131072 {
            return Err(PaintError::Limit);
        }
        let mut recent: Self = serde_json::from_slice(&data)?;
        if recent.paths.len() > 20
            || recent.opened_unix_seconds.len() > 20
            || recent.opened_unix_seconds.len() > recent.paths.len()
            || recent
                .paths
                .iter()
                .any(|p| p.as_os_str().len() > 4096 || p.extension().is_none_or(|e| e != "rpl"))
        {
            return Err(PaintError::Invalid);
        }
        recent.opened_unix_seconds.resize(recent.paths.len(), 0);
        Ok(recent)
    }
    pub fn remember(&mut self, path: &Path) -> Result<(), PaintError> {
        let path = fs::canonicalize(path)?;
        if path.as_os_str().len() > 4096 || path.extension().is_none_or(|e| e != "rpl") {
            return Err(PaintError::Invalid);
        }
        let opened = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        let existing = self.paths.iter().position(|p| p == &path);
        if let Some(index) = existing {
            self.paths.remove(index);
            if self.opened_unix_seconds.len() > index {
                self.opened_unix_seconds.remove(index);
            }
        }
        self.paths.insert(0, path);
        self.opened_unix_seconds.insert(0, opened);
        self.paths.truncate(20);
        self.opened_unix_seconds.truncate(20);
        Ok(())
    }
    pub fn save(&self, path: &Path) -> Result<(), PaintError> {
        let data = serde_json::to_vec(self)?;
        if data.len() > 131072 || self.paths.len() > 20 {
            return Err(PaintError::Limit);
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let temp = parent.join(format!(
            ".recent-{}-{}.tmp",
            std::process::id(),
            NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| {
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            f.write_all(&data)?;
            f.sync_all()?;
            fs::rename(&temp, path)?;
            Ok(())
        })();
        let _ = fs::remove_file(temp);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "rpl-recent-{}-{}.json",
            std::process::id(),
            NONCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn legacy_recent_lists_migrate_and_open_time_tracks_path_order() {
        let preferences = path();
        let a = std::env::temp_dir().join("legacy-a.rpl");
        let b = std::env::temp_dir().join("legacy-b.rpl");
        fs::write(&preferences, serde_json::json!({"paths":[a,b]}).to_string()).unwrap();
        let mut recent = Recent::load(&preferences).unwrap();
        assert_eq!(recent.opened_unix_seconds, [0, 0]);
        assert!(recent.paths[0].ends_with("legacy-a.rpl"));

        let artwork = std::env::temp_dir().join(format!(
            "rpl-recent-artwork-{}-{}.rpl",
            std::process::id(),
            NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&artwork, b"synthetic file").unwrap();
        recent.remember(&artwork).unwrap();
        assert_eq!(recent.paths[0], fs::canonicalize(&artwork).unwrap());
        assert!(recent.opened_unix_seconds[0] > 0);
        assert_eq!(recent.paths.len(), recent.opened_unix_seconds.len());
        let _ = fs::remove_file(preferences);
        let _ = fs::remove_file(artwork);
    }
}
