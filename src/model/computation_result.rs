use std::time::{Duration};
use std::fmt::{Formatter, Debug};
use colored::Colorize;

#[derive(Copy, Clone)]
pub enum ComputationResult { Ok(Duration), Timeout(Duration), Error(Duration) }

impl ComputationResult {
    pub fn is_err(&self) -> bool {
        match self {
            ComputationResult::Error(_) => true,
            _ => false
        }
    }

    pub fn is_timeout(&self) -> bool {
        match self {
            ComputationResult::Timeout(_) => true,
            _ => false
        }
    }
}

impl Debug for ComputationResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ComputationResult::Error(time) => f.write_fmt(format_args!("{}     Time:  {:.2}s ({})", "Error".red(), time.as_millis() as f64 / 1000.0, humantime::Duration::from(*time))),
            ComputationResult::Ok(time) => f.write_fmt(format_args!("{}      Time:  {:.2}s ({})", "Done".green(), time.as_millis() as f64 / 1000.0, humantime::Duration::from(*time))),
            ComputationResult::Timeout(limit) => f.write_fmt(format_args!("{}   Limit: {}", "Timeout".yellow(), humantime::Duration::from(*limit)))
        }
    }
}

impl ToString for ComputationResult {
    fn to_string(&self) -> String {
        match self {
            ComputationResult::Ok(_) => String::from("Ok"),
            ComputationResult::Timeout(_) => String::from("Timeout"),
            ComputationResult::Error(_) => String::from("Error"),
        }
    }
}


