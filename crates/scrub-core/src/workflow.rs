use crate::{
    engine::{Report, Sanitizer, MAX_TEXT_BYTES},
    Settings,
};

#[derive(Debug)]
pub struct ClipboardError;

pub trait Clipboard {
    /// None means no text representation, not an operating-system error.
    fn read_text(&mut self) -> Result<Option<String>, ClipboardError>;
    /// Must replace all existing representations, not just add a text flavor.
    fn replace_text(&mut self, text: String) -> Result<(), ClipboardError>;
}

pub fn sanitize_clipboard(clipboard: &mut impl Clipboard, settings: &Settings) -> Report {
    if !settings.enabled {
        return Report::status("disabled", "ClipNScrub is paused. Enable it from the tray.");
    }
    let original = match clipboard.read_text() {
        Ok(Some(s)) => s,
        Ok(None) => return Report::status("non_text", "The clipboard has no text to sanitize."),
        Err(ClipboardError) => {
            return Report::status("error", "Could not read the clipboard. Try again.")
        }
    };
    if original.is_empty() {
        return Report::status("empty", "The clipboard is empty.");
    }
    if original.len() > MAX_TEXT_BYTES {
        return Report::status(
            "too_large",
            "Clipboard text exceeds 1 MiB. Copy a smaller selection.",
        );
    }
    let sanitized = match Sanitizer::new(settings).and_then(|s| s.sanitize(&original)) {
        Ok(s) => s,
        Err(_) => {
            return Report::status("error", "Sanitization failed. Clipboard was not changed.")
        }
    };
    if sanitized.replacements() == 0 {
        return Report::status("unchanged", "No matches found. Clipboard was not changed.");
    }
    match clipboard.read_text() {
        Ok(Some(current)) if current == original => (),
        Ok(_) => {
            return Report::status(
                "changed",
                "Clipboard changed during sanitization. Try again.",
            )
        }
        Err(ClipboardError) => {
            return Report::status(
                "error",
                "Could not verify the clipboard. Nothing was written.",
            )
        }
    }
    let count = sanitized.replacements();
    if clipboard.replace_text(sanitized.text).is_err() {
        return Report::status(
            "error",
            "Could not replace the clipboard. Copy the source and try again.",
        );
    }
    Report {
        status: "sanitized".into(),
        message: format!(
            "Sanitized {count} match{}. Ready to paste.",
            if count == 1 { "" } else { "es" }
        ),
        counts: sanitized.counts,
    }
}

pub trait ShortcutRegistry {
    fn register(&mut self, shortcut: &str) -> Result<(), String>;
    fn unregister(&mut self, shortcut: &str) -> Result<(), String>;
}

/// Register the replacement first so a conflict cannot remove the working shortcut.
pub fn transition_shortcut(
    registry: &mut impl ShortcutRegistry,
    old: &Settings,
    new: &Settings,
) -> Result<(), String> {
    if old.enabled == new.enabled && old.shortcut == new.shortcut {
        return Ok(());
    }
    if new.enabled {
        registry.register(&new.shortcut)?;
    }
    if old.enabled {
        if let Err(error) = registry.unregister(&old.shortcut) {
            if new.enabled {
                let _ = registry.unregister(&new.shortcut);
            }
            return Err(error);
        }
    }
    Ok(())
}
