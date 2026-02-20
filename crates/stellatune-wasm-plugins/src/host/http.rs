use crate::error::Result;

pub trait HttpClientHost: Send + Sync {
    fn fetch_json(&self, url: &str) -> Result<String>;
}
