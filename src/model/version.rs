use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Version(pub u8, pub u8, pub u8);

impl ToString for Version {
    fn to_string(&self) -> String {
        format!("{}.{}.{}", self.0, self.1, self.2)
    }
}