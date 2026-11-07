//! Persist the cloud session (keeps the user signed in; holds the token, so user-only perms).

use directories::ProjectDirs;
use std::fs;
use std::path::PathBuf;
use tapo_core::Session;

fn path() -> Option<PathBuf> {
    let dirs = ProjectDirs::from("dev", "Dhananjay-JSR", "tapo-desktop")?;
    fs::create_dir_all(dirs.config_dir()).ok()?;
    Some(dirs.config_dir().join("session.json"))
}

pub fn load() -> Session {
    path()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(session: &Session) {
    if let (Some(p), Ok(json)) = (path(), serde_json::to_string(session)) {
        if fs::write(&p, json).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&p, fs::Permissions::from_mode(0o600));
            }
        }
    }
}

pub fn clear() {
    if let Some(p) = path() {
        let _ = std::fs::remove_file(p);
    }
}
