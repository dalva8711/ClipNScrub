use scrub_core::{
    engine::MAX_TEXT_BYTES,
    workflow::{
        sanitize_clipboard, transition_shortcut, Clipboard, ClipboardError, ShortcutRegistry,
    },
    Settings,
};
use std::collections::VecDeque;

struct FakeClipboard {
    reads: VecDeque<Result<Option<String>, ClipboardError>>,
    writes: Vec<String>,
    fail_write: bool,
}
impl FakeClipboard {
    fn new(values: &[Option<&str>]) -> Self {
        Self {
            reads: values.iter().map(|s| Ok(s.map(str::to_owned))).collect(),
            writes: vec![],
            fail_write: false,
        }
    }
}
impl Clipboard for FakeClipboard {
    fn read_text(&mut self) -> Result<Option<String>, ClipboardError> {
        self.reads.pop_front().expect("unexpected clipboard read")
    }
    fn replace_text(&mut self, text: String) -> Result<(), ClipboardError> {
        if self.fail_write {
            Err(ClipboardError)
        } else {
            self.writes.push(text);
            Ok(())
        }
    }
}

#[test]
fn disabled_does_not_read_clipboard() {
    let mut clipboard = FakeClipboard::new(&[]);
    let settings = Settings {
        enabled: false,
        ..Settings::default()
    };
    assert_eq!(
        sanitize_clipboard(&mut clipboard, &settings).status,
        "disabled"
    );
    assert!(clipboard.writes.is_empty());
}

#[test]
fn sanitizes_after_verifying_current_contents() {
    let mut clipboard = FakeClipboard::new(&[Some("API_KEY=secret"), Some("API_KEY=secret")]);
    let report = sanitize_clipboard(&mut clipboard, &Settings::default());
    assert_eq!(report.status, "sanitized");
    assert_eq!(clipboard.writes, ["API_KEY=<REDACTED>"]);
    assert!(!report.message.contains("API_KEY"));
}

#[test]
fn changed_clipboard_is_not_overwritten() {
    for second in [Some("new content"), None] {
        let mut clipboard = FakeClipboard::new(&[Some("API_KEY=secret"), second]);
        assert_eq!(
            sanitize_clipboard(&mut clipboard, &Settings::default()).status,
            "changed"
        );
        assert!(clipboard.writes.is_empty());
    }
}

#[test]
fn untouched_clipboard_outcomes() {
    let large = "x".repeat(MAX_TEXT_BYTES + 1);
    for (text, expected) in [
        (None, "non_text"),
        (Some(""), "empty"),
        (Some("hello"), "unchanged"),
        (Some(large.as_str()), "too_large"),
    ] {
        let mut clipboard = FakeClipboard::new(&[text]);
        assert_eq!(
            sanitize_clipboard(&mut clipboard, &Settings::default()).status,
            expected
        );
        assert!(clipboard.writes.is_empty());
    }
}

#[test]
fn errors_never_report_success_or_expose_source() {
    let mut clipboard = FakeClipboard::new(&[]);
    clipboard.reads.push_back(Err(ClipboardError));
    assert_eq!(
        sanitize_clipboard(&mut clipboard, &Settings::default()).status,
        "error"
    );
    let mut clipboard = FakeClipboard::new(&[Some("API_KEY=secret")]);
    clipboard.reads.push_back(Err(ClipboardError));
    assert_eq!(
        sanitize_clipboard(&mut clipboard, &Settings::default()).status,
        "error"
    );
    assert!(clipboard.writes.is_empty());
    let mut clipboard = FakeClipboard::new(&[Some("API_KEY=secret"), Some("API_KEY=secret")]);
    clipboard.fail_write = true;
    let report = sanitize_clipboard(&mut clipboard, &Settings::default());
    assert_eq!(report.status, "error");
    assert!(report.counts.is_empty());
    assert!(!report.message.contains("API_KEY"));
}

#[derive(Default)]
struct Registry {
    active: Vec<String>,
    blocked: Option<String>,
}
impl ShortcutRegistry for Registry {
    fn register(&mut self, shortcut: &str) -> Result<(), String> {
        if self.blocked.as_deref() == Some(shortcut) {
            return Err("conflict".into());
        }
        self.active.push(shortcut.into());
        Ok(())
    }
    fn unregister(&mut self, shortcut: &str) -> Result<(), String> {
        self.active.retain(|s| s != shortcut);
        Ok(())
    }
}

#[test]
fn shortcut_conflict_keeps_old_registration() {
    let old = Settings::default();
    let new = Settings {
        shortcut: "Control+Shift+X".into(),
        ..old.clone()
    };
    let mut registry = Registry {
        active: vec![old.shortcut.clone()],
        blocked: Some(new.shortcut.clone()),
    };
    assert!(transition_shortcut(&mut registry, &old, &new).is_err());
    assert_eq!(registry.active, [old.shortcut]);
}

#[test]
fn pause_unregisters_and_resume_registers() {
    let old = Settings::default();
    let mut new = old.clone();
    new.enabled = false;
    let mut registry = Registry {
        active: vec![old.shortcut.clone()],
        blocked: None,
    };
    transition_shortcut(&mut registry, &old, &new).unwrap();
    assert!(registry.active.is_empty());
    transition_shortcut(&mut registry, &new, &old).unwrap();
    assert_eq!(registry.active, [old.shortcut]);
}

#[test]
fn shortcut_replacement_and_unchanged_settings() {
    let old = Settings::default();
    let new = Settings {
        shortcut: "Control+Shift+X".into(),
        ..old.clone()
    };
    let mut registry = Registry {
        active: vec![old.shortcut.clone()],
        blocked: None,
    };
    transition_shortcut(&mut registry, &old, &new).unwrap();
    assert_eq!(
        registry.active.as_slice(),
        std::slice::from_ref(&new.shortcut)
    );
    transition_shortcut(&mut registry, &new, &new).unwrap();
    assert_eq!(registry.active, [new.shortcut]);
}
