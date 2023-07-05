use std::collections::HashMap;
use std::str::FromStr;
use serde::{Deserialize, Serialize};

pub type Aliases = HashMap<String, Alias>;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Alias {
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String)
}

impl ToString for Alias {
    fn to_string(&self) -> String {
        let inner_type: &dyn ToString = match self {
            Alias::Boolean(b) => b,
            Alias::Integer(i) => i,
            Alias::Float(f) => f,
            Alias::String(s) => s,
        };
        inner_type.to_string()
    }
}

impl FromStr for Alias {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "true" => Ok(Alias::Boolean(true)),
            "false" => Ok(Alias::Boolean(false)),
            _ => {
                if let Ok(i) = s.parse() {
                    Ok(Alias::Integer(i))
                } else if let Ok(f) = s.parse() {
                    Ok(Alias::Float(f))
                } else {
                    Ok(Alias::String(s.to_string()))
                }
            }
        }
    }
}