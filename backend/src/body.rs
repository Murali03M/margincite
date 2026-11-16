//! Reads an outside service's response body with a size limit, so a service
//! that sends far too much cannot use up this server's memory.

/// The most any outside service may send in one response.
pub const MAX_BODY_BYTES: usize = 20 * 1024 * 1024;

#[derive(Debug)]
pub enum BodyError {
    /// The body is larger than the limit.
    TooLarge,
    /// The connection failed while the body was coming in.
    Read(reqwest::Error),
}

impl BodyError {
    pub fn is_timeout(&self) -> bool {
        matches!(self, BodyError::Read(err) if err.is_timeout())
    }
}

/// The whole body, or an error as soon as it is known to be over `limit`:
/// from `content-length` when it is sent, otherwise as the chunks add up.
pub async fn read_capped(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, BodyError> {
    let announced = response.content_length();
    if announced.is_some_and(|length| length > limit as u64) {
        return Err(BodyError::TooLarge);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(BodyError::Read)? {
        if body.len() + chunk.len() > limit {
            return Err(BodyError::TooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
