use std::time::Duration;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Experiment {
    pub name: String,
    #[serde(default)]
    pub parameters: Vec<String>,
    #[serde(default)]
    pub difficulty: u32,
    #[serde(default, with="humantime_serde")]
    pub timeout: Option<Duration>
}