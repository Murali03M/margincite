use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Meta {
    pub total: i64,
    pub page: i64,
    pub limit: i64,
}

#[derive(Debug, Serialize)]
pub struct Envelope<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub meta: Option<Meta>,
}

impl<T: Serialize> Envelope<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            meta: None,
        }
    }

    pub fn page(data: T, meta: Meta) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            meta: Some(meta),
        }
    }

    pub fn err(message: impl Into<String>) -> Envelope<()> {
        Envelope {
            success: false,
            data: None,
            error: Some(message.into()),
            meta: None,
        }
    }
}
