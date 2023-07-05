use serde::{Serialize, Serializer};


#[derive(Serialize, Debug)]
pub struct OutputLine {
    pub name: String,
    pub status: String,
    pub time: Seconds,
    pub iterations: Iterations,
}

#[derive(Serialize, Debug)]
pub struct Seconds(pub f64);

#[derive(Debug)]
pub struct Iterations(pub u32, pub u32);

impl Serialize for Iterations {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        serializer.serialize_str(&format!("{}/{}", self.0, self.1))
    }
}