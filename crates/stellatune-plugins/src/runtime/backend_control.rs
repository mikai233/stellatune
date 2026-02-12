use crossbeam_channel::Sender;

#[derive(Debug, Clone)]
pub struct BackendControlRequest {
    pub plugin_id: String,
    pub request_json: String,
    pub response_tx: Sender<BackendControlResponse>,
}

#[derive(Debug, Clone)]
pub struct BackendControlResponse {
    pub status_code: i32,
    pub response_json: String,
    pub error_message: Option<String>,
}

impl BackendControlResponse {
    pub fn ok(response_json: impl Into<String>) -> Self {
        Self {
            status_code: 0,
            response_json: response_json.into(),
            error_message: None,
        }
    }

    pub fn error(status_code: i32, message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            status_code,
            response_json: String::new(),
            error_message: if message.is_empty() {
                None
            } else {
                Some(message)
            },
        }
    }
}
