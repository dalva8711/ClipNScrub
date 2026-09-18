pub mod engine;
pub mod workflow;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub enabled: bool,
    pub shortcut: String,
    pub detectors: Detectors,
    pub customer_patterns: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            shortcut: "CommandOrControl+Shift+S".into(),
            detectors: Detectors::default(),
            customer_patterns: vec![],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Detectors {
    pub aws: bool,
    pub jwt: bool,
    pub email: bool,
    pub ip: bool,
    pub api_key: bool,
    pub database_url: bool,
    pub private_key: bool,
    pub customer_id: bool,
}

impl Default for Detectors {
    fn default() -> Self {
        Self {
            aws: true,
            jwt: true,
            email: true,
            ip: true,
            api_key: true,
            database_url: true,
            private_key: true,
            customer_id: true,
        }
    }
}
