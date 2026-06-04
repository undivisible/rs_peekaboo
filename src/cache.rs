use crate::{PeekabooError, Result, Snapshot};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn cache_dir() -> Result<PathBuf> {
    let base = dirs::cache_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".cache")))
        .ok_or(PeekabooError::MissingArgument("cache dir"))?;
    Ok(base.join("rs_peekaboo"))
}

pub fn snapshot_dir() -> Result<PathBuf> {
    Ok(cache_dir()?.join("snapshots"))
}

pub fn new_snapshot_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("snap-{millis}")
}

pub fn save_snapshot(snapshot: &Snapshot) -> Result<()> {
    let dir = snapshot_dir()?;
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", snapshot.snapshot_id));
    fs::write(path, serde_json::to_vec_pretty(snapshot)?)?;
    Ok(())
}

pub fn load_snapshot(id: &str) -> Result<Snapshot> {
    let path = snapshot_dir()?.join(format!("{id}.json"));
    let data = fs::read(path)?;
    Ok(serde_json::from_slice(&data)?)
}

pub fn clean_snapshots(all: bool, snapshot: Option<&str>) -> Result<usize> {
    let dir = snapshot_dir()?;
    if let Some(snapshot) = snapshot {
        let path = dir.join(format!("{snapshot}.json"));
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
