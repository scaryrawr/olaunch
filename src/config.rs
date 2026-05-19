use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{OlaunchError, Result};
use crate::integrations::ConfigEdit;

pub fn apply_edits(edits: &[ConfigEdit]) -> Result<Vec<PathBuf>> {
    let mut backups = Vec::new();
    for edit in edits {
        if let Some(parent) = edit.path.parent() {
            fs::create_dir_all(parent)?;
        }
        if edit.path.exists() {
            backups.push(create_backup(&edit.path, &edit.integration)?);
        }
        write_atomic(&edit.path, edit.content.as_bytes())?;
    }
    Ok(backups)
}

pub fn create_backup(path: &Path, integration: &str) -> Result<PathBuf> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| OlaunchError::Message(format!("system clock before unix epoch: {err}")))?
        .as_secs();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| OlaunchError::Message(format!("invalid config path {}", path.display())))?;
    let backup = path.with_file_name(format!("{file_name}.olaunch-{integration}.{timestamp}.bak"));
    fs::copy(path, &backup)?;
    Ok(backup)
}

pub fn restore_latest(path: &Path, integration: &str) -> Result<Option<PathBuf>> {
    let Some(parent) = path.parent() else {
        return Ok(None);
    };
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return Ok(None);
    };
    let needle = format!("{file_name}.olaunch-{integration}.");
    let mut backups = Vec::new();
    if !parent.exists() {
        return Ok(None);
    }
    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(&needle) && name.ends_with(".bak") {
            backups.push(entry.path());
        }
    }
    backups.sort();
    let Some(latest) = backups.pop() else {
        return Ok(None);
    };
    fs::copy(&latest, path)?;
    Ok(Some(latest))
}

fn write_atomic(path: &Path, content: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| OlaunchError::Message(format!("invalid config path {}", path.display())))?;
    let tmp = parent.join(format!(
        ".{}.olaunch.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("config")
    ));
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(content)?;
        file.sync_all()?;
    }
    fs::rename(tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{apply_edits, restore_latest};
    use crate::integrations::ConfigEdit;

    #[test]
    fn backs_up_and_restores_latest_edit() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, "old").unwrap();
        apply_edits(&[ConfigEdit {
            path: path.clone(),
            content: "new".into(),
            description: "test".into(),
            integration: "codex".into(),
        }])
        .unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
        let restored = restore_latest(&path, "codex").unwrap();
        assert!(restored.is_some());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "old");
    }
}
