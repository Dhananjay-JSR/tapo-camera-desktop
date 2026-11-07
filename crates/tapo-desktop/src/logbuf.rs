//! Process-wide log sink shared by the UI, background threads, and the panic hook, so the Log
//! tab's "Copy all" captures everything (events, backend warnings, crashes) in one place.

use directories::ProjectDirs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const CAP: usize = 2000;

fn buf() -> &'static Mutex<Vec<String>> {
    static B: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    B.get_or_init(|| Mutex::new(Vec::new()))
}

fn file() -> &'static Mutex<Option<File>> {
    static F: OnceLock<Mutex<Option<File>>> = OnceLock::new();
    F.get_or_init(|| Mutex::new(None))
}

fn path_cell() -> &'static OnceLock<Option<PathBuf>> {
    static P: OnceLock<Option<PathBuf>> = OnceLock::new();
    &P
}

/// Full path of the on-disk log file, if available.
pub fn path() -> Option<PathBuf> {
    path_cell().get().cloned().flatten()
}

/// Create a fresh log file for this session. Call once at startup.
pub fn init() {
    let p = ProjectDirs::from("dev", "Dhananjay-JSR", "tapo-desktop").and_then(|d| {
        std::fs::create_dir_all(d.data_dir()).ok()?;
        Some(d.data_dir().join("tapo-desktop.log"))
    });
    if let Some(ref p) = p {
        if let Ok(f) = File::create(p) {
            *file().lock().unwrap() = Some(f);
        }
    }
    let _ = path_cell().set(p);
    log("session started");
}

fn hms() -> String {
    let s = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        % 86400) as i64;
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

/// Append a timestamped line. Safe to call from any thread.
pub fn log(msg: impl AsRef<str>) {
    let line = format!("{}  {}", hms(), msg.as_ref());
    if let Ok(mut b) = buf().lock() {
        b.push(line.clone());
        let overflow = b.len().saturating_sub(CAP);
        if overflow > 0 {
            b.drain(0..overflow);
        }
    }
    if let Ok(mut f) = file().lock() {
        if let Some(f) = f.as_mut() {
            let _ = writeln!(f, "{line}");
            let _ = f.flush();
        }
    }
}

pub fn snapshot() -> Vec<String> {
    buf().lock().map(|b| b.clone()).unwrap_or_default()
}

pub fn clear() {
    if let Ok(mut b) = buf().lock() {
        b.clear();
    }
}

/// Route panics (including on background threads) into the log, then run the default handler.
pub fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .to_string();
        log(format!("PANIC [{thread}]: {info}"));
        prev(info);
    }));
}

/// Open the log file in the OS default application.
pub fn open_file() {
    let Some(p) = path() else { return };
    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(cmd).arg(p).spawn();
}
