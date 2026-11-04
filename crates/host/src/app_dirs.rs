//! Where Artheffect keeps its per-user data (settings, shortcuts, crash recovery, logs, models,
//! plug-ins): the one directory [`config_dir`](crate::config_dir) returns.
//!
//! In order:
//! 1. `ARTHEFFECT_CONFIG_DIR`, when set (tests, agents, custom setups).
//! 2. **Portable mode** (#993): a marker file ([`PORTABLE_MARKERS`]) beside the executable puts
//!    everything in `<exe dir>/ArtheffectData`, so a portable copy on a USB stick leaves nothing in
//!    `%APPDATA%`. The Windows portable zip ships with `portable.txt`; the MSI doesn't. If that
//!    folder can't be written (read-only medium, Program Files), a warning is logged and the
//!    platform folder is used.
//! 3. The platform convention: macOS `~/Library/Application Support/Artheffect`, Windows
//!    `%APPDATA%\Artheffect`, Linux `$XDG_CONFIG_HOME/artheffect` or `~/.config/artheffect`.
//!
//! Artheffect is a fork of EffectCraft: the first launch that finds no platform folder yet but an
//! EffectCraft one copies it over once ([`migrate_legacy`]), so settings, shortcuts and the
//! launch guard's Window Graphics choice carry across.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Files beside the executable that switch on portable mode (either one; contents are ignored).
pub const PORTABLE_MARKERS: [&str; 2] = ["portable.txt", "Artheffect.portable"];
/// The data folder created beside the executable in portable mode.
pub const PORTABLE_DATA_DIR: &str = "ArtheffectData";

/// How [`DataDir::dir`] was chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// `ARTHEFFECT_CONFIG_DIR`.
    Override,
    /// `<exe dir>/ArtheffectData`.
    Portable,
    /// The platform's per-user config directory.
    Platform,
}

/// The resolved data directory.
#[derive(Clone, Debug)]
pub struct DataDir {
    /// `None` only when no platform directory is known either (no `HOME`/`APPDATA`).
    pub dir: Option<PathBuf>,
    pub mode: Mode,
    /// Set when a portable marker was found but its data folder isn't writable.
    pub warning: Option<String>,
}

/// The data directory for this process, resolved once (the portable check probes the disk).
pub fn current() -> &'static DataDir {
    static DIR: OnceLock<DataDir> = OnceLock::new();
    DIR.get_or_init(|| {
        let exe = std::env::current_exe().ok();
        let env = |k: &str| std::env::var_os(k);
        let d = resolve(env, exe.as_deref().and_then(Path::parent));
        if let Some(w) = &d.warning {
            log::warn!("{w}");
        }
        if d.mode == Mode::Platform
            && let (Some(new), Some(old)) = (&d.dir, legacy_platform_dir(&env))
        {
            match migrate_legacy(&old, new) {
                Ok(true) => log::info!("copied the EffectCraft settings in {} to {}", old.display(), new.display()),
                Ok(false) => {}
                Err(e) => log::warn!("couldn't copy the EffectCraft settings in {} to {}: {e}", old.display(), new.display()),
            }
        }
        d
    })
}

/// Resolve the data directory from the environment (`env`) and the executable's folder.
pub fn resolve(env: impl Fn(&str) -> Option<OsString>, exe_dir: Option<&Path>) -> DataDir {
    if let Some(d) = env("ARTHEFFECT_CONFIG_DIR") {
        return DataDir { dir: Some(PathBuf::from(d)), mode: Mode::Override, warning: None };
    }
    let platform = platform_dir(&env);
    let Some((exe_dir, marker)) = exe_dir.and_then(|d| portable_marker(d).map(|m| (d, m))) else {
        return DataDir { dir: platform, mode: Mode::Platform, warning: None };
    };
    let data = exe_dir.join(PORTABLE_DATA_DIR);
    match ensure_writable(&data) {
        Ok(()) => DataDir { dir: Some(data), mode: Mode::Portable, warning: None },
        Err(e) => {
            let fallback = platform.as_ref().map_or_else(|| "nowhere (no user folder found)".to_string(), |p| p.display().to_string());
            let warning =
                format!("portable mode ({} found) but {} isn't writable ({e}); settings are kept in {fallback} instead", marker.display(), data.display());
            DataDir { dir: platform, mode: Mode::Platform, warning: Some(warning) }
        }
    }
}

