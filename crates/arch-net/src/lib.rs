use std::{fs, io::Read, time::Duration};

use reqwest::blocking::Client;
use thiserror::Error;
use url::Url;

pub const DOCUMENT_LIMIT: usize = 5 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct ResponseBytes {
    pub final_url: Url,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("unsupported URL scheme: {0}")]
    UnsupportedScheme(String),
    #[error("resource exceeds {limit} bytes")]
    ResourceTooLarge { limit: usize },
    #[error("file load failed: {0}")]
    File(#[from] std::io::Error),
    #[error("network load failed: {0}")]
    Network(#[from] reqwest::Error),
    #[error("file URL cannot be converted to a local path")]
    InvalidFileUrl,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadErrorKind {
    UnsupportedScheme,
    ResourceTooLarge,
    File,
    Timeout,
    Connection,
    HttpStatus,
    Network,
    InvalidFileUrl,
}

impl LoadError {
    #[must_use]
    pub fn kind(&self) -> LoadErrorKind {
        match self {
            Self::UnsupportedScheme(_) => LoadErrorKind::UnsupportedScheme,
            Self::ResourceTooLarge { .. } => LoadErrorKind::ResourceTooLarge,
            Self::File(_) => LoadErrorKind::File,
            Self::Network(error) if error.is_timeout() => LoadErrorKind::Timeout,
            Self::Network(error) if error.is_connect() => LoadErrorKind::Connection,
            Self::Network(error) if error.status().is_some() => LoadErrorKind::HttpStatus,
            Self::Network(_) => LoadErrorKind::Network,
            Self::InvalidFileUrl => LoadErrorKind::InvalidFileUrl,
        }
    }
}

pub struct Loader {
    client: Client,
}

impl Loader {
    /// Builds the constrained V3 HTTP client.
    ///
    /// # Errors
    /// Returns [`LoadError`] when the TLS-enabled client cannot be constructed.
    pub fn new() -> Result<Self, LoadError> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()?;
        Ok(Self { client })
    }

    /// Loads a `file`, `http`, or `https` document within the V3 size and timeout limits.
    ///
    /// # Errors
    /// Returns [`LoadError`] for unsupported schemes, I/O failures, network failures, HTTP error
    /// status codes, or resources that exceed the configured limit.
    pub fn load(&self, url: &Url) -> Result<ResponseBytes, LoadError> {
        self.load_with_limit(url, DOCUMENT_LIMIT)
    }

    /// Loads a supported resource while enforcing a caller-provided byte limit.
    ///
    /// # Errors
    /// Returns [`LoadError`] for unsupported schemes, I/O failures, network failures, HTTP error
    /// status codes, or resources that exceed `limit`.
    pub fn load_with_limit(&self, url: &Url, limit: usize) -> Result<ResponseBytes, LoadError> {
        match url.scheme() {
            "file" => Self::load_file(url, limit),
            "http" | "https" => self.load_http(url, limit),
            scheme => Err(LoadError::UnsupportedScheme(scheme.to_owned())),
        }
    }

    fn load_file(url: &Url, limit: usize) -> Result<ResponseBytes, LoadError> {
        let path = url.to_file_path().map_err(|()| LoadError::InvalidFileUrl)?;
        let file = fs::File::open(path)?;
        let mut body = Vec::new();
        file.take(limit.saturating_add(1) as u64)
            .read_to_end(&mut body)?;
        ensure_limit(&body, limit)?;
        Ok(ResponseBytes {
            final_url: url.clone(),
            content_type: None,
            body,
        })
    }

    fn load_http(&self, url: &Url, limit: usize) -> Result<ResponseBytes, LoadError> {
        let response = self.client.get(url.clone()).send()?.error_for_status()?;
        if response
            .content_length()
            .is_some_and(|size| size > limit as u64)
        {
            return Err(LoadError::ResourceTooLarge { limit });
        }
        let final_url = response.url().clone();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let mut body = Vec::new();
        response
            .take(limit.saturating_add(1) as u64)
            .read_to_end(&mut body)?;
        ensure_limit(&body, limit)?;
        Ok(ResponseBytes {
            final_url,
            content_type,
            body,
        })
    }
}

impl Default for Loader {
    fn default() -> Self {
        Self::new().expect("TLS client construction should succeed")
    }
}

fn ensure_limit(body: &[u8], limit: usize) -> Result<(), LoadError> {
    if body.len() > limit {
        Err(LoadError::ResourceTooLarge { limit })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_scheme() {
        let error = Loader::default().load(&Url::parse("data:text/plain,hi").unwrap());
        let error = error.unwrap_err();
        assert_eq!(error.kind(), LoadErrorKind::UnsupportedScheme);
    }

    #[test]
    fn rejects_file_over_custom_limit() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
        let url = Url::from_file_path(path).unwrap();
        let error = Loader::default().load_with_limit(&url, 1);
        assert!(matches!(
            error,
            Err(LoadError::ResourceTooLarge { limit: 1 })
        ));
    }
}
