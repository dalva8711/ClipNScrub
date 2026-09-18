# Validation checklist

## Automated checks

`cargo test --workspace --locked` covers eight detection categories, format
preservation, repeated values, overlaps, Unicode, idempotency, disabled detectors,
entropy false positives, invalid patterns, byte limits, clipboard races and
errors, shortcut conflicts, enable/disable transitions, and preference persistence.

CI runs on macOS Apple Silicon, macOS Intel, and Windows x64. A build passing
does **not** substitute for the following real desktop checks.

## Desktop acceptance (run on both macOS and Windows)

Use only synthetic source data. Record date, OS version, app build, and outcome.

- [ ] First launch shows onboarding; subsequent launch stays in the tray.
- [ ] Closing Settings unloads the window; shortcut and tray remain functional.
- [ ] Reopening Settings works; launching a second copy opens existing Settings.
- [ ] Copy the README example with synthetic originals in TextEdit/Notepad,
      invoke the global shortcut, then paste into a browser text field. Verify
      keys and layout remain, and values have the expected placeholders.
- [ ] Copy browser rich text containing `alice@example.com`. Sanitize via tray,
      paste into TextEdit/Word, and confirm plain sanitized text with no original
      HTML/RTF data. Inspect OS clipboard formats where possible.
- [ ] Notification never takes focus or exposes values; status remains visible
      in tray and Settings if OS notifications are denied.
- [ ] Pause: icon changes, sanitize is disabled, shortcut no longer acts. Resume
      restores it. Restart preserves the state.
- [ ] Remap shortcut, test it globally; occupy a shortcut in another app and
      confirm a conflict is reported without losing the old working shortcut.
- [ ] Toggle detectors; save and reopen. Add a valid customer-ID regex. Invalid
      regex must report an error while retaining the last saved configuration.
- [ ] Empty clipboard, image/file-only clipboard, text over 1 MiB, and unchanged
      text report accurate outcomes and do not replace clipboard content.
- [ ] Copy rapidly during a long operation; stale text should not be written
      when the verification detects a change. Do not claim atomic OS behavior.
- [ ] Trigger rapidly; operations do not run concurrently or duplicate replacements.
- [ ] Quit exits fully and releases the shortcut.
- [ ] Disconnect network and repeat copy → sanitize → paste.
- [ ] Check idle CPU and memory with Settings closed, and time a 1 MiB operation.

## Release status

Unsigned development builds only. Signing, notarization, public release, and
Windows interactive acceptance require separate validation before distribution.

### Local validation — 2026-09-18

- Apple Silicon host: all 22 Rust tests passed, strict Clippy passed, TypeScript
  check and Vite production build passed.
- Apple Silicon and Intel macOS `.app` packages built successfully. The Apple
  Silicon app bundle is approximately 5.5 MiB (excluding system webview).
- Native settings window launched and displayed enabled detectors and shortcut.
- Synthetic environment text sanitized to the exact three-line README example.
- Bold synthetic email text copied from TextEdit sanitized through the app action
  and pasted into a new TextEdit document as plain text with `<EMAIL_1>`.
- Pause/resume changed the switch and disabled/enabled the sanitize action.
- Invalid custom regex rejected; valid empty pattern list saved successfully.
- Global shortcut registration succeeded, but automated application-targeted
  keystrokes did not exercise the OS hotkey path. Physical-keyboard validation
  remains required; this is not recorded as a passing shortcut integration test.
- Windows GUI checks and Intel hardware launch checks have not been run locally.
