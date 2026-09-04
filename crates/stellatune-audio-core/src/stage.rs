#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StageId(String);

impl StageId {
    pub fn new(value: impl Into<String>) -> Result<Self, &'static str> {
        let value = value.into().trim().to_owned();
        if value.is_empty() {
            return Err("stage id cannot be empty");
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
