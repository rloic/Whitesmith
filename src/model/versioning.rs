use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Versioning {
    pub url: String,
    pub commit: Option<String>,
    #[serde(default)]
    pub sub_modules: bool,
}