use std::time::{Duration};
use std::fmt::{Formatter, Debug};
use colored::Colorize;

#[derive(Copy, Clone)]
pub enum ComputationResult { Ok(Duration), Timeout(Duration), Error }

impl ComputationResult {
    pub fn is_err(&self) -> bool {
        match self {
            ComputationResult::Error => true,
            _ => false
        }
    }

    pub fn is_timeout(&self) -> bool {
        match self {
            ComputationResult::Timeout(_) => true,
            _ => false
        }
    }

    pub fn is_ok(&self) -> bool {
        match self {
            ComputationResult::Ok(_) => true,
            _ => false
        }
    }
}

impl Debug for ComputationResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ComputationResult::Error => f.write_fmt(format_args!("{}", "Error".red())),
            ComputationResult::Ok(time) => f.write_fmt(format_args!("{}      Time:  {:.2}s", "Done".green(), time.as_millis() as f64 / 1000.0)),
            ComputationResult::Timeout(limit) => f.write_fmt(format_args!("{}   Limit: {}", "Timeout".yellow(), humantime::Duration::from(*limit)))
        }
    }
}

impl ToString for ComputationResult {
    fn to_string(&self) -> String {
        match self {
            ComputationResult::Ok(time) => format!("{:.2}", time.as_millis() as f64 / 1000.0).to_owned(),
            ComputationResult::Timeout(limit) => format!("T - {:.2}", limit.as_millis() as f64 / 1000.0).to_owned(),
            ComputationResult::Error => "Error".to_owned(),
        }
    }
}


