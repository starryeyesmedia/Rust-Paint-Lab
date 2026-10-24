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
        let recent: Self = serde_json::from_slice(&data)?;
        if recent.paths.len() > 20
            || recent
                .paths
                .iter()
                .any(|p| p.as_os_str().len() > 4096 || p.extension().is_none_or(|e| e != "rpl"))
        {
            return Err(PaintError::Invalid);
        }
        Ok(recent)
    }
    pub fn remember(&mut self, path: &Path) -> Result<(), PaintError> {
        let path = fs::canonicalize(path)?;
        if path.as_os_str().len() > 4096 || path.extension().is_none_or(|e| e != "rpl") {
            return Err(PaintError::Invalid);
        }
        self.paths.retain(|p| p != &path);
        self.paths.insert(0, path);
        self.paths.truncate(20);
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