/// The platform convention, from `env` (so tests can fake `HOME`/`APPDATA`/`XDG_CONFIG_HOME`).
pub fn platform_dir(env: &impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    platform_dir_named(env, "Artheffect", "artheffect")
}

/// Where EffectCraft, which Artheffect was forked from, kept the same data.
pub fn legacy_platform_dir(env: &impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    platform_dir_named(env, "EffectCraft", "effectcraft")
}

fn platform_dir_named(env: &impl Fn(&str) -> Option<OsString>, name: &str, unix_name: &str) -> Option<PathBuf> {
    let home = || env("HOME").map(PathBuf::from);
    if cfg!(target_os = "macos") {
        return home().map(|h| h.join("Library/Application Support").join(name));
    }
    if cfg!(target_os = "windows") {
        return env("APPDATA").map(|a| PathBuf::from(a).join(name));
    }
    env("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| home().map(|h| h.join(".config"))).map(|c| c.join(unix_name))
}

/// How deep [`migrate_legacy`] follows folders (the data folder is a few levels deep at most).
const MIGRATE_MAX_DEPTH: usize = 16;

/// Copy the EffectCraft data folder `old` to `new` once: only when `new` doesn't exist yet and
/// `old` is a folder. `Ok(true)` when it copied. A file that can't be copied stops the copy, and
/// the partial folder is removed so the next launch tries again.
pub fn migrate_legacy(old: &Path, new: &Path) -> std::io::Result<bool> {
    if new.exists() || !old.is_dir() {
        return Ok(false);
    }
    if let Err(e) = copy_dir(old, new, 0) {
        // Best effort: a leftover partial copy would stop the next attempt.
        let _ = std::fs::remove_dir_all(new);
        return Err(e);
    }
    Ok(true)
}

fn copy_dir(from: &Path, to: &Path, depth: usize) -> std::io::Result<()> {
    if depth > MIGRATE_MAX_DEPTH {
        return Ok(());
    }
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let dst = to.join(entry.file_name());
        if kind.is_dir() {
            copy_dir(&entry.path(), &dst, depth + 1)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), &dst)?;
        }
    }
    Ok(())
}

/// The first portable marker file in `exe_dir`, if any.
pub fn portable_marker(exe_dir: &Path) -> Option<PathBuf> {
    PORTABLE_MARKERS.iter().map(|m| exe_dir.join(m)).find(|p| p.is_file())
}

