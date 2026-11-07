use crate::model::Log;
use fs2::FileExt;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

pub struct Storage {
    pub root: PathBuf,
    _lock: File,
}
impl Storage {
    pub fn open(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join("radio-log.lock"))
            .map_err(|e| e.to_string())?;
        lock.try_lock_exclusive()
            .map_err(|_| "This log is already open in another app window.".to_string())?;
        Ok(Self { root, _lock: lock })
    }
    pub fn path(&self) -> PathBuf {
        self.root.join("radio-log.json")
    }
    pub fn load(&self) -> Result<Log, String> {
        if !self.path().exists() {
            return Ok(Log::default());
        }
        Log::parse(&fs::read(self.path()).map_err(|e| e.to_string())?)
    }
    pub fn save(&self, log: &Log) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(log).map_err(|e| e.to_string())?;
        if self.path().exists() {
            let backups = self.root.join("backups");
            fs::create_dir_all(&backups).map_err(|e| e.to_string())?;
            let file = format!(
                "{}-{}.json",
                chrono::Utc::now().format("%Y%m%dT%H%M%S"),
                uuid::Uuid::new_v4()
            );
            fs::copy(self.path(), backups.join(file))
                .map_err(|e| format!("Backup failed; log was not changed: {e}"))?;
        }
        atomic_write(&self.path(), &bytes)
    }
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("Invalid file path")?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    tmp.write_all(bytes).map_err(|e| e.to_string())?;
    tmp.as_file().sync_all().map_err(|e| e.to_string())?;
    tmp.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
