use std::{collections::BTreeSet, str};

use arch_dom::NodeKind;
use arch_net::{LoadError, Loader};
use arch_session::cookies::CookieJar;
use archetype_protocol::{BrokeredResource, Codec, ResourceBytes, ResourceKind};
use archetype_types::{ArchetypeUrl, NavigationId, PageId};
use thiserror::Error;
use url::Url;

use crate::runtime_supervisor::StaticDocument;

const DOCUMENT_BYTE_LIMIT: usize = 4 * 1024 * 1024;
const RESOURCE_BYTE_LIMIT: usize = 4 * 1024 * 1024;
const TOTAL_RESOURCE_BYTE_LIMIT: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct BrokerRequest {
    pub page_id: PageId,
    pub navigation_id: NavigationId,
    pub url: Url,
    pub viewport_width_px: u32,
}

#[derive(Debug, Error)]
pub enum BrokerError {
    #[error("could not load document {url}: {source}")]
    DocumentLoad {
        url: Url,
        #[source]
        source: LoadError,
    },
    #[error("document {url} is not valid UTF-8")]
    DocumentEncoding { url: Url },
    #[error("document URL cannot cross the runtime protocol boundary: {0}")]
    InvalidUrl(String),
    #[error("viewport width must be between 1 and 65535 pixels")]
    InvalidViewport,
    #[error("brokered document exceeds the runtime frame limit: {0}")]
    Frame(String),
}

/// Loads one document and its same-origin subresources in the Browser process.
///
/// # Errors
/// Returns a typed error when the main document fails to load, exceeds its byte budget, is not
/// UTF-8, or cannot be represented by the runtime protocol.
pub fn load_static_document(
    loader: &Loader,
    request: &BrokerRequest,
) -> Result<StaticDocument, BrokerError> {
    let mut load = |url: &Url, limit: usize, _document: bool| loader.load_with_limit(url, limit);
    load_static_document_with(request, &mut load)
}

/// Loads a static document through the Browser-owned Cookie policy.
///
/// # Errors
/// Returns a typed broker error under the same limits as [`load_static_document`].
pub fn load_static_document_with_cookies(
    loader: &Loader,
    request: &BrokerRequest,
    cookie_jar: &mut CookieJar,
    top_level_url: &Url,
) -> Result<StaticDocument, BrokerError> {
    let mut current_top_level = top_level_url.clone();
    let mut load = |url: &Url, limit: usize, document: bool| {
        let response =
            loader.load_with_cookies(url, limit, cookie_jar, &current_top_level, document)?;
        if document {
            current_top_level.clone_from(&response.final_url);
        }
        Ok(response)
    };
    load_static_document_with(request, &mut load)
}

fn load_static_document_with(
    request: &BrokerRequest,
    load: &mut impl FnMut(&Url, usize, bool) -> Result<arch_net::ResponseBytes, LoadError>,
) -> Result<StaticDocument, BrokerError> {
    if request.viewport_width_px == 0 || request.viewport_width_px > u32::from(u16::MAX) {
        return Err(BrokerError::InvalidViewport);
    }
    let response = load(&request.url, DOCUMENT_BYTE_LIMIT, true).map_err(|source| {
        BrokerError::DocumentLoad {
            url: request.url.clone(),
            source,
        }
    })?;
    let html = str::from_utf8(&response.body)
        .map_err(|_| BrokerError::DocumentEncoding {
            url: response.final_url.clone(),
        })?
        .to_owned();
    let document = arch_html::parse(&html);
    let mut diagnostics = Vec::new();
    let mut resources = Vec::new();
    let mut loaded_urls = BTreeSet::new();
    let mut resource_bytes = 0_usize;

    for (kind, resource_url) in resource_urls(&document, &response.final_url, &mut diagnostics) {
        let key = format!("{kind:?}:{resource_url}");
        if !loaded_urls.insert(key) {
            continue;
        }
        if !same_origin(&response.final_url, &resource_url) {
            diagnostics.push(format!(
                "ignored cross-origin {}: {resource_url}",
                resource_name(kind)
            ));
            continue;
        }
        let remaining = TOTAL_RESOURCE_BYTE_LIMIT.saturating_sub(resource_bytes);
        if remaining == 0 {
            diagnostics.push(format!(
                "ignored {} beyond broker resource budget: {resource_url}",
                resource_name(kind)
            ));
            continue;
        }
        let limit = remaining.min(RESOURCE_BYTE_LIMIT);
        match load(&resource_url, limit, false) {
            Ok(resource) if !same_origin(&response.final_url, &resource.final_url) => {
                diagnostics.push(format!(
                    "ignored {} redirected across origins: {resource_url}",
                    resource_name(kind)
                ));
            }
            Ok(resource) => {
                resource_bytes = resource_bytes.saturating_add(resource.body.len());
                resources.push(BrokeredResource {
                    requested_url: protocol_url(&resource_url)?,
                    final_url: protocol_url(&resource.final_url)?,
                    kind,
                    body: ResourceBytes::new(resource.body),
                });
            }
            Err(error) => diagnostics.push(format!(
                "could not load {} {resource_url}: {error}",
                resource_name(kind)
            )),
        }
    }

    let document = StaticDocument {
        page_id: request.page_id.clone(),
        navigation_id: request.navigation_id,
        url: protocol_url(&response.final_url)?,
        html,
        viewport_width_px: request.viewport_width_px,
        resources,
        broker_diagnostics: diagnostics,
    };
    Codec::default()
        .encode(Vec::new(), &document.protocol_envelope(1))
        .map_err(|error| BrokerError::Frame(error.to_string()))?;
    Ok(document)
}

fn resource_urls(
    document: &arch_dom::Document,
    base: &Url,
    diagnostics: &mut Vec<String>,
) -> Vec<(ResourceKind, Url)> {
    let mut resources = Vec::new();
    for node in document.descendants(document.root()) {
        let NodeKind::Element(element) = &node.kind else {
            continue;
        };
        let candidate = if element.name == "link"
            && element
                .attribute("rel")
                .is_some_and(|value| value.split_whitespace().any(|item| item == "stylesheet"))
        {
            element
                .attribute("href")
                .map(|value| (ResourceKind::Stylesheet, value))
        } else if element.name == "img" {
            element
                .attribute("src")
                .map(|value| (ResourceKind::Image, value))
        } else {
            None
        };
        let Some((kind, reference)) = candidate else {
            continue;
        };
        match base.join(reference) {
            Ok(url) => resources.push((kind, url)),
            Err(_) => diagnostics.push(format!(
                "ignored {} with invalid URL: {reference}",
                resource_name(kind)
            )),
        }
    }
    resources
}

fn same_origin(document: &Url, resource: &Url) -> bool {
    match document.scheme() {
        "file" => file_resource_is_scoped(document, resource),
        "http" | "https" => {
            document.scheme() == resource.scheme()
                && document.host_str() == resource.host_str()
                && document.port_or_known_default() == resource.port_or_known_default()
        }
        _ => false,
    }
}

fn file_resource_is_scoped(document: &Url, resource: &Url) -> bool {
    if resource.scheme() != "file" {
        return false;
    }
    let Ok(document_path) = document.to_file_path() else {
        return false;
    };
    let Ok(resource_path) = resource.to_file_path() else {
        return false;
    };
    let Some(document_directory) = document_path.parent() else {
        return false;
    };
    let Ok(document_directory) = document_directory.canonicalize() else {
        return false;
    };
    let Ok(resource_path) = resource_path.canonicalize() else {
        return false;
    };
    resource_path.starts_with(document_directory)
}

fn protocol_url(url: &Url) -> Result<ArchetypeUrl, BrokerError> {
    url.as_str()
        .parse()
        .map_err(|_| BrokerError::InvalidUrl(url.as_str().to_owned()))
}

const fn resource_name(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Stylesheet => "stylesheet",
        ResourceKind::Image => "image",
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::*;

    #[test]
    fn brokers_fixture_stylesheets_and_images_as_bounded_bytes() {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/pages/05-image/index.html");
        let request = BrokerRequest {
            page_id: PageId::new(),
            navigation_id: NavigationId::zero().saturating_next(),
            url: Url::from_file_path(fixture).unwrap(),
            viewport_width_px: 1280,
        };

        let document = load_static_document(&Loader::default(), &request).unwrap();

        assert!(document.html.contains("<img"));
        assert!(
            document
                .resources
                .iter()
                .any(|resource| resource.kind == ResourceKind::Image)
        );
        assert!(
            document
                .resources
                .iter()
                .all(|resource| !resource.body.as_slice().is_empty())
        );
    }

    #[test]
    fn rejects_documents_over_the_broker_limit() {
        let directory = std::env::temp_dir().join(format!("archetype-broker-{}", PageId::new()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("large.html");
        fs::write(&path, vec![b'x'; DOCUMENT_BYTE_LIMIT + 1]).unwrap();
        let request = BrokerRequest {
            page_id: PageId::new(),
            navigation_id: NavigationId::zero(),
            url: Url::from_file_path(&path).unwrap(),
            viewport_width_px: 800,
        };

        let error = load_static_document(&Loader::default(), &request).unwrap_err();

        assert!(matches!(
            error,
            BrokerError::DocumentLoad {
                source: LoadError::ResourceTooLarge { .. },
                ..
            }
        ));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn local_documents_cannot_broker_files_outside_their_directory_tree() {
        let root = std::env::temp_dir().join(format!("archetype-scope-{}", PageId::new()));
        let document_directory = root.join("document");
        fs::create_dir_all(&document_directory).unwrap();
        fs::write(root.join("secret.png"), b"not available to the document").unwrap();
        let path = document_directory.join("index.html");
        fs::write(&path, "<img src='../secret.png' alt='secret'>").unwrap();
        let request = BrokerRequest {
            page_id: PageId::new(),
            navigation_id: NavigationId::zero(),
            url: Url::from_file_path(path).unwrap(),
            viewport_width_px: 800,
        };

        let document = load_static_document(&Loader::default(), &request).unwrap();

        assert!(document.resources.is_empty());
        assert!(
            document
                .broker_diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains("cross-origin image"))
        );
        fs::remove_dir_all(root).unwrap();
    }
}