/// Create `dir` and prove a file can be written in it (a folder can exist yet be read-only).
fn ensure_writable(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let probe = dir.join(format!(".write-test-{}", std::process::id()));
    std::fs::write(&probe, b"")?;
    // A leftover probe is harmless; don't fail portable mode over it.
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first Artheffect launch copies an EffectCraft data folder once, and never overwrites.
    #[test]
    fn legacy_effectcraft_data_is_copied_once() {
        let root = std::env::temp_dir().join(format!("artheffect-migrate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (old, new) = (root.join("EffectCraft"), root.join("Artheffect"));
        std::fs::create_dir_all(old.join("Logs")).unwrap();
        std::fs::write(old.join("prefs.json"), b"{\"gl\":true}").unwrap();
        std::fs::write(old.join("Logs").join("a.log"), b"x").unwrap();
        assert!(migrate_legacy(&old, &new).unwrap());
        assert_eq!(std::fs::read(new.join("prefs.json")).unwrap(), b"{\"gl\":true}");
        assert_eq!(std::fs::read(new.join("Logs").join("a.log")).unwrap(), b"x");
        std::fs::write(new.join("prefs.json"), b"{}").unwrap();
        assert!(!migrate_legacy(&old, &new).unwrap(), "an existing folder is left alone");
        assert_eq!(std::fs::read(new.join("prefs.json")).unwrap(), b"{}");
        assert!(!migrate_legacy(&root.join("missing"), &root.join("other")).unwrap());
        assert!(!root.join("other").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn temp(tag: &str) -> PathBuf {
        static N: AtomicUsize = AtomicUsize::new(0);
        let d = std::env::temp_dir().join(format!("artheffect-app-dirs-{tag}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn env_of(pairs: &[(&str, &Path)]) -> impl Fn(&str) -> Option<OsString> + use<> {
        let m: HashMap<String, OsString> = pairs.iter().map(|(k, v)| (k.to_string(), v.as_os_str().to_owned())).collect();
        move |k| m.get(k).cloned()
    }

    /// A fake user folder that every platform's convention resolves under.
    fn fake_home(root: &Path) -> impl Fn(&str) -> Option<OsString> + use<> {
        let home = root.join("home");
        let appdata = root.join("appdata");
        env_of(&[("HOME", &home), ("APPDATA", &appdata)])
    }

    #[test]
    fn no_marker_uses_the_platform_dir() {
        let root = temp("platform");
        let exe = root.join("bin");
        std::fs::create_dir_all(&exe).unwrap();
        let d = resolve(fake_home(&root), Some(&exe));
        assert_eq!(d.mode, Mode::Platform);
        assert!(d.warning.is_none());
        let dir = d.dir.unwrap();
        if cfg!(target_os = "macos") {
            assert_eq!(dir, root.join("home/Library/Application Support/Artheffect"));
        } else if cfg!(target_os = "windows") {
            assert_eq!(dir, root.join("appdata").join("Artheffect"));
        } else {
            assert_eq!(dir, root.join("home/.config/artheffect"));
        }
        assert!(!exe.join(PORTABLE_DATA_DIR).exists(), "no portable folder without a marker");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn either_marker_keeps_everything_beside_the_exe() {
        for marker in PORTABLE_MARKERS {
            let root = temp("portable");
            let exe = root.join("Artheffect");
            std::fs::create_dir_all(&exe).unwrap();
            std::fs::write(exe.join(marker), b"").unwrap();
            let d = resolve(fake_home(&root), Some(&exe));
            assert_eq!(d.mode, Mode::Portable, "{marker}");
            assert!(d.warning.is_none());
            let dir = d.dir.unwrap();
            assert_eq!(dir, exe.join("ArtheffectData"));
            assert!(dir.is_dir(), "the data folder is created");
            assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "the write probe is removed");
            assert!(!root.join("home").exists() && !root.join("appdata").exists(), "nothing written to the user folder");
            let _ = std::fs::remove_dir_all(&root);
        }
    }

    #[test]
    fn a_marker_folder_is_not_a_marker() {
        let root = temp("dir-marker");
        let exe = root.join("bin");
        std::fs::create_dir_all(exe.join("portable.txt")).unwrap();
        assert_eq!(resolve(fake_home(&root), Some(&exe)).mode, Mode::Platform);
        assert!(!exe.join(PORTABLE_DATA_DIR).exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unwritable_portable_folder_falls_back_with_a_warning() {
        let root = temp("unwritable");
        let exe = root.join("bin");
        std::fs::create_dir_all(&exe).unwrap();
        std::fs::write(exe.join("portable.txt"), b"").unwrap();
        // A file where the data folder should go: it can't be created, on every platform.
        std::fs::write(exe.join(PORTABLE_DATA_DIR), b"in the way").unwrap();
        let env = fake_home(&root);
        let d = resolve(&env, Some(&exe));
        assert_eq!(d.mode, Mode::Platform);
        assert_eq!(d.dir, platform_dir(&env));
        assert!(d.dir.is_some());
        let w = d.warning.unwrap();
        assert!(w.contains("portable") && w.contains("isn't writable"), "{w}");
        // No user folder at all: still no panic, just no directory.
        let d = resolve(|_| None, Some(&exe));
        assert!(d.dir.is_none() && d.warning.is_some());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn override_wins_over_portable() {
        let root = temp("override");
        let exe = root.join("bin");
        std::fs::create_dir_all(&exe).unwrap();
        std::fs::write(exe.join("portable.txt"), b"").unwrap();
        let custom = root.join("custom");
        let d = resolve(env_of(&[("ARTHEFFECT_CONFIG_DIR", &custom)]), Some(&exe));
        assert_eq!((d.mode, d.dir), (Mode::Override, Some(custom)));
        assert!(!exe.join(PORTABLE_DATA_DIR).exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_exe_dir_or_env_never_panics() {
        let d = resolve(|_| None, None);
        assert_eq!((d.mode, d.dir), (Mode::Platform, None));
    }
}
