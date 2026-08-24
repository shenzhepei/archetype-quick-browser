use std::io::{BufReader, Cursor};

use cookie::SameSite;
use cookie_store::{CookieStore, RawCookie, StoreAction};
use thiserror::Error;
use url::Url;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestMethod {
    Get,
    Post,
}

impl RequestMethod {
    const fn is_safe(self) -> bool {
        matches!(self, Self::Get)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CookieRequest<'a> {
    pub url: &'a Url,
    pub top_level_url: &'a Url,
    pub method: RequestMethod,
    pub is_top_level_navigation: bool,
}

#[derive(Debug, Error)]
pub enum CookieJarError {
    #[error("invalid Set-Cookie header: {0}")]
    InvalidHeader(String),
    #[error("could not serialize cookies: {0}")]
    Serialize(String),
    #[error("could not deserialize cookies: {0}")]
    Deserialize(String),
}

#[derive(Clone, Debug, Default)]
pub struct CookieJar {
    store: CookieStore,
}

impl CookieJar {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores one HTTP `Set-Cookie` response header.
    ///
    /// # Errors
    /// Returns [`CookieJarError`] when the header cannot be parsed or violates the secure
    /// `SameSite=None` requirement.
    pub fn store_response_header(
        &mut self,
        response_url: &Url,
        header: &str,
    ) -> Result<StoreAction, CookieJarError> {
        let cookie = RawCookie::parse(header.to_owned())
            .map_err(|error| CookieJarError::InvalidHeader(error.to_string()))?;
        if cookie.same_site() == Some(SameSite::None) && cookie.secure() != Some(true) {
            return Err(CookieJarError::InvalidHeader(
                "SameSite=None requires Secure".to_owned(),
            ));
        }
        self.store
            .insert_raw(&cookie, response_url)
            .map_err(|error| CookieJarError::InvalidHeader(error.to_string()))
    }

    #[must_use]
    pub fn request_header(&self, request: CookieRequest<'_>) -> Option<String> {
        let same_site = is_same_site(request.url, request.top_level_url);
        let values: Vec<_> = self
            .store
            .matches(request.url)
            .into_iter()
            .filter(|cookie| match cookie.same_site().unwrap_or(SameSite::Lax) {
                SameSite::Strict => same_site,
                SameSite::Lax => {
                    same_site || (request.is_top_level_navigation && request.method.is_safe())
                }
                SameSite::None => true,
            })
            .map(|cookie| format!("{}={}", cookie.name(), cookie.value()))
            .collect();
        (!values.is_empty()).then(|| values.join("; "))
    }

    /// Serializes unexpired persistent cookies. Session cookies are intentionally omitted.
    ///
    /// # Errors
    /// Returns [`CookieJarError`] when serialization fails.
    pub fn persistent_json(&self) -> Result<Vec<u8>, CookieJarError> {
        let mut output = Vec::new();
        cookie_store::serde::json::save(&self.store, &mut output)
            .map_err(|error| CookieJarError::Serialize(error.to_string()))?;
        Ok(output)
    }

