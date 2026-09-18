#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod preferences;

use scrub_core::{
    engine::{Report, Sanitizer},
    workflow::{self, ClipboardError, ShortcutRegistry},
    Settings,
};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_notification::NotificationExt;

struct Runtime {
    settings: Mutex<Settings>,
    operation: Mutex<()>,
    last_report: Mutex<Report>,
}

#[derive(Serialize)]
struct Snapshot {
    settings: Settings,
    report: Report,
}

struct NativeClipboard(arboard::Clipboard);
impl workflow::Clipboard for NativeClipboard {
    fn read_text(&mut self) -> Result<Option<String>, ClipboardError> {
        match self.0.get_text() {
            Ok(text) => Ok(Some(text)),
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(_) => Err(ClipboardError),
        }
    }
    fn replace_text(&mut self, text: String) -> Result<(), ClipboardError> {
        // arboard's macOS/Windows set_text replaces the pasteboard contents.
        self.0.set_text(text).map_err(|_| ClipboardError)
    }
}

struct Shortcuts<'a>(&'a tauri::AppHandle);
impl ShortcutRegistry for Shortcuts<'_> {
    fn register(&mut self, shortcut: &str) -> Result<(), String> {
        self.0
            .global_shortcut()
            .register(shortcut)
            .map_err(|_| "Shortcut unavailable. Choose a different shortcut in Settings.".into())
    }
    fn unregister(&mut self, shortcut: &str) -> Result<(), String> {
        self.0
            .global_shortcut()
            .unregister(shortcut)
            .map_err(|_| "Could not release the shortcut. Restart ClipNScrub and try again.".into())
    }
}

fn show_settings(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let result = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("index.html".into()))
        .title("ClipNScrub")
        .inner_size(760.0, 850.0)
        .min_inner_size(600.0, 580.0)
        .on_navigation(|url| {
            matches!(url.scheme(), "tauri" | "http" | "https")
                && matches!(
                    url.host_str(),
                    Some("tauri.localhost" | "localhost" | "127.0.0.1")
                )
        })
        .build();
    if result.is_err() {
        publish(
            app,
            Report::status("error", "Could not open Settings. Restart ClipNScrub."),
        );
    }
}

fn tray_image(enabled: bool) -> tauri::image::Image<'static> {
    // Small template shield; paused state uses two bars instead of a check.
    let mut rgba = vec![0; 32 * 32 * 4];
    for y in 0..32usize {
        for x in 0..32usize {
            let border = ((5..=26).contains(&x) && (y == 4 || y == 5))
                || ((x == 5 || x == 6 || x == 25 || x == 26) && (5..=20).contains(&y))
                || ((21..=29).contains(&y) && ((x as i32 - 16).abs() == (29 - y) as i32));
            let mark = if enabled {
                ((10..=15).contains(&x) && y == x + 4) || ((15..=23).contains(&x) && y == 34 - x)
            } else {
                (x == 12 || x == 13 || x == 19 || x == 20) && (11..=21).contains(&y)
            };
            if border || mark {
                let i = (y * 32 + x) * 4;
                #[cfg(target_os = "macos")]
                let color = [0, 0, 0, 255];
                #[cfg(not(target_os = "macos"))]
                let color = if enabled {
                    [46, 211, 158, 255]
                } else {
                    [170, 170, 170, 255]
                };
                rgba[i..i + 4].copy_from_slice(&color);
            }
        }
    }
    tauri::image::Image::new_owned(rgba, 32, 32)
}

fn refresh_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let enabled = app.state::<Runtime>().settings.lock().unwrap().enabled;
    let status = app
        .state::<Runtime>()
        .last_report
        .lock()
        .unwrap()
        .message
        .clone();
    let toggle = MenuItem::with_id(
        app,
        "toggle",
        if enabled {
            "Pause ClipNScrub"
        } else {
            "Enable ClipNScrub"
        },
        true,
        None::<&str>,
    )?;
    let sanitize = MenuItem::with_id(app, "sanitize", "Sanitize Clipboard", enabled, None::<&str>)?;
    let last = MenuItem::with_id(app, "status", &status, false, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit ClipNScrub", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &sanitize, &last, &settings, &quit])?;
    if let Some(tray) = app.tray_by_id("main") {
        tray.set_menu(Some(menu))?;
        tray.set_icon(Some(tray_image(enabled)))?;
        tray.set_tooltip(Some(if enabled {
            "ClipNScrub · Enabled"
        } else {
            "ClipNScrub · Paused"
        }))?;
    }
    Ok(())
}

fn publish(app: &tauri::AppHandle, report: Report) {
    *app.state::<Runtime>().last_report.lock().unwrap() = report.clone();
    let _ = app.emit("status", &report);
    let _ = refresh_tray(app);
    // Native notifications do not activate another app. The tray retains status
    // when notifications are denied or suppressed by the OS.
    let _ = app
        .notification()
        .builder()
        .title("ClipNScrub")
        .body(&report.message)
        .show();
}

