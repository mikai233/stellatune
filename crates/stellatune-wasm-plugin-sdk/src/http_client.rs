use serde::de::DeserializeOwned;

use crate::error::{SdkError, SdkResult};

pub trait HttpClient {
    fn fetch_json(&mut self, url: &str) -> SdkResult<String>;
}

pub trait HttpClientExt: HttpClient {
    fn fetch_json_typed<T: DeserializeOwned>(&mut self, url: &str) -> SdkResult<T> {
        let raw = self.fetch_json(url)?;
        serde_json::from_str::<T>(&raw)
            .map_err(|error| SdkError::invalid_arg(format!("invalid JSON response: {error}")))
    }
}

impl<T: HttpClient + ?Sized> HttpClientExt for T {}
