use std::{fs, io::Read, time::Duration};

use arch_session::cookies::{CookieJar, CookieRequest, RequestMethod};
use reqwest::blocking::Client;
use thiserror::Error;
use url::Url;

pub const DOCUMENT_LIMIT: usize = 5 * 1024 * 1024;
pub const FORM_BODY_LIMIT: usize = 1024 * 1024;
const MAXIMUM_REDIRECTS: usize = 10;

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
    #[error("network redirect is invalid or missing a location")]
    InvalidRedirect,
    #[error("network redirect limit exceeded")]
    TooManyRedirects,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadErrorKind {
    UnsupportedScheme,
    ResourceTooLarge,
    File,
    Timeout,
    Tls,
    Connection,
    HttpStatus,
    Network,
    InvalidFileUrl,
    InvalidRedirect,
    TooManyRedirects,
}

impl LoadError {
    #[must_use]
    pub fn kind(&self) -> LoadErrorKind {
        match self {
            Self::UnsupportedScheme(_) => LoadErrorKind::UnsupportedScheme,
            Self::ResourceTooLarge { .. } => LoadErrorKind::ResourceTooLarge,
            Self::File(_) => LoadErrorKind::File,
            Self::Network(error) if error.is_timeout() => LoadErrorKind::Timeout,
            Self::Network(error) if error_chain_contains_tls(error) => LoadErrorKind::Tls,
            Self::Network(error) if error.is_connect() => LoadErrorKind::Connection,
            Self::Network(error) if error.status().is_some() => LoadErrorKind::HttpStatus,
            Self::Network(_) => LoadErrorKind::Network,
            Self::InvalidFileUrl => LoadErrorKind::InvalidFileUrl,
            Self::InvalidRedirect => LoadErrorKind::InvalidRedirect,
            Self::TooManyRedirects => LoadErrorKind::TooManyRedirects,
        }
    }
}

fn error_chain_contains_tls(error: &(dyn std::error::Error + 'static)) -> bool {
    let mut current = Some(error);
    while let Some(cause) = current {
        if cause.downcast_ref::<rustls::Error>().is_some() {
            return true;
        }
        current = cause.source();
    }
    false
}

pub struct Loader {
    client: Client,
}

#[derive(Clone, Copy)]
struct HttpNavigation<'a> {
    url: &'a Url,
    limit: usize,
    top_level_url: &'a Url,
    is_top_level_navigation: bool,
    method: RequestMethod,
    body: Option<&'a str>,
}

impl Loader {
    /// Builds the constrained V3 HTTP client.
    ///
    /// # Errors
    /// Returns [`LoadError`] when the TLS-enabled client cannot be constructed.
    pub fn new() -> Result<Self, LoadError> {
        Self::with_timeouts(Duration::from_secs(10), Duration::from_secs(30))
    }

