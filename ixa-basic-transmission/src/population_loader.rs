use serde::Deserialize;

#[derive(Deserialize)]
pub struct SyntheticPopRecord {
    pub age: u8,
    pub household: u32,
}

pub fn load_synthetic_population(path: &str) -> Result<Vec<SyntheticPopRecord>, std::io::Error> {
    let data = std::fs::read_to_string(path)?;
    let people: Vec<SyntheticPopRecord> = serde_json::from_str(&data)?;
    Ok(people)
}
