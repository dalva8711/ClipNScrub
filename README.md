# ClipNScrub

A small, free, offline clipboard sanitizer for macOS and Windows. Copy text,
sanitize it with a shortcut, then paste with your usual paste command.

```ini
DATABASE_URL=<REDACTED>
API_KEY=<REDACTED>
USER_EMAIL=<EMAIL_1>
```

## Use

1. Open ClipNScrub. The settings window explains the workflow on first launch.
2. Copy text from any app.
3. Press **Cmd+Shift+S** on macOS or **Ctrl+Shift+S** on Windows, or choose
   **Sanitize Clipboard** from the menu bar / system tray.
4. Wait for the result notification (also available in the tray menu), then paste.

Pause or enable the tool from the tray or Settings. Pausing unregisters the
shortcut and blocks sanitization; it does not restore previously removed text.
Settings let you change the shortcut, switch each detector on or off, and add
custom customer-ID patterns. Close Settings to unload its webview; the tray
continues running. Quit from the tray to exit.

If another app owns the shortcut, ClipNScrub reports the conflict. On startup,
it pauses until you configure a usable shortcut. A failed shortcut change keeps
the previous working shortcut. No accessibility permission for simulated paste
is needed: you paste manually. OS notification settings may suppress banners;
the tray and Settings always retain the last outcome.

## Detection

| Category | Behavior |
| --- | --- |
| AWS keys | Known access-key prefixes and labeled secret/session keys |
| JWTs | Three-part token with a JSON header containing `alg` |
| Emails | Common ASCII email formats → `<EMAIL_n>` |
| IP addresses | Parsed IPv4 and IPv6 literals → `<IP_n>` |
| API keys | Common GitHub, OpenAI/Anthropic-style, Stripe, Slack, Google token formats; labeled API keys, passwords, and credentials |
| Database URLs | PostgreSQL, MySQL, MongoDB, Redis, SQL Server, Oracle and related URI formats; labeled database URLs / connection strings |
| Private keys | Complete PEM and OpenSSH private-key blocks |
| Customer IDs | `customer_id`, `customerId`, `custId` and related labels; custom Rust regexes → `<CUSTOMER_ID_n>` |

Credentials become `<REDACTED>`. Larger credential matches take precedence over
embedded emails and IPs. Repeated values share a numbered placeholder within a
single operation; mappings are discarded afterward. Keys, quotes, and line
breaks are preserved. Existing placeholders are left alone.

Entropy detection is deliberately contextual: a value needs at least 20 bytes,
Shannon entropy of at least 3.5 bits per byte, and a secret/token/credential field
name. Unlabeled random strings and UUIDs are not automatically removed. Explicit
labels such as `API_KEY` are redacted even when their value has low entropy.

Custom patterns match the **whole customer ID**, e.g. `\bcus_[A-Za-z0-9]+\b`.
Use one per line, up to 20 patterns and 1,024 bytes per pattern. Rust regex syntax
does not support lookaround or backreferences. Invalid or empty-matching patterns
cannot be saved. Do not paste actual secrets into preference fields.

## Privacy and limits

- Detection runs locally in Rust on explicit invocation, with no network access,
  telemetry, accounts, updater, polling, or clipboard history.
- Only preferences are persisted, in the OS app-config directory under
  `com.clipnscrub.desktop/settings.json`. Clipboard contents and replacement maps
  are never written to application logs, preferences, notifications, or the UI.
- Sanitized text replaces all clipboard formats. Formatting is intentionally
  lost when a replacement occurs; original HTML/RTF is not kept alongside it.
- Empty, non-text, unchanged, or text larger than 1 MiB is left untouched.
- Operations are serialized, and text is re-read before writing. Clipboard APIs
  do not provide a cross-platform atomic compare-and-swap: an external copy in
  the tiny interval between verification and replacement can still race.
- A write failure never reports success. Depending on OS failure timing, the
  clipboard may be empty; copy the source again before retrying.
- Detection is best-effort, not a guarantee that all sensitive information has
  been removed. Review the result before sharing. Encoded, obfuscated, incomplete,
  unknown-format, or image-contained secrets may be missed.
- This app cannot erase originals retained by another clipboard manager,
  clipboard sync service, source app, or OS. It does not securely erase process
  memory; plaintext exists transiently while the operation runs.

## Development

Prerequisites: Node.js 22, npm, and current stable Rust. macOS needs Xcode command
line tools. Windows needs Visual Studio C++ Build Tools and WebView2. See the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

```sh
npm ci
npm run tauri dev
```

```sh
npm run build
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check
```

The pure engine and fake-clipboard tests can run independently:
`cargo test -p scrub-core --locked`. `npm run dev` alone previews the settings
layout but deliberately disables desktop actions. Clipboard access is available
only through the native application.

## Unsigned development packages

macOS Apple Silicon:

```sh
rustup target add aarch64-apple-darwin
npm run tauri build -- --target aarch64-apple-darwin --bundles app
```

macOS Intel (run on macOS, including cross-compilation from Apple Silicon):

```sh
rustup target add x86_64-apple-darwin
npm run tauri build -- --target x86_64-apple-darwin --bundles app
```

Windows x64 (run on Windows):

```powershell
rustup target add x86_64-pc-windows-msvc
npm run tauri build -- --target x86_64-pc-windows-msvc --bundles nsis
```

Packages are written to `target/<target>/release/bundle/`. The Windows installer
includes the offline WebView2 installer (larger download, no runtime download
required). macOS uses the system webview. CI tests and uploads private build
artifacts for all three targets; it does not create public releases. Signing and
notarization are not configured. These development builds may trigger OS trust
prompts and should not be described as signed production releases.

## Architecture

- `crates/scrub-core`: deterministic sanitization, preferences types, testable
  clipboard workflow, shortcut transitions. No OS or network dependencies.
- `src-tauri`: native tray, shortcuts, arboard clipboard adapter, atomic preference
  writes, notifications, single-instance handling and on-demand settings window.
- `src`: dependency-light TypeScript/CSS settings UI. Native commands expose
  settings, enable/disable, sanitization, and result counts; no raw clipboard API.

MIT licensed. See [CONTRIBUTING.md](CONTRIBUTING.md) and
[the manual validation checklist](docs/VALIDATION.md).
