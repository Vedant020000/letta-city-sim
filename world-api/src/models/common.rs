use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum NotificationMode {
    Instant,
    Deferred,
}

#[derive(Debug, Serialize, Clone)]
pub struct NotificationPayload {
    pub message: String,
    pub mode: NotificationMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta_seconds: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification: Option<NotificationPayload>,
}

impl<T: Serialize> From<T> for ApiResponse<T> {
    fn from(data: T) -> Self {
        Self {
            data,
            notification: None,
        }
    }
}

impl<T: Serialize> ApiResponse<T> {
    pub fn with_notification(mut self, notification: NotificationPayload) -> Self {
        self.notification = Some(notification);
        self
    }
}
