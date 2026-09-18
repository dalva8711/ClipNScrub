# Contributing

Use synthetic test strings only. Do not attach real copied credentials or
customer data to issues, screenshots, tests, logs, or pull requests.

1. Install the prerequisites in README.md and run `npm ci`.
2. Keep detection changes in the pure Rust engine, with positive and negative
   fixtures demonstrating precision, syntax preservation, and idempotency.
3. Run the build, tests, clippy, and formatting checks in README.md.
4. For desktop integration changes, complete the macOS and Windows scenarios in
   docs/VALIDATION.md and state which platforms you actually tested.

Do not add telemetry, remote detection, clipboard history, or raw-content IPC.
Keep idle work event-driven and dependencies small. New provider patterns need
bounded matching and examples that cannot be mistaken for real credentials.

Bug reports should include OS version, app version, detector settings, and a
synthetic reproducer. Security reports should describe the problem without
posting live secrets; use GitHub private vulnerability reporting if enabled.