    fn with_timeouts(connect_timeout: Duration, timeout: Duration) -> Result<Self, LoadError> {
        let client = Client::builder()
            .connect_timeout(connect_timeout)
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
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

    /// Loads a resource while applying Browser-owned Cookie policy to every redirect hop.
    ///
    /// # Errors
    /// Returns [`LoadError`] for unsupported schemes, invalid redirects, I/O failures, network
    /// failures, HTTP error status codes, or resources that exceed `limit`.
    pub fn load_with_cookies(
        &self,
        url: &Url,
        limit: usize,
        cookie_jar: &mut CookieJar,
        top_level_url: &Url,
        is_top_level_navigation: bool,
    ) -> Result<ResponseBytes, LoadError> {
        match url.scheme() {
            "file" => Self::load_file(url, limit),
            "http" | "https" => self.load_http_redirects(
                HttpNavigation {
                    url,
                    limit,
                    top_level_url,
                    is_top_level_navigation,
                    method: RequestMethod::Get,
                    body: None,
                },
                Some(cookie_jar),
            ),
            scheme => Err(LoadError::UnsupportedScheme(scheme.to_owned())),
        }
    }

    /// Submits a user-initiated form-urlencoded POST with Cookie policy on every redirect hop.
    ///
    /// # Errors
    /// Returns [`LoadError`] when the body exceeds 1 MiB or the request, redirect, response, or
    /// response size violates the loader limits.
    pub fn submit_with_cookies(
        &self,
        url: &Url,
        limit: usize,
        cookie_jar: &mut CookieJar,
        top_level_url: &Url,
        body: &str,
    ) -> Result<ResponseBytes, LoadError> {
        ensure_limit(body.as_bytes(), FORM_BODY_LIMIT)?;
        match url.scheme() {
            "http" | "https" => self.load_http_redirects(
                HttpNavigation {
                    url,
                    limit,
                    top_level_url,
                    is_top_level_navigation: true,
                    method: RequestMethod::Post,
                    body: Some(body),
                },
                Some(cookie_jar),
            ),
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
        self.load_http_redirects(
            HttpNavigation {
                url,
                limit,
                top_level_url: url,
                is_top_level_navigation: true,
                method: RequestMethod::Get,
                body: None,
            },
            None,
        )
    }

    fn load_http_redirects(
        &self,
        navigation: HttpNavigation<'_>,
        mut cookie_jar: Option<&mut CookieJar>,
    ) -> Result<ResponseBytes, LoadError> {
        let mut current_url = navigation.url.clone();
        let mut method = navigation.method;
        for redirect_count in 0..=MAXIMUM_REDIRECTS {
            let cookie_header = cookie_jar.as_deref().and_then(|jar| {
                jar.request_header(CookieRequest {
                    url: &current_url,
                    top_level_url: navigation.top_level_url,
                    method,
                    is_top_level_navigation: navigation.is_top_level_navigation,
                })
            });
            let mut request = match method {
                RequestMethod::Get => self.client.get(current_url.clone()),
                RequestMethod::Post => self
                    .client
                    .post(current_url.clone())
                    .header(
                        reqwest::header::CONTENT_TYPE,
                        "application/x-www-form-urlencoded",
                    )
                    .body(navigation.body.unwrap_or_default().to_owned()),
            };
            if let Some(cookie_header) = cookie_header {
                request = request.header(reqwest::header::COOKIE, cookie_header);
            }
            let response = request.send()?;
            if let Some(jar) = cookie_jar.as_deref_mut() {
                for header in response.headers().get_all(reqwest::header::SET_COOKIE) {
                    if let Ok(header) = header.to_str() {
                        let _ = jar.store_response_header(&current_url, header);
                    }
                }
            }
            if response.status().is_redirection() {
                if redirect_count == MAXIMUM_REDIRECTS {
                    return Err(LoadError::TooManyRedirects);
                }
                let location = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or(LoadError::InvalidRedirect)?;
                if response.status() == reqwest::StatusCode::SEE_OTHER
                    || (matches!(
                        response.status(),
                        reqwest::StatusCode::MOVED_PERMANENTLY | reqwest::StatusCode::FOUND
                    ) && method == RequestMethod::Post)
                {
                    method = RequestMethod::Get;
                }
                current_url = current_url
                    .join(location)
                    .map_err(|_| LoadError::InvalidRedirect)?;
                continue;
            }
            let response = response.error_for_status()?;
            return Self::read_http_response(response, navigation.limit);
        }
        Err(LoadError::TooManyRedirects)
    }

    fn read_http_response(
        response: reqwest::blocking::Response,
        limit: usize,
    ) -> Result<ResponseBytes, LoadError> {
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
    use std::{io::Write as _, net::TcpListener, thread};

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
        let error = Loader::load_file(&url, 1);
        assert!(matches!(
            error,
            Err(LoadError::ResourceTooLarge { limit: 1 })
        ));
    }

    #[test]
    fn recognizes_tls_errors_in_an_error_chain() {
        let error = rustls::Error::General("certificate validation failed".to_owned());
        assert!(error_chain_contains_tls(&error));
    }

    #[test]
    fn loads_http_and_follows_a_redirect() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            for expected_path in ["/start", "/final"] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0u8; 1024];
                let length = stream.read(&mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..length]);
                assert!(request.starts_with(&format!("GET {expected_path} ")));
                if expected_path == "/start" {
                    write!(
                        stream,
                        "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    )
                    .unwrap();
                } else {
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 12\r\nConnection: close\r\n\r\n<p>ready</p>"
                    )
                    .unwrap();
                }
            }
        });
        let response = Loader::default()
            .load(&Url::parse(&format!("http://{address}/start")).unwrap())
            .unwrap();
        server.join().unwrap();
        assert_eq!(response.final_url.path(), "/final");
        assert_eq!(response.body, b"<p>ready</p>");
        assert_eq!(response.content_type.as_deref(), Some("text/html"));
    }

    #[test]
    fn applies_cookies_between_redirect_hops() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            for expected_path in ["/start", "/final"] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 2048];
                let length = stream.read(&mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..length]);
                assert!(request.starts_with(&format!("GET {expected_path} ")));
                if expected_path == "/start" {
                    assert!(!request.to_ascii_lowercase().contains("cookie:"));
                    write!(
                        stream,
                        "HTTP/1.1 302 Found\r\nLocation: /final\r\nSet-Cookie: hop=ready; Path=/; HttpOnly\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    )
                    .unwrap();
                } else {
                    assert!(request.to_ascii_lowercase().contains("cookie: hop=ready"));
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 12\r\nConnection: close\r\n\r\n<p>ready</p>"
                    )
                    .unwrap();
                }
            }
        });
        let url = Url::parse(&format!("http://{address}/start")).unwrap();
        let mut jar = CookieJar::new();
        let response = Loader::default()
            .load_with_cookies(&url, DOCUMENT_LIMIT, &mut jar, &url, true)
            .unwrap();
        server.join().unwrap();
        assert_eq!(response.final_url.path(), "/final");
        assert_eq!(response.body, b"<p>ready</p>");
    }

    #[test]
    fn post_redirects_switch_or_preserve_methods_by_status() {
        for (status, expected_method) in [(303, "GET"), (307, "POST")] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = thread::spawn(move || {
                for index in 0..2 {
                    let (mut stream, _) = listener.accept().unwrap();
                    let mut request = [0_u8; 4096];
                    let length = stream.read(&mut request).unwrap();
                    let request = String::from_utf8_lossy(&request[..length]);
                    if index == 0 {
                        assert!(request.starts_with("POST /submit "));
                        assert!(request.contains("query=rust+browser"));
                        write!(
                            stream,
                            "HTTP/1.1 {status} Redirect\r\nLocation: /done\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        )
                        .unwrap();
                    } else {
                        assert!(request.starts_with(&format!("{expected_method} /done ")));
                        if expected_method == "POST" {
                            assert!(request.contains("query=rust+browser"));
                        }
                        write!(
                            stream,
                            "HTTP/1.1 200 OK\r\nContent-Length: 19\r\nConnection: close\r\n\r\n<title>Done</title>"
                        )
                        .unwrap();
                    }
                }
            });
            let url = Url::parse(&format!("http://{address}/submit")).unwrap();
            let mut jar = CookieJar::new();
            let response = Loader::default()
                .submit_with_cookies(&url, DOCUMENT_LIMIT, &mut jar, &url, "query=rust+browser")
                .unwrap();
            server.join().unwrap();
            assert_eq!(response.final_url.path(), "/done");
        }
    }

    #[test]
    fn classifies_http_status_connection_and_timeout_failures() {
        let status_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let status_address = status_listener.local_addr().unwrap();
        let status_server = thread::spawn(move || {
            let (mut stream, _) = status_listener.accept().unwrap();
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request).unwrap();
            write!(
                stream,
                "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
        });
        let status_error = Loader::default()
            .load(&Url::parse(&format!("http://{status_address}/")).unwrap())
            .unwrap_err();
        status_server.join().unwrap();
        assert_eq!(status_error.kind(), LoadErrorKind::HttpStatus);

        let refused_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let refused_address = refused_listener.local_addr().unwrap();
        drop(refused_listener);
        let connection_error = Loader::default()
            .load(&Url::parse(&format!("http://{refused_address}/")).unwrap())
            .unwrap_err();
        assert_eq!(connection_error.kind(), LoadErrorKind::Connection);

        let timeout_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let timeout_address = timeout_listener.local_addr().unwrap();
        let timeout_server = thread::spawn(move || {
            let (_stream, _) = timeout_listener.accept().unwrap();
            thread::sleep(Duration::from_millis(200));
        });
        let timeout_error =
            Loader::with_timeouts(Duration::from_millis(25), Duration::from_millis(25))
                .unwrap()
                .load(&Url::parse(&format!("http://{timeout_address}/")).unwrap())
                .unwrap_err();
        timeout_server.join().unwrap();
        assert_eq!(timeout_error.kind(), LoadErrorKind::Timeout);
    }
}