#[tauri::command]
fn get_snapshot(app: tauri::AppHandle) -> Snapshot {
    let runtime = app.state::<Runtime>();
    let settings = runtime.settings.lock().unwrap().clone();
    let report = runtime.last_report.lock().unwrap().clone();
    Snapshot { settings, report }
}

fn apply_settings(app: &tauri::AppHandle, new: Settings) -> Result<Settings, String> {
    Sanitizer::new(&new)?;
    if new.shortcut.len() > 100 || new.shortcut.parse::<Shortcut>().is_err() {
        return Err("Enter a valid shortcut, for example CommandOrControl+Shift+S.".into());
    }
    let runtime = app.state::<Runtime>();
    let _operation = runtime.operation.lock().unwrap();
    let mut current = runtime.settings.lock().unwrap();
    workflow::transition_shortcut(&mut Shortcuts(app), &current, &new)?;
    if let Err(error) = preferences::save(app, &new) {
        if workflow::transition_shortcut(&mut Shortcuts(app), &new, &current).is_err() {
            let _ = app.global_shortcut().unregister_all();
            current.enabled = false;
            let _ = preferences::save(app, &current);
            return Err("Settings could not be saved and the shortcut could not be restored. ClipNScrub is paused; restart it.".into());
        }
        return Err(error);
    }
    *current = new.clone();
    drop(current);
    let _ = refresh_tray(app);
    let _ = app.emit("settings-changed", &new);
    Ok(new)
}

#[tauri::command]
fn save_settings(app: tauri::AppHandle, settings: Settings) -> Result<Settings, String> {
    apply_settings(&app, settings)
}

#[tauri::command]
fn set_enabled(app: tauri::AppHandle, enabled: bool) -> Result<Settings, String> {
    let mut settings = app.state::<Runtime>().settings.lock().unwrap().clone();
    settings.enabled = enabled;
    apply_settings(&app, settings)
}

#[tauri::command]
async fn sanitize_clipboard(app: tauri::AppHandle) -> Report {
    let handle = app.clone();
    let report = tauri::async_runtime::spawn_blocking(move || {
        let runtime = handle.state::<Runtime>();
        let Ok(_operation) = runtime.operation.try_lock() else {
            return Report::status("busy", "Sanitization is already running.");
        };
        let settings = runtime.settings.lock().unwrap().clone();
        if !settings.enabled {
            return Report::status("disabled", "ClipNScrub is paused. Enable it from the tray.");
        }
        match arboard::Clipboard::new() {
            Ok(clipboard) => {
                workflow::sanitize_clipboard(&mut NativeClipboard(clipboard), &settings)
            }
            Err(_) => Report::status("error", "Could not access the clipboard. Try again."),
        }
    })
    .await
    .unwrap_or_else(|_| {
        Report::status(
            "error",
            "Sanitization failed. Copy the source and try again.",
        )
    });
    publish(&app, report.clone());
    report
}

fn trigger(app: &tauri::AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        sanitize_clipboard(app).await;
    });
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show_settings(app)))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(|app, _, event| {
            if event.state() == ShortcutState::Pressed { trigger(app); }
        }).build())
        .invoke_handler(tauri::generate_handler![get_snapshot, save_settings, set_enabled, sanitize_clipboard])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let (mut settings, first_launch, warning) = preferences::load(app.handle());
            let mut report = Report::status("ready", "Copy → sanitize → paste. Your clipboard stays local.");
            if let Some(message) = warning { report = Report::status("error", message); }
            if settings.enabled && Shortcuts(app.handle()).register(&settings.shortcut).is_err() {
                settings.enabled = false;
                report = Report::status("error", "Shortcut unavailable. ClipNScrub is paused; choose a different shortcut in Settings.");
            }
            app.manage(Runtime { settings: Mutex::new(settings.clone()), operation: Mutex::new(()), last_report: Mutex::new(report.clone()) });
            TrayIconBuilder::with_id("main").icon(tray_image(settings.enabled)).icon_as_template(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "sanitize" => trigger(app),
                    "settings" => show_settings(app),
                    "quit" => app.exit(0),
                    "toggle" => {
                        let enabled = !app.state::<Runtime>().settings.lock().unwrap().enabled;
                        if let Err(message) = set_enabled(app.clone(), enabled) { publish(app, Report::status("error", &message)); }
                    },
                    _ => (),
                }).build(app)?;
            refresh_tray(app.handle())?;
            if first_launch || report.status == "error" { show_settings(app.handle()); }
            if first_launch { if let Err(message) = preferences::save(app.handle(), &settings) { publish(app.handle(), Report::status("error", &message)); } }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Could not initialize ClipNScrub")
        .run(|app, event| {
            // The tray owns app lifetime; closing Settings destroys its webview.
            match event {
                tauri::RunEvent::ExitRequested { api, code: None, .. } => api.prevent_exit(),
                #[cfg(target_os = "macos")]
                tauri::RunEvent::Reopen { has_visible_windows: false, .. } => show_settings(app),
                _ => { let _ = app; },
            }
        });
}
