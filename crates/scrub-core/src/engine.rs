use crate::Settings;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use regex::{Regex, RegexBuilder};
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashMap},
    net::IpAddr,
    sync::LazyLock,
};

pub const MAX_TEXT_BYTES: usize = 1024 * 1024;

fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("built-in regex must compile")
}

static PRIVATE: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"(?s)-----BEGIN (?:RSA |EC |DSA |OPENSSH |ENCRYPTED )?PRIVATE KEY-----.*?-----END (?:RSA |EC |DSA |OPENSSH |ENCRYPTED )?PRIVATE KEY-----",
    )
});
static DATABASE: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r#"(?i)\b(?:postgres(?:ql)?|mysql|mariadb|mongodb(?:\+srv)?|redis|rediss|mssql|sqlserver|oracle|cockroachdb)://[^\s<>"'`]+"#,
    )
});
static AWS: LazyLock<Regex> = LazyLock::new(|| regex(r"\b(?:AKIA|ASIA|AIDA|AROA)[A-Z0-9]{16}\b"));
static JWT: LazyLock<Regex> =
    LazyLock::new(|| regex(r"\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b"));
static PROVIDER: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"\b(?:gh[pousr]_[A-Za-z0-9]{20,255}|github_pat_[A-Za-z0-9_]{20,255}|sk-(?:proj-|ant-api\d+-)?[A-Za-z0-9_-]{20,255}|[sr]k_(?:live|test)_[A-Za-z0-9]{16,255}|xox[baprs]-[A-Za-z0-9-]{16,255}|AIza[A-Za-z0-9_-]{35})\b",
    )
});
static EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"\b[A-Za-z0-9.!#$%&'*+/?^_`{|}~-]+@[A-Za-z0-9](?:[A-Za-z0-9.-]*[A-Za-z0-9])?\.[A-Za-z]{2,63}\b",
    )
});
static IPV4: LazyLock<Regex> = LazyLock::new(|| regex(r"\b(?:[0-9]{1,3}\.){3}[0-9]{1,3}\b"));
static IPV6: LazyLock<Regex> = LazyLock::new(|| regex(r"[0-9A-Fa-f:.]*:[0-9A-Fa-f:.]+"));
// Capture only values, preserving assignment syntax, surrounding quotes and whitespace.
static FIELD: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r#"(?im)(?:^|[\s{,;])['"]?([A-Za-z_][A-Za-z0-9_.-]*)['"]?[ \t]*[:=][ \t]*(?:"([^"\r\n]*)"|'([^'\r\n]*)'|([^\s,;\}\]"']+))"#,
    )
});
static PLACEHOLDER: LazyLock<Regex> =
    LazyLock::new(|| regex(r"<(?:REDACTED|(?:EMAIL|IP|CUSTOMER_ID)_[0-9]+)>"));

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Kind {
    Secret,
    Email,
    Ip,
    Customer,
}
impl Kind {
    fn label(self) -> &'static str {
        match self {
            Self::Secret => "secrets",
            Self::Email => "emails",
            Self::Ip => "ip_addresses",
            Self::Customer => "customer_ids",
        }
    }
    fn placeholder(self) -> &'static str {
        match self {
            Self::Secret => "REDACTED",
            Self::Email => "EMAIL",
            Self::Ip => "IP",
            Self::Customer => "CUSTOMER_ID",
        }
    }
}

struct Hit {
    start: usize,
    end: usize,
    kind: Kind,
    priority: u8,
}

#[derive(Debug)]
pub struct Sanitized {
    pub text: String,
    pub counts: BTreeMap<String, usize>,
}
impl Sanitized {
    pub fn replacements(&self) -> usize {
        self.counts.values().sum()
    }
}

pub struct Sanitizer {
    settings: Settings,
    custom: Vec<Regex>,
}

impl Sanitizer {
    pub fn new(settings: &Settings) -> Result<Self, String> {
        if settings.customer_patterns.len() > 20 {
            return Err("Use at most 20 customer-ID patterns.".into());
        }
        let mut custom = Vec::new();
        for (index, pattern) in settings.customer_patterns.iter().enumerate() {
            if pattern.len() > 1024 {
                return Err(format!(
                    "Customer-ID pattern {} exceeds 1,024 bytes.",
                    index + 1
                ));
            }
            let re = RegexBuilder::new(pattern).size_limit(1024 * 1024).build()
                .map_err(|_| format!("Customer-ID pattern {} is invalid or too complex. Use Rust regex syntax (no lookaround or backreferences).", index + 1))?;
            if re.is_match("") {
                return Err(format!(
                    "Customer-ID pattern {} must not match empty text.",
                    index + 1
                ));
            }
            custom.push(re);
        }
        Ok(Self {
            settings: settings.clone(),
            custom,
        })
    }

