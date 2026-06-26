use crate::{PeekabooError, Result, Snapshot};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static SNAPSHOT_ID_COUNTER: AtomicU32 = AtomicU32::new(0);

pub fn cache_dir() -> Result<PathBuf> {
    let base = dirs::cache_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".cache")))
        .ok_or(PeekabooError::MissingArgument("cache dir"))?;
    Ok(base.join("rs_peekaboo"))
}

pub fn snapshot_dir() -> Result<PathBuf> {
    Ok(cache_dir()?.join("snapshots"))
}

pub fn validate_snapshot_id(id: &str) -> Result<()> {
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(PeekabooError::InvalidSnapshotId(id.to_string()));
    }
    Ok(())
}

fn snapshot_path(id: &str) -> Result<PathBuf> {
    validate_snapshot_id(id)?;
    let dir = snapshot_dir()?;
    let path = dir.join(format!("{id}.json"));
    ensure_within_dir(&dir, &path)?;
    Ok(path)
}

fn ensure_within_dir(dir: &Path, path: &Path) -> Result<()> {
    fs::create_dir_all(dir)?;
    let dir = fs::canonicalize(dir)?;
    let parent = path
        .parent()
        .ok_or_else(|| PeekabooError::InvalidSnapshotId(path.to_string_lossy().into_owned()))?;
    if parent != dir {
        return Err(PeekabooError::InvalidSnapshotId(
            path.to_string_lossy().into_owned(),
        ));
    }
    Ok(())
}

pub fn new_snapshot_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let n = SNAPSHOT_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("snap-{millis}-{n}")
}

pub fn save_snapshot(snapshot: &Snapshot) -> Result<()> {
    validate_snapshot_id(&snapshot.snapshot_id)?;
    let path = snapshot_path(&snapshot.snapshot_id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(snapshot)?)?;
    Ok(())
}

pub fn load_snapshot(id: &str) -> Result<Snapshot> {
    let path = snapshot_path(id)?;
    let data = fs::read(path)?;
    Ok(serde_json::from_slice(&data)?)
}

pub fn clean_snapshots(all: bool, snapshot: Option<&str>) -> Result<usize> {
    let dir = snapshot_dir()?;
    if let Some(snapshot) = snapshot {
        let path = snapshot_path(snapshot)?;
        if path.exists() {
            fs::remove_file(path)?;
            return Ok(1);
        }
        return Ok(0);
    }
    if !all || !dir.exists() {
        return Ok(0);
    }
    let mut removed = 0;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.path().extension().is_some_and(|ext| ext == "json") {
            fs::remove_file(entry.path())?;
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_snapshot_id_should_reject_path_traversal() {
        assert!(validate_snapshot_id("../etc/passwd").is_err());
        assert!(validate_snapshot_id("snap-ok").is_ok());
        assert!(validate_snapshot_id("snap_123").is_ok());
    }

    #[test]
    fn new_snapshot_id_should_be_unique_and_valid() {
        let id1 = new_snapshot_id();
        let id2 = new_snapshot_id();
        assert_ne!(id1, id2);
        assert!(validate_snapshot_id(&id1).is_ok());
        assert!(validate_snapshot_id(&id2).is_ok());
        assert!(id1.starts_with("snap-"));
        assert!(id2.starts_with("snap-"));
    }
}
