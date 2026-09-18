use scrub_core::{
    engine::{Sanitizer, MAX_TEXT_BYTES},
    Settings,
};

fn scrub(text: &str) -> String {
    Sanitizer::new(&Settings::default())
        .unwrap()
        .sanitize(text)
        .unwrap()
        .text
}

#[test]
fn preserves_env_json_and_yaml_syntax() {
    assert_eq!(scrub("DATABASE_URL=postgres://user:pw@db.example.com/app\nAPI_KEY=my-example-key\nUSER_EMAIL=alice@example.com"), "DATABASE_URL=<REDACTED>\nAPI_KEY=<REDACTED>\nUSER_EMAIL=<EMAIL_1>");
    assert_eq!(
        scrub(r#"{"apiKey": "test-value", "customerId": "cus_123", "email": "a@example.org"}"#),
        r#"{"apiKey": "<REDACTED>", "customerId": "<CUSTOMER_ID_1>", "email": "<EMAIL_1>"}"#
    );
    assert_eq!(
        scrub("  api_token: 'test-token'\r\n  customer_id: 12345"),
        "  api_token: '<REDACTED>'\r\n  customer_id: <CUSTOMER_ID_1>"
    );
}

#[test]
fn aws_formats_and_context() {
    assert_eq!(scrub("AKIAABCDEFGHIJKLMNOP ASIA1234567890123456\nAWS_SECRET_ACCESS_KEY=short-test\nAWS_SESSION_TOKEN=another-test"), "<REDACTED> <REDACTED>\nAWS_SECRET_ACCESS_KEY=<REDACTED>\nAWS_SESSION_TOKEN=<REDACTED>");
}

#[test]
fn jwt_requires_a_json_header_with_algorithm() {
    let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjMifQ.c2lnbmF0dXJl";
    assert_eq!(scrub(jwt), "<REDACTED>");
    assert_eq!(
        scrub("eyJinvalid.payload.signature"),
        "eyJinvalid.payload.signature"
    );
}

#[test]
fn common_provider_tokens() {
    // Generate obviously synthetic values at runtime; never check complete
    // provider-shaped credentials into source control.
    for (prefix, length) in [
        ("ghp_", 36),
        ("github_pat_", 36),
        ("sk-proj-", 32),
        ("sk-ant-api03-", 32),
        ("sk_live_", 24),
        ("xoxb-", 26),
        ("AIza", 35),
    ] {
        let token = format!("{prefix}{}", "a".repeat(length));
        assert_eq!(
            scrub(&token),
            "<REDACTED>",
            "provider token must be removed"
        );
    }
}

#[test]
fn addresses_number_by_first_occurrence_and_reuse() {
    assert_eq!(
        scrub("b@example.com a@example.com b@example.com 10.0.0.1 ::1 10.0.0.1"),
        "<EMAIL_1> <EMAIL_2> <EMAIL_1> <IP_1> <IP_2> <IP_1>"
    );
    assert_eq!(
        scrub("[2001:db8::1]:443, ::ffff:192.0.2.128"),
        "[<IP_1>]:443, <IP_2>"
    );
    assert_eq!(
        scrub("999.1.2.3 12:34 2026-09-18"),
        "999.1.2.3 12:34 2026-09-18"
    );
}

#[test]
fn multiline_private_keys_take_precedence() {
    for prefix in ["", "RSA ", "EC ", "DSA ", "OPENSSH ", "ENCRYPTED "] {
        let text = format!("before\n-----BEGIN {prefix}PRIVATE KEY-----\nabc\na@example.com\n-----END {prefix}PRIVATE KEY-----\nafter");
        assert_eq!(scrub(&text), "before\n<REDACTED>\nafter");
    }
}

#[test]
fn entire_database_url_wins_over_email_and_ip() {
    assert_eq!(
        scrub("See (postgres://user:pw@db/app), then continue."),
        "See (<REDACTED>), then continue."
    );
    assert_eq!(
        scrub("url: 'postgresql://alice@example.com:secret@127.0.0.1:5432/mydb?sslmode=require'"),
        "url: '<REDACTED>'"
    );
    assert_eq!(
        scrub("mongodb+srv://user:pw@host/db redis://127.0.0.1/0"),
        "<REDACTED> <REDACTED>"
    );
}

#[test]
fn contextual_entropy_does_not_remove_prose_or_unlabeled_identifiers() {
    let value = "aB3dE5fG7hI9jK1lM3nO5pQ7";
    assert_eq!(scrub(value), value);
    assert_eq!(scrub(&format!("secret={value}")), "secret=<REDACTED>");
    assert_eq!(
        scrub("secret=aaaaaaaaaaaaaaaaaaaaaaaa"),
        "secret=aaaaaaaaaaaaaaaaaaaaaaaa"
    );
    let ordinary = "Hello world. build_hash=abcdef0123456789abcdef0123456789\n550e8400-e29b-41d4-a716-446655440000";
    assert_eq!(scrub(ordinary), ordinary);
}

#[test]
fn customer_ids_are_contextual_and_customizable() {
    assert_eq!(
        scrub("customer_id=123 custId=123 customerId=456 id=789"),
        "customer_id=<CUSTOMER_ID_1> custId=<CUSTOMER_ID_1> customerId=<CUSTOMER_ID_2> id=789"
    );
    let settings = Settings {
        customer_patterns: vec![r"\bCUS-\d+\b".into()],
        ..Settings::default()
    };
    assert_eq!(
        Sanitizer::new(&settings)
            .unwrap()
            .sanitize("CUS-123 CUS-123")
            .unwrap()
            .text,
        "<CUSTOMER_ID_1> <CUSTOMER_ID_1>"
    );
}

#[test]
fn invalid_and_empty_custom_patterns_are_rejected() {
    for pattern in ["[", ".*", "", "(?=foo)"] {
        let settings = Settings {
            customer_patterns: vec![pattern.into()],
            ..Settings::default()
        };
        assert!(Sanitizer::new(&settings).is_err());
    }
}

#[test]
fn unicode_and_idempotency() {
    let original =
        "🔐 café 日本語 USER_EMAIL=diego@example.com\nAPI_KEY=secret\ncustomer_id=123\n::1";
    let sanitized = scrub(original);
    assert!(sanitized.starts_with("🔐 café 日本語 USER_EMAIL=<EMAIL_1>"));
    assert_eq!(scrub(&sanitized), sanitized);
    assert_eq!(
        scrub("API_KEY=<REDACTED> USER_EMAIL=<EMAIL_1>"),
        "API_KEY=<REDACTED> USER_EMAIL=<EMAIL_1>"
    );
}

#[test]
fn switches_disable_each_detector() {
    let cases = [
        "AKIAABCDEFGHIJKLMNOP",
        "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjMifQ.c2lnbmF0dXJl",
        "a@example.com",
        "10.0.0.1",
        "API_KEY=secret",
        "postgres://user:pw@host/db",
        "-----BEGIN PRIVATE KEY-----\nabc\n-----END PRIVATE KEY-----",
        "customer_id=123",
    ];
    for (i, text) in cases.iter().enumerate() {
        let mut s = Settings::default();
        let d = &mut s.detectors;
        *[
            &mut d.aws,
            &mut d.jwt,
            &mut d.email,
            &mut d.ip,
            &mut d.api_key,
            &mut d.database_url,
            &mut d.private_key,
            &mut d.customer_id,
        ][i] = false;
        assert_eq!(
            Sanitizer::new(&s).unwrap().sanitize(text).unwrap().text,
            *text
        );
    }
}

#[test]
fn counts_count_occurrences_and_limit_is_bytes() {
    let engine = Sanitizer::new(&Settings::default()).unwrap();
    let result = engine
        .sanitize("a@example.com a@example.com API_KEY=test")
        .unwrap();
    assert_eq!(result.replacements(), 3);
    assert_eq!(result.counts["emails"], 2);
    assert!(engine.sanitize(&"x".repeat(MAX_TEXT_BYTES)).is_ok());
    assert!(engine.sanitize(&"é".repeat(MAX_TEXT_BYTES)).is_err());
}