    /// Restores persistent cookies from a previously serialized profile value.
    ///
    /// # Errors
    /// Returns [`CookieJarError`] when the serialized value is invalid.
    pub fn from_persistent_json(value: &[u8]) -> Result<Self, CookieJarError> {
        let reader = BufReader::new(Cursor::new(value));
        let store = cookie_store::serde::json::load(reader)
            .map_err(|error| CookieJarError::Deserialize(error.to_string()))?;
        Ok(Self { store })
    }
}

fn is_same_site(request_url: &Url, top_level_url: &Url) -> bool {
    request_url.scheme() == top_level_url.scheme()
        && site_domain(request_url) == site_domain(top_level_url)
}

fn site_domain(url: &Url) -> Option<Vec<u8>> {
    let host = url.host_str()?.to_ascii_lowercase();
    Some(match psl::domain(host.as_bytes()) {
        Some(domain) => domain.as_bytes().to_vec(),
        None => host.into_bytes(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(value: &str) -> Url {
        Url::parse(value).unwrap()
    }

    fn request<'a>(
        request_url: &'a Url,
        top_level_url: &'a Url,
        method: RequestMethod,
    ) -> CookieRequest<'a> {
        CookieRequest {
            url: request_url,
            top_level_url,
            method,
            is_top_level_navigation: true,
        }
    }

    #[test]
    fn enforces_domain_path_secure_and_http_only_rules() {
        let origin = url("https://accounts.example.com/login");
        let mut jar = CookieJar::new();
        jar.store_response_header(
            &origin,
            "session=secret; Domain=example.com; Path=/account; Secure; HttpOnly",
        )
        .unwrap();

        let matching = url("https://shop.example.com/account/orders");
        assert_eq!(
            jar.request_header(request(&matching, &matching, RequestMethod::Get)),
            Some("session=secret".to_owned())
        );
        let insecure = url("http://shop.example.com/account/orders");
        assert_eq!(
            jar.request_header(request(&insecure, &insecure, RequestMethod::Get)),
            None
        );
        let wrong_path = url("https://shop.example.com/settings");
        assert_eq!(
            jar.request_header(request(&wrong_path, &wrong_path, RequestMethod::Get)),
            None
        );
    }

    #[test]
    fn applies_schemeful_same_site_to_get_and_post_navigation() {
        let origin = url("https://login.example.com/");
        let mut jar = CookieJar::new();
        jar.store_response_header(
            &origin,
            "strict=1; Domain=example.com; SameSite=Strict; Secure",
        )
        .unwrap();
        jar.store_response_header(&origin, "lax=1; Domain=example.com; SameSite=Lax; Secure")
            .unwrap();
        jar.store_response_header(&origin, "none=1; Domain=example.com; SameSite=None; Secure")
            .unwrap();

        let destination = url("https://app.example.com/dashboard");
        let same_site = jar
            .request_header(request(&destination, &origin, RequestMethod::Post))
            .unwrap();
        assert!(same_site.contains("strict=1"));
        assert!(same_site.contains("lax=1"));
        assert!(same_site.contains("none=1"));

        let cross_site = url("https://different.test/");
        let cross_site_get = jar
            .request_header(request(&destination, &cross_site, RequestMethod::Get))
            .unwrap();
        assert!(!cross_site_get.contains("strict=1"));
        assert!(cross_site_get.contains("lax=1"));
        assert!(cross_site_get.contains("none=1"));
        assert_eq!(
            jar.request_header(request(&destination, &cross_site, RequestMethod::Post)),
            Some("none=1".to_owned())
        );
    }

    #[test]
    fn rejects_insecure_none_and_drops_expired_cookies() {
        let origin = url("https://example.com/");
        let mut jar = CookieJar::new();
        assert!(
            jar.store_response_header(&origin, "invalid=1; SameSite=None")
                .is_err()
        );
        assert!(
            jar.store_response_header(&origin, "gone=1; Max-Age=0")
                .is_err()
        );
        assert_eq!(
            jar.request_header(request(&origin, &origin, RequestMethod::Get)),
            None
        );
    }

    #[test]
    fn persistence_excludes_session_cookies() {
        let origin = url("https://example.com/");
        let mut jar = CookieJar::new();
        jar.store_response_header(&origin, "session=temporary; Secure")
            .unwrap();
        jar.store_response_header(
            &origin,
            "persistent=kept; Secure; Expires=Tue, 03 Aug 2100 00:38:37 GMT",
        )
        .unwrap();

        let serialized = jar.persistent_json().unwrap();
        let restored = CookieJar::from_persistent_json(&serialized).unwrap();
        assert_eq!(
            restored.request_header(request(&origin, &origin, RequestMethod::Get)),
            Some("persistent=kept".to_owned())
        );
        assert!(!String::from_utf8(serialized).unwrap().contains("temporary"));
    }
}
