use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Versioning {
    pub url: String,
    #[serde(default)]
    pub commit: Option<String>,
    #[serde(default)]
    pub sub_modules: bool,
}