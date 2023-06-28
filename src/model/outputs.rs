use serde::{Serialize, Deserialize};
use std::fs::File;
use rev_lines::RevLines;
use std::io::BufReader;

#[derive(Debug, Serialize, Deserialize)]
pub struct Outputs {
    pub delimiter: String,
    pub columns: Vec<Option<String>>
}

impl Outputs {
    pub fn get_results(&self, log_file: File) -> Vec<String> {
        let mut rev_lines = RevLines::new(BufReader::new(log_file));
        let mut results = Vec::new();

        while let Some(line) = rev_lines.next() {
            let line = line.unwrap();
            if !is_blank_or_empty(&line) {
                let line = line.trim();
                let parts = line.split(&self.delimiter).collect::<Vec<_>>();
                for (i, col) in self.columns.iter().enumerate() {
                    if col.is_some() {
                        if i < parts.len() {
                            results.push(parts[i].to_owned());
                        } else {
                            results.push(String::from("-"));
                        }
                    }
                }
                break;
            }
        }
        results
    }
}

fn is_blank_or_empty(s: &str) -> bool {
    s.is_empty() || s.chars().all(|it| it.is_whitespace())
}