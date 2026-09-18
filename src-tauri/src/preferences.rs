use scrub_core::{engine::Sanitizer, Settings};
use std::{fs, io::Write, path::PathBuf};
use tauri::Manager;

fn path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|p| p.join("settings.json"))
        .map_err(|_| "Could not locate the preferences directory.".into())
}

pub fn load(app: &tauri::AppHandle) -> (Settings, bool, Option<&'static str>) {
    let result = path(app).and_then(|p| match fs::read(p) {
        Ok(bytes) => serde_json::from_slice::<Settings>(&bytes)
            .map(Some)
            .map_err(|_| "Invalid preferences".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("Unreadable preferences".into()),
    });
    match result {
        Ok(Some(s)) if Sanitizer::new(&s).is_ok() => (s, false, None),
        Ok(None) => (Settings::default(), true, None),
        _ => {
            let s = Settings {
                enabled: false,
                ..Settings::default()
            };
            (s, false, Some("Preferences could not be loaded. ClipNScrub is paused; review and save Settings."))
        }
    }
}

pub fn save(app: &tauri::AppHandle, settings: &Settings) -> Result<(), String> {
    save_at(&path(app)?, settings)
        .map_err(|_| "Could not save preferences. Previous settings are still active.".into())
}

fn save_at(path: &std::path::Path, settings: &Settings) -> Result<(), Box<dyn std::error::Error>> {
    let parent = path.parent().ok_or("missing preferences directory")?;
    fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(&serde_json::to_vec_pretty(settings)?)?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_replace_existing_file_without_clipboard_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        save_at(&path, &Settings::default()).unwrap();
        let paused = Settings {
            enabled: false,
            ..Settings::default()
        };
        save_at(&path, &paused).unwrap();
        assert_eq!(
            serde_json::from_slice::<Settings>(&fs::read(path).unwrap()).unwrap(),
            paused
        );
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