    pub fn sanitize(&self, input: &str) -> Result<Sanitized, String> {
        if input.len() > MAX_TEXT_BYTES {
            return Err("Clipboard text exceeds the 1 MiB limit.".into());
        }
        let d = &self.settings.detectors;
        let mut hits = Vec::new();
        let mut add = |re: &Regex, kind, priority| {
            for m in re.find_iter(input) {
                hits.push(Hit {
                    start: m.start(),
                    end: m.end(),
                    kind,
                    priority,
                });
            }
        };
        if d.private_key {
            add(&PRIVATE, Kind::Secret, 0);
        }
        if d.aws {
            add(&AWS, Kind::Secret, 2);
        }
        if d.api_key {
            add(&PROVIDER, Kind::Secret, 2);
        }
        if d.email {
            add(&EMAIL, Kind::Email, 4);
        }
        if d.customer_id {
            for re in &self.custom {
                add(re, Kind::Customer, 3);
            }
        }
        if d.database_url {
            for m in DATABASE.find_iter(input) {
                let value = m.as_str().trim_end_matches([',', ';', ')', ']', '}', '.']);
                hits.push(Hit {
                    start: m.start(),
                    end: m.start() + value.len(),
                    kind: Kind::Secret,
                    priority: 1,
                });
            }
        }
        if d.jwt {
            for m in JWT.find_iter(input) {
                let header = m.as_str().split('.').next().unwrap_or_default();
                let valid = URL_SAFE_NO_PAD
                    .decode(header)
                    .ok()
                    .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                    .is_some_and(|v| v.get("alg").is_some());
                if valid {
                    hits.push(Hit {
                        start: m.start(),
                        end: m.end(),
                        kind: Kind::Secret,
                        priority: 2,
                    });
                }
            }
        }
        if d.ip {
            for re in [&*IPV4, &*IPV6] {
                for m in re.find_iter(input) {
                    let value = m.as_str().trim_end_matches('.');
                    if value.parse::<IpAddr>().is_ok() {
                        hits.push(Hit {
                            start: m.start(),
                            end: m.start() + value.len(),
                            kind: Kind::Ip,
                            priority: 4,
                        });
                    }
                }
            }
        }
        for c in FIELD.captures_iter(input) {
            let raw = &c[1];
            let key: String = raw
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect();
            let value = (2..=4).find_map(|n| c.get(n)).expect("field value");
            if value.is_empty() || PLACEHOLDER.is_match(value.as_str()) {
                continue;
            }
            let aws = key.contains("aws")
                && (key.contains("secret")
                    || key.contains("accesskey")
                    || key.contains("sessiontoken"));
            let explicit = [
                "apikey",
                "apitoken",
                "accesstoken",
                "authtoken",
                "clientsecret",
                "password",
                "passwd",
                "secretkey",
                "privatekey",
            ]
            .iter()
            .any(|k| key.ends_with(k));
            let contextual = ["secret", "token", "credential"]
                .iter()
                .any(|k| key.contains(k));
            let customer = key.ends_with("customerid")
                || key.ends_with("customeridentifier")
                || key.ends_with("custid");
            let database = key.ends_with("databaseurl")
                || key.ends_with("dburl")
                || key.ends_with("databaseuri")
                || key.ends_with("connectionstring");
            let kind = if (d.aws && aws)
                || (d.api_key
                    && !aws
                    && (explicit
                        || (contextual && value.len() >= 20 && entropy(value.as_str()) >= 3.5)))
                || (d.database_url && database)
            {
                Some(Kind::Secret)
            } else if d.customer_id && customer {
                Some(Kind::Customer)
            } else {
                None
            };
            if let Some(kind) = kind {
                hits.push(Hit {
                    start: value.start(),
                    end: value.end(),
                    kind,
                    priority: 1,
                });
            }
        }
        let protected: Vec<_> = PLACEHOLDER.find_iter(input).map(|m| m.range()).collect();
        hits.retain(|h| {
            h.end > h.start && !protected.iter().any(|p| h.start < p.end && p.start < h.end)
        });
        // Earlier priority wins; longer matches win within one priority. BTreeMap
        // keeps overlap resolution O(n log n), including large adversarial inputs.
        hits.sort_by_key(|h| (h.priority, std::cmp::Reverse(h.end - h.start), h.start));
        let mut accepted: BTreeMap<usize, Hit> = BTreeMap::new();
        for hit in hits {
            let overlaps_left = accepted
                .range(..=hit.start)
                .next_back()
                .is_some_and(|(_, h)| h.end > hit.start);
            let overlaps_right = accepted.range(hit.start..hit.end).next().is_some();
            if !overlaps_left && !overlaps_right {
                accepted.insert(hit.start, hit);
            }
        }
        let mut output = String::with_capacity(input.len());
        let mut cursor = 0;
        let mut mappings: HashMap<(Kind, &str), String> = HashMap::new();
        let mut numbering: HashMap<Kind, usize> = HashMap::new();
        let mut counts = BTreeMap::new();
        for hit in accepted.values() {
            output.push_str(&input[cursor..hit.start]);
            let original = &input[hit.start..hit.end];
            let replacement = mappings.entry((hit.kind, original)).or_insert_with(|| {
                if hit.kind == Kind::Secret {
                    "<REDACTED>".into()
                } else {
                    let n = numbering.entry(hit.kind).or_default();
                    *n += 1;
                    format!("<{}_{}>", hit.kind.placeholder(), n)
                }
            });
            output.push_str(replacement);
            *counts.entry(hit.kind.label().to_string()).or_default() += 1;
            cursor = hit.end;
        }
        output.push_str(&input[cursor..]);
        Ok(Sanitized {
            text: output,
            counts,
        })
    }
}

fn entropy(value: &str) -> f64 {
    let mut counts = [0usize; 256];
    for byte in value.bytes() {
        counts[byte as usize] += 1;
    }
    counts
        .iter()
        .filter(|&&n| n > 0)
        .map(|&n| {
            let p = n as f64 / value.len() as f64;
            -p * p.log2()
        })
        .sum()
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Report {
    pub status: String,
    pub message: String,
    pub counts: BTreeMap<String, usize>,
}
impl Report {
    pub fn status(status: &str, message: &str) -> Self {
        Self {
            status: status.into(),
            message: message.into(),
            counts: BTreeMap::new(),
        }
    }
}
