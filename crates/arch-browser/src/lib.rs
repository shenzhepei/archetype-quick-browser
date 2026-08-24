use std::{collections::HashMap, path::Path, str};

use anyhow::{Context, Result};
use arch_dom::NodeKind;
use arch_net::{LoadError, LoadErrorKind, Loader};
use arch_paint::DisplayList;
use arch_session::{BrowserCommand, BrowserEvent, Session};
use arch_store::{Bookmark, Page, Space, Store};
use archetype_types::{NavigationId, PageId};
use thiserror::Error;
use url::Url;

pub mod snapshot;

const IMAGE_LIMIT: usize = 20 * 1024 * 1024;
const PAGE_LIMIT: usize = 50 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct RenderedPage {
    pub final_url: Url,
    pub title: String,
    pub display_list: DisplayList,
    pub diagnostics: Vec<String>,
    pub image_resources: HashMap<String, Vec<u8>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderErrorKind {
    Load(LoadErrorKind),
    Parse,
    Render,
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("could not load {url}: {source}")]
    Load {
        url: Url,
        #[source]
        source: LoadError,
    },
    #[error("could not parse {url}: V3 only accepts UTF-8 HTML")]
    Parse {
        url: Url,
        #[source]
        source: str::Utf8Error,
    },
    #[error("could not render {url}: viewport width must be finite and greater than zero")]
    Render { url: Url },
}

impl RenderError {
    #[must_use]
    pub fn kind(&self) -> RenderErrorKind {
        match self {
            Self::Load { source, .. } => RenderErrorKind::Load(source.kind()),
            Self::Parse { .. } => RenderErrorKind::Parse,
            Self::Render { .. } => RenderErrorKind::Render,
        }
    }
}

pub struct BrowserCore {
    store: Store,
    session: Session,
    loader: Loader,
}

#[derive(Clone, Debug)]
pub struct PendingNavigation {
    page_id: PageId,
    navigation_id: NavigationId,
    url: Url,
}

impl PendingNavigation {
    #[must_use]
    pub fn url(&self) -> &Url {
        &self.url
    }
}

impl BrowserCore {
    /// Opens a persistent browser profile and restores its page identities.
    ///
    /// # Errors
    /// Returns an error when the profile database or network client cannot be initialized.
    pub fn open(profile: impl AsRef<Path>) -> Result<Self> {
        let store = Store::open(profile).context("could not open browser profile")?;
        Self::with_store(store)
    }

    /// Creates an in-memory browser profile for tests.
    ///
    /// # Errors
    /// Returns an error when the profile database or network client cannot be initialized.
    pub fn in_memory() -> Result<Self> {
        Self::with_store(Store::in_memory()?)
    }

    fn with_store(store: Store) -> Result<Self> {
        let mut core = Self {
            store,
            session: Session::default(),
            loader: Loader::new()?,
        };
        for page in core.store.pages()? {
            if let Ok(id) = page.id.parse::<PageId>() {
                if let Ok(url) = Url::parse(&page.url) {
                    core.session.restore_page(id, url);
                } else {
                    core.session.open_page(id);
                }
            }
        }
        Ok(core)
    }

    /// Creates and persists a Space.
    ///
    /// # Errors
    /// Returns an error when the database transaction fails.
    pub fn create_space(&mut self, name: &str) -> Result<Space> {
        Ok(self.store.create_space(name)?)
    }

    /// Renames a persisted Space.
    ///
    /// # Errors
    /// Returns an error when the database update fails.
    pub fn rename_space(&self, id: &str, name: &str) -> Result<bool> {
        Ok(self.store.rename_space(id, name)?)
    }

    /// Deletes a Space and its bookmarks without changing global tabs.
    ///
    /// # Errors
    /// Returns an error when the database transaction fails.
    pub fn delete_space(&mut self, id: &str) -> Result<bool> {
        Ok(self.store.delete_space(id)?)
    }

    /// Lists persisted Spaces in UI order.
    ///
    /// # Errors
    /// Returns an error when the database query fails.
    pub fn spaces(&self) -> Result<Vec<Space>> {
        Ok(self.store.spaces()?)
    }

    /// Creates a URL bookmark in a Space folder or root.
    ///
    /// # Errors
    /// Returns an error when the bookmark data or database transaction is invalid.
    pub fn create_bookmark(
        &mut self,
        space_id: &str,
        parent_id: Option<&str>,
        title: &str,
        url: &Url,
    ) -> Result<Bookmark> {
        Ok(self
            .store
            .create_bookmark(space_id, parent_id, title, url.as_str())?)
    }

    /// Creates a bookmark folder in a Space folder or root.
    ///
    /// # Errors
    /// Returns an error when the folder data or database transaction is invalid.
    pub fn create_bookmark_folder(
        &mut self,
        space_id: &str,
        parent_id: Option<&str>,
        title: &str,
    ) -> Result<Bookmark> {
        Ok(self
            .store
            .create_bookmark_folder(space_id, parent_id, title)?)
    }

    /// Lists direct bookmark children in a Space folder or root.
    ///
    /// # Errors
    /// Returns an error when the database query fails.
    pub fn bookmarks(&self, space_id: &str, parent_id: Option<&str>) -> Result<Vec<Bookmark>> {
        Ok(self.store.bookmarks(space_id, parent_id)?)
    }

    /// Renames a bookmark or folder.
    ///
    /// # Errors
    /// Returns an error when the title is empty or the database update fails.
    pub fn rename_bookmark(&self, id: &str, title: &str) -> Result<bool> {
        Ok(self.store.rename_bookmark(id, title)?)
    }

    /// Deletes a bookmark or folder subtree.
    ///
    /// # Errors
    /// Returns an error when the database transaction fails.
    pub fn delete_bookmark(&mut self, id: &str) -> Result<bool> {
        Ok(self.store.delete_bookmark(id)?)
    }

    /// Creates and persists a global tab, assigning a stable V7 UUID.
    ///
    /// # Errors
    /// Returns an error when the database transaction fails.
    pub fn create_page(&mut self, url: &Url) -> Result<Page> {
        let page = self.store.create_page(url.as_str())?;
        let id = page
            .id
            .parse::<PageId>()
            .context("store generated invalid page UUID")?;
        self.session.open_page(id);
        Ok(page)
    }

    /// Closes and removes a page from the profile.
    ///
    /// # Errors
    /// Returns an error when the page ID is invalid or the database transaction fails.
    pub fn close_page(&mut self, page: &Page) -> Result<bool> {
        let id = page.id.parse::<PageId>().context("page has invalid UUID")?;
        self.session.close_page(&id);
        Ok(self.store.delete_page(&page.id)?)
    }

    /// Navigates a page and commits only the newest navigation result.
    ///
    /// # Errors
    /// Returns an error when the page ID or URL is invalid, loading/rendering fails, or the
    /// completed navigation cannot be persisted.
    pub fn navigate(
        &mut self,
        page: &Page,
        url: &Url,
        viewport_width: f32,
    ) -> Result<RenderedPage> {
        let pending = self.start_navigation(page, url)?;
        self.execute_navigation(page, &pending, viewport_width)
    }

    /// Navigates to the previous in-memory history entry.
    ///
    /// # Errors
    /// Returns an error when there is no previous entry or loading/rendering fails.
    pub fn back(&mut self, page: &Page, viewport_width: f32) -> Result<RenderedPage> {
        let pending = self.start_back(page)?;
        self.execute_navigation(page, &pending, viewport_width)
    }

    /// Navigates to the next in-memory history entry.
    ///
    /// # Errors
    /// Returns an error when there is no next entry or loading/rendering fails.
    pub fn forward(&mut self, page: &Page, viewport_width: f32) -> Result<RenderedPage> {
        let pending = self.start_forward(page)?;
        self.execute_navigation(page, &pending, viewport_width)
    }

    /// Reloads the current in-memory history entry.
    ///
    /// # Errors
    /// Returns an error when there is no current entry or loading/rendering fails.
    pub fn reload(&mut self, page: &Page, viewport_width: f32) -> Result<RenderedPage> {
        let pending = self.start_reload(page)?;
        self.execute_navigation(page, &pending, viewport_width)
    }

    /// Starts a navigation without blocking the caller on loading and rendering.
    ///
    /// # Errors
    /// Returns an error when the page ID is invalid.
    pub fn start_navigation(&mut self, page: &Page, url: &Url) -> Result<PendingNavigation> {
        let page_id = parsed_page_id(page).context("page has invalid UUID")?;
        self.start_command(BrowserCommand::Navigate {
            page_id,
            url: url.clone(),
        })
    }

    /// Starts a history-back navigation without blocking the caller.
    ///
    /// # Errors
    /// Returns an error when the page ID is invalid or no previous history entry exists.
    pub fn start_back(&mut self, page: &Page) -> Result<PendingNavigation> {
        let page_id = parsed_page_id(page).context("page has invalid UUID")?;
        self.start_command(BrowserCommand::Back { page_id })
    }

    /// Starts a history-forward navigation without blocking the caller.
    ///
    /// # Errors
    /// Returns an error when the page ID is invalid or no next history entry exists.
    pub fn start_forward(&mut self, page: &Page) -> Result<PendingNavigation> {
        let page_id = parsed_page_id(page).context("page has invalid UUID")?;
        self.start_command(BrowserCommand::Forward { page_id })
    }

    /// Starts a reload without blocking the caller.
    ///
    /// # Errors
    /// Returns an error when the page ID is invalid or has no current URL.
    pub fn start_reload(&mut self, page: &Page) -> Result<PendingNavigation> {
        let page_id = parsed_page_id(page).context("page has invalid UUID")?;
        self.start_command(BrowserCommand::Reload { page_id })
    }

    /// Commits a background navigation only when it is still the page's newest request.
    ///
    /// # Errors
    /// Returns an error when persisting the final URL and title fails.
    pub fn finish_navigation(
        &mut self,
        page: &Page,
        pending: &PendingNavigation,
        rendered: &RenderedPage,
    ) -> Result<bool> {
        if !self.session.commit_final_url(
            &pending.page_id,
            pending.navigation_id,
            rendered.final_url.clone(),
        ) {
            return Ok(false);
        }
        self.store.update_page_navigation(
            &page.id,
            rendered.final_url.as_str(),
            &rendered.title,
        )?;
        Ok(true)
    }

    /// Invalidates the active navigation for a page.
    ///
    /// # Errors
    /// Returns an error when the page ID is invalid.
    pub fn stop(&mut self, page: &Page) -> Result<bool> {
        let page_id = parsed_page_id(page).context("page has invalid UUID")?;
        Ok(matches!(
            self.session.handle(BrowserCommand::Stop { page_id }),
            BrowserEvent::LoadStageChanged {
                stage: arch_session::LoadStage::Cancelled,
                ..
            }
        ))
    }

    #[must_use]
    pub fn accepts_navigation(&self, pending: &PendingNavigation) -> bool {
        self.session
            .accepts(&pending.page_id, pending.navigation_id)
    }

    /// Reports whether the page has an older in-memory history entry.
    #[must_use]
    pub fn can_go_back(&self, page: &Page) -> bool {
        parsed_page_id(page).is_some_and(|page_id| self.session.can_go_back(&page_id))
    }

    /// Reports whether the page has a newer in-memory history entry.
    #[must_use]
    pub fn can_go_forward(&self, page: &Page) -> bool {
        parsed_page_id(page).is_some_and(|page_id| self.session.can_go_forward(&page_id))
    }

    fn execute_navigation(
        &mut self,
        page: &Page,
        pending: &PendingNavigation,
        viewport_width: f32,
    ) -> Result<RenderedPage> {
        let rendered = render_url(&self.loader, &pending.url, viewport_width)?;
        if !self.finish_navigation(page, pending, &rendered)? {
            anyhow::bail!("navigation result became stale");
        }
        Ok(rendered)
    }

    fn start_command(&mut self, command: BrowserCommand) -> Result<PendingNavigation> {
        let event = self.session.handle(command);
        let BrowserEvent::NavigationStarted {
            page_id,
            navigation_id,
            url,
        } = event
        else {
            anyhow::bail!("navigation command was ignored");
        };
        Ok(PendingNavigation {
            page_id,
            navigation_id,
            url,
        })
    }

    /// Lists persisted global tabs.
    ///
    /// # Errors
    /// Returns an error when the database query fails.
    pub fn pages(&self) -> Result<Vec<Page>> {
        Ok(self.store.pages()?)
    }

    /// Saves the selected Space and page IDs for restart restoration.
    ///
    /// # Errors
    /// Returns an error when either database write fails.
    pub fn save_selection(&self, space_id: Option<&str>, page_id: Option<&str>) -> Result<()> {
        self.store
            .set_state("selected_space_id", space_id.unwrap_or_default())?;
        self.store
            .set_state("selected_page_id", page_id.unwrap_or_default())?;
        Ok(())
    }

    /// Loads the selected Space and page IDs from the profile.
    ///
    /// # Errors
    /// Returns an error when either database query fails.
    pub fn selection(&self) -> Result<(Option<String>, Option<String>)> {
        let empty_to_none = |value: Option<String>| value.filter(|item| !item.is_empty());
        Ok((
            empty_to_none(self.store.state("selected_space_id")?),
            empty_to_none(self.store.state("selected_page_id")?),
        ))
    }
}

fn parsed_page_id(page: &Page) -> Option<PageId> {
    page.id.parse().ok()
}

/// Loads and renders a UTF-8 static document into a V3 display list.
///
/// # Errors
/// Returns a typed error when loading fails, the document is not valid UTF-8, or the viewport is
/// invalid.
pub fn render_url(loader: &Loader, url: &Url, viewport_width: f32) -> Result<RenderedPage> {
    if !viewport_width.is_finite() || viewport_width <= 0.0 {
        return Err(RenderError::Render { url: url.clone() }.into());
    }
    let response = loader.load(url).map_err(|source| RenderError::Load {
        url: url.clone(),
        source,
    })?;
    let source = str::from_utf8(&response.body).map_err(|source| RenderError::Parse {
        url: response.final_url.clone(),
        source,
    })?;
    let document = arch_html::parse(source);
    let mut css = inline_css(&document);
    let mut resource_diagnostics = Vec::new();
    let mut resource_total = response.body.len();
    for stylesheet_url in stylesheet_urls(&document, &response.final_url) {
        if !same_origin(&response.final_url, &stylesheet_url) {
            resource_diagnostics.push(format!("ignored cross-origin stylesheet: {stylesheet_url}"));
            continue;
        }
        match loader.load(&stylesheet_url) {
            Ok(stylesheet) if !same_origin(&response.final_url, &stylesheet.final_url) => {
                resource_diagnostics.push(format!(
                    "ignored stylesheet redirected across origins: {stylesheet_url}"
                ));
            }
            Ok(stylesheet) if resource_total.saturating_add(stylesheet.body.len()) > PAGE_LIMIT => {
                resource_diagnostics.push(format!(
                    "ignored stylesheet beyond page resource budget: {stylesheet_url}"
                ));
            }
            Ok(stylesheet) => match str::from_utf8(&stylesheet.body) {
                Ok(content) => {
                    resource_total = resource_total.saturating_add(stylesheet.body.len());
                    css.push_str(content);
                    css.push('\n');
                }
                Err(_) => resource_diagnostics
                    .push(format!("ignored non-UTF-8 stylesheet: {stylesheet_url}")),
            },
            Err(error) => resource_diagnostics.push(format!(
                "could not load stylesheet {stylesheet_url}: {error}"
            )),
        }
    }
    let (images, image_resources) = load_images(
        loader,
        &document,
        &response.final_url,
        &mut resource_total,
        &mut resource_diagnostics,
    );
    let mut rendered = render_document(
        &response.final_url,
        &document,
        &css,
        viewport_width,
        &images,
    );
    rendered.image_resources = image_resources;
    rendered.diagnostics.splice(0..0, resource_diagnostics);
    Ok(rendered)
}

#[must_use]
pub fn render_html(url: &Url, source: &str, viewport_width: f32) -> RenderedPage {
    let document = arch_html::parse(source);
    let css = inline_css(&document);
    render_document(url, &document, &css, viewport_width, &HashMap::new())
}

fn render_document(
    url: &Url,
    document: &arch_dom::Document,
    css: &str,
    viewport_width: f32,
    images: &HashMap<arch_dom::NodeId, arch_layout::ImageBox>,
) -> RenderedPage {
    let title = arch_html::title(document).unwrap_or_else(|| url.as_str().to_owned());
    let stylesheet = arch_css::parse(css);
    let styled = arch_style::style_document(document, &stylesheet);
    let links = link_targets(document, url);
    let layout = arch_layout::layout(document, &styled, viewport_width, images, &links);
    let display_list = arch_paint::paint(&layout);
    let mut diagnostics = stylesheet.diagnostics;
    diagnostics.extend(document_diagnostics(document));
    RenderedPage {
        final_url: url.clone(),
        title,
        display_list,
        diagnostics,
        image_resources: HashMap::new(),
    }
}

fn document_diagnostics(document: &arch_dom::Document) -> Vec<String> {
    let mut scripts = 0usize;
    let mut event_attributes = 0usize;
    for node in document.descendants(document.root()) {
        let NodeKind::Element(element) = &node.kind else {
            continue;
        };
        scripts += usize::from(element.name == "script");
        event_attributes += element
            .attributes
            .iter()
            .filter(|(name, _)| name.starts_with("on"))
            .count();
    }
    let mut diagnostics = Vec::new();
    if scripts > 0 {
        diagnostics.push(format!(
            "ignored {scripts} script element(s); JavaScript is disabled"
        ));
    }
    if event_attributes > 0 {
        diagnostics.push(format!(
            "ignored {event_attributes} inline event attribute(s); JavaScript is disabled"
        ));
    }
    diagnostics
}

fn link_targets(document: &arch_dom::Document, base: &Url) -> HashMap<arch_dom::NodeId, String> {
    document
        .descendants(document.root())
        .filter_map(|node| {
            matches!(&node.kind, NodeKind::Text(_))
                .then(|| nearest_link(document, node.id, base))
                .flatten()
                .map(|target| (node.id, target))
        })
        .collect()
}

fn nearest_link(
    document: &arch_dom::Document,
    node_id: arch_dom::NodeId,
    base: &Url,
) -> Option<String> {
    let mut ancestor = document.node(node_id)?.parent;
    while let Some(id) = ancestor {
        let node = document.node(id)?;
        if let NodeKind::Element(element) = &node.kind {
            if element.name == "a" {
                return base
                    .join(element.attribute("href")?)
                    .ok()
                    .map(|url| url.to_string());
            }
        }
        ancestor = node.parent;
    }
    None
}

fn load_images(
    loader: &Loader,
    document: &arch_dom::Document,
    base: &Url,
    resource_total: &mut usize,
    diagnostics: &mut Vec<String>,
) -> (
    HashMap<arch_dom::NodeId, arch_layout::ImageBox>,
    HashMap<String, Vec<u8>>,
) {
    let mut output = HashMap::new();
    let mut resources = HashMap::new();
    for node in document.descendants(document.root()) {
        let NodeKind::Element(element) = &node.kind else {
            continue;
        };
        if element.name != "img" {
            continue;
        }
        let alt = element.attribute("alt").unwrap_or_default().to_owned();
        let Some(source) = element.attribute("src").and_then(|src| base.join(src).ok()) else {
            diagnostics.push("ignored image with invalid source".to_owned());
            output.insert(
                node.id,
                arch_layout::ImageBox {
                    source: String::new(),
                    alt,
                    intrinsic_width: 160,
                    intrinsic_height: 32,
                    loaded: false,
                },
            );
            continue;
        };
        output.insert(
            node.id,
            arch_layout::ImageBox {
                source: source.to_string(),
                alt: alt.clone(),
                intrinsic_width: 160,
                intrinsic_height: 32,
                loaded: false,
            },
        );
        if !same_origin(base, &source) {
            diagnostics.push(format!("ignored cross-origin image: {source}"));
            continue;
        }
        match loader.load_with_limit(&source, IMAGE_LIMIT) {
            Ok(resource) if !same_origin(base, &resource.final_url) => {
                diagnostics.push(format!("ignored image redirected across origins: {source}"));
            }
            Ok(resource) => {
                let next_total = resource_total.saturating_add(resource.body.len());
                if next_total > PAGE_LIMIT {
                    diagnostics.push(format!(
                        "ignored image beyond page resource budget: {source}"
                    ));
                    continue;
                }
                match image::load_from_memory(&resource.body) {
                    Ok(decoded) => {
                        *resource_total = next_total;
                        resources.insert(source.to_string(), resource.body);
                        output.insert(
                            node.id,
                            arch_layout::ImageBox {
                                source: source.to_string(),
                                alt,
                                intrinsic_width: decoded.width(),
                                intrinsic_height: decoded.height(),
                                loaded: true,
                            },
                        );
                    }
                    Err(error) => {
                        diagnostics.push(format!("could not decode image {source}: {error}"));
                    }
                }
            }
            Err(error) => diagnostics.push(format!("could not load image {source}: {error}")),
        }
    }
    (output, resources)
}

fn stylesheet_urls(document: &arch_dom::Document, base: &Url) -> Vec<Url> {
    document
        .descendants(document.root())
        .filter_map(|node| {
            let NodeKind::Element(element) = &node.kind else {
                return None;
            };
            if element.name != "link"
                || !element
                    .attribute("rel")
                    .is_some_and(|rel| rel.split_whitespace().any(|item| item == "stylesheet"))
            {
                return None;
            }
            base.join(element.attribute("href")?).ok()
        })
        .collect()
}

fn same_origin(document: &Url, resource: &Url) -> bool {
    match document.scheme() {
        "file" => resource.scheme() == "file",
        "http" | "https" => {
            document.scheme() == resource.scheme()
                && document.host_str() == resource.host_str()
                && document.port_or_known_default() == resource.port_or_known_default()
        }
        _ => false,
    }
}

fn inline_css(document: &arch_dom::Document) -> String {
    let mut output = String::new();
    for node in document.descendants(document.root()) {
        if matches!(&node.kind, NodeKind::Element(element) if element.name == "style") {
            output.push_str(&document.text_content(node.id));
            output.push('\n');
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs};

    use super::*;
    use uuid::Uuid;

    #[derive(serde::Deserialize)]
    struct FixtureExpectation {
        title: String,
        text: String,
        link_ends_with: Option<String>,
        loaded_image_ends_with: Option<String>,
        #[serde(default)]
        diagnostic_contains: Vec<String>,
    }

    #[test]
    fn render_errors_classify_parse_and_viewport_failures() {
        let path =
            std::env::temp_dir().join(format!("archetype-invalid-utf8-{}.html", Uuid::now_v7()));
        fs::write(&path, [0xff, 0xfe]).unwrap();
        let url = Url::from_file_path(&path).unwrap();
        let error = render_url(&Loader::default(), &url, 1280.0).unwrap_err();
        let render_error = error.downcast_ref::<RenderError>().unwrap();
        assert_eq!(render_error.kind(), RenderErrorKind::Parse);
        fs::remove_file(path).unwrap();

        let error = render_url(&Loader::default(), &url, 0.0).unwrap_err();
        let render_error = error.downcast_ref::<RenderError>().unwrap();
        assert_eq!(render_error.kind(), RenderErrorKind::Render);
    }

    #[test]
    fn application_core_persists_bookmarks_in_folder() {
        let mut core = BrowserCore::in_memory().unwrap();
        let space = core.create_space("Research").unwrap();
        let folder = core
            .create_bookmark_folder(&space.id, None, "References")
            .unwrap();
        let nested = core
            .create_bookmark_folder(&space.id, Some(&folder.id), "Rust")
            .unwrap();
        let url = Url::parse("https://example.com/reference").unwrap();
        let bookmark = core
            .create_bookmark(&space.id, Some(&nested.id), "Example", &url)
            .unwrap();
        assert!(core.rename_bookmark(&bookmark.id, "Example Docs").unwrap());

        let root = core.bookmarks(&space.id, None).unwrap();
        assert_eq!(root, vec![folder.clone()]);
        assert_eq!(folder.kind, arch_store::BookmarkKind::Folder);
        assert_eq!(folder.parent_id, None);

        let children = core.bookmarks(&space.id, Some(&folder.id)).unwrap();
        assert_eq!(children, vec![nested.clone()]);
        assert_eq!(nested.parent_id.as_deref(), Some(folder.id.as_str()));

        let children = core.bookmarks(&space.id, Some(&nested.id)).unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].title, "Example Docs");
        assert_eq!(children[0].kind, arch_store::BookmarkKind::Bookmark);
        assert_eq!(children[0].parent_id.as_deref(), Some(nested.id.as_str()));
        assert_eq!(children[0].url.as_deref(), Some(url.as_str()));
    }

    #[test]
    fn fixture_runs_through_display_list() {
        let html = include_str!("../../../fixtures/pages/01-document/index.html");
        let page = render_html(&Url::parse("file:///fixture.html").unwrap(), html, 1280.0);
        assert_eq!(page.title, "Archetype V3 Fixture");
        assert!(page.display_list.commands.len() >= 3);
    }

    #[test]
    fn every_fixture_document_matches_corpus_expectations() {
        let pages = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/pages");
        let expectations: BTreeMap<String, FixtureExpectation> =
            serde_json::from_str(&fs::read_to_string(pages.join("expectations.json")).unwrap())
                .unwrap();
        assert_eq!(
            expectations.len(),
            30,
            "fixture manifest must contain 30 pages"
        );

        let mut fixture_directories = fs::read_dir(&pages)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_dir())
            .collect::<Vec<_>>();
        fixture_directories.sort();
        assert_eq!(
            fixture_directories.len(),
            30,
            "fixture corpus must contain 30 directories"
        );

        let mut documents = Vec::new();
        let loader = Loader::default();
        for directory in fixture_directories {
            let name = directory.file_name().unwrap().to_str().unwrap();
            let expectation = expectations
                .get(name)
                .unwrap_or_else(|| panic!("missing fixture expectation for {name}"));
            let index = directory.join("index.html");
            assert!(index.is_file(), "{name} is missing index.html");
            let rendered = render_url(&loader, &Url::from_file_path(&index).unwrap(), 1280.0)
                .unwrap_or_else(|error| panic!("{} failed: {error}", index.display()));
            assert_eq!(
                rendered.title, expectation.title,
                "unexpected title for {name}"
            );
            assert!(
                rendered.display_list.commands.iter().any(|command| {
                    matches!(command, arch_paint::DisplayCommand::Text { content, .. } if content.contains(&expectation.text))
                }),
                "{name} did not paint expected text {:?}",
                expectation.text
            );
            if let Some(expected) = &expectation.link_ends_with {
                assert!(
                    rendered.display_list.commands.iter().any(|command| {
                        matches!(command, arch_paint::DisplayCommand::Text { link: Some(link), .. } if link.ends_with(expected))
                    }),
                    "{name} did not resolve a link ending with {expected:?}"
                );
            }
            if let Some(expected) = &expectation.loaded_image_ends_with {
                assert!(
                    rendered.display_list.commands.iter().any(|command| {
                        matches!(command, arch_paint::DisplayCommand::Image { source, loaded: true, .. } if source.ends_with(expected))
                    }),
                    "{name} did not load an image ending with {expected:?}"
                );
            }
            for expected in &expectation.diagnostic_contains {
                assert!(
                    rendered
                        .diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.contains(expected)),
                    "{name} did not report diagnostic containing {expected:?}"
                );
            }

            for entry in fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                if path.extension().and_then(|value| value.to_str()) == Some("html") {
                    documents.push(path);
                }
            }
        }
        documents.sort();
        assert!(documents.len() >= 30, "fixture corpus unexpectedly shrank");
        for document in documents {
            let url = Url::from_file_path(&document).unwrap();
            let rendered = render_url(&loader, &url, 1280.0)
                .unwrap_or_else(|error| panic!("{} failed: {error}", document.display()));
            assert!(
                !rendered.display_list.commands.is_empty(),
                "{} produced no display commands",
                document.display()
            );
        }
    }

    #[test]
    fn local_png_is_decoded_laid_out_and_retained_for_display() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pages/05-image/index.html")
            .canonicalize()
            .unwrap();
        let url = Url::from_file_path(fixture).unwrap();
        let page = render_url(&Loader::default(), &url, 1280.0).unwrap();
        let image = page
            .display_list
            .commands
            .iter()
            .find_map(|command| match command {
                arch_paint::DisplayCommand::Image {
                    source,
                    intrinsic_width,
                    intrinsic_height,
                    ..
                } => Some((source, intrinsic_width, intrinsic_height)),
                arch_paint::DisplayCommand::Box { .. }
                | arch_paint::DisplayCommand::Text { .. } => None,
            })
            .expect("image command");
        assert_eq!((*image.1, *image.2), (4, 3));
        assert!(page.image_resources.contains_key(image.0));
        assert!(page.diagnostics.is_empty(), "{:?}", page.diagnostics);
    }

    #[test]
    fn relative_link_target_is_resolved_in_display_list() {
        let page = render_html(
            &Url::parse("https://example.test/articles/index.html").unwrap(),
            "<p><a href='../about.html'>About</a></p>",
            800.0,
        );
        let link = page
            .display_list
            .commands
            .iter()
            .find_map(|command| match command {
                arch_paint::DisplayCommand::Text { link, .. } => link.as_deref(),
                arch_paint::DisplayCommand::Box { .. }
                | arch_paint::DisplayCommand::Image { .. } => None,
            });
        assert_eq!(link, Some("https://example.test/about.html"));
    }

    #[test]
    fn active_content_is_reported_as_ignored() {
        let page = render_html(
            &Url::parse("file:///fixture.html").unwrap(),
            "<button onclick='run()'>No action</button><script>run()</script>",
            800.0,
        );
        assert!(page.diagnostics.iter().any(|item| item.contains("script")));
        assert!(
            page.diagnostics
                .iter()
                .any(|item| item.contains("event attribute"))
        );
        assert!(!page.display_list.commands.iter().any(|command| {
            matches!(command, arch_paint::DisplayCommand::Text { content, .. } if content.contains("run()"))
        }));
    }

    #[test]
    fn style_like_script_text_is_not_parsed_as_css() {
        let page = render_html(
            &Url::parse("file:///fixture.html").unwrap(),
            "<script>\"<style>p { color: red }</style>\"</script><p>Visible</p>",
            800.0,
        );
        let color = page
            .display_list
            .commands
            .iter()
            .find_map(|command| match command {
                arch_paint::DisplayCommand::Text { content, color, .. } if content == "Visible" => {
                    Some(*color)
                }
                _ => None,
            });
        assert_eq!(color, Some(None));
    }

    #[test]
    fn painted_box_precedes_its_nested_text() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pages/07-box-paint/index.html")
            .canonicalize()
            .unwrap();
        let page = render_url(
            &Loader::default(),
            &Url::from_file_path(fixture).unwrap(),
            1280.0,
        )
        .unwrap();
        let box_index = page
            .display_list
            .commands
            .iter()
            .position(|command| {
                matches!(
                    command,
                    arch_paint::DisplayCommand::Box {
                        background: Some(_),
                        border_width_px,
                        ..
                    } if *border_width_px > 0.0
                )
            })
            .expect("painted box command");
        let text_index = page
            .display_list
            .commands
            .iter()
            .position(|command| {
                matches!(command, arch_paint::DisplayCommand::Text { content, .. } if content == "Painted box")
            })
            .expect("nested heading command");
        assert!(box_index < text_index);
    }

    #[test]
    fn font_family_fixture_reaches_the_display_list() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pages/10-font-family/index.html")
            .canonicalize()
            .unwrap();
        let page = render_url(
            &Loader::default(),
            &Url::from_file_path(fixture).unwrap(),
            1280.0,
        )
        .unwrap();
        let family_for = |expected: &str| {
            page.display_list.commands.iter().any(|command| {
                matches!(
                    command,
                    arch_paint::DisplayCommand::Text {
                        font_family: Some(family),
                        ..
                    } if family == expected
                )
            })
        };
        assert!(family_for("Helvetica Neue"));
        assert!(family_for("Courier New"));
    }

    #[test]
    fn overflow_fixture_clips_descendant_commands() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pages/11-overflow-hidden/index.html")
            .canonicalize()
            .unwrap();
        let page = render_url(
            &Loader::default(),
            &Url::from_file_path(fixture).unwrap(),
            1280.0,
        )
        .unwrap();
        assert!(page.display_list.commands.iter().any(|command| {
            matches!(
                command,
                arch_paint::DisplayCommand::Box {
                    clip: Some(clip),
                    bounds,
                    ..
                } if bounds.y + bounds.height > clip.y + clip.height
            )
        }));
        assert!(page.display_list.commands.iter().any(|command| {
            matches!(
                command,
                arch_paint::DisplayCommand::Text {
                    content,
                    clip: Some(_),
                    ..
                } if content.contains("Nested clipping")
            )
        }));
    }

    #[test]
    fn border_shorthand_fixture_reaches_the_display_list() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pages/12-border-shorthand/index.html")
            .canonicalize()
            .unwrap();
        let page = render_url(
            &Loader::default(),
            &Url::from_file_path(fixture).unwrap(),
            1280.0,
        )
        .unwrap();
        assert!(page.display_list.commands.iter().any(|command| {
            matches!(
                command,
                arch_paint::DisplayCommand::Box {
                    border: Some(arch_paint::PaintColor {
                        red: 36,
                        green: 87,
                        blue: 197,
                        alpha: 255,
                    }),
                    border_width_px,
                    ..
                } if (*border_width_px - 4.0).abs() < f32::EPSILON
            )
        }));
    }

    #[test]
    fn jpeg_loads_while_missing_image_keeps_alt_fallback() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pages/09-image-formats/index.html")
            .canonicalize()
            .unwrap();
        let page = render_url(
            &Loader::default(),
            &Url::from_file_path(fixture).unwrap(),
            1280.0,
        )
        .unwrap();
        assert!(page.display_list.commands.iter().any(|command| {
            matches!(command, arch_paint::DisplayCommand::Image { loaded: true, source, .. } if source.ends_with("sample.jpg"))
        }));
        assert!(page.display_list.commands.iter().any(|command| {
            matches!(command, arch_paint::DisplayCommand::Image { loaded: false, alt, .. } if alt == "missing image fallback")
        }));
        assert!(
            page.diagnostics
                .iter()
                .any(|item| item.contains("missing.png"))
        );
    }

    #[test]
    fn application_core_persists_and_restores_navigation() {
        let path = std::env::temp_dir().join(format!("archetype-{}.db", Uuid::now_v7()));
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pages/01-document/index.html")
            .canonicalize()
            .unwrap();
        let url = Url::from_file_path(fixture).unwrap();
        {
            let mut core = BrowserCore::open(&path).unwrap();
            core.create_space("Research").unwrap();
            let page = core.create_page(&url).unwrap();
            let rendered = core.navigate(&page, &url, 1280.0).unwrap();
            assert_eq!(rendered.title, "Archetype V3 Fixture");
        }
        {
            let mut core = BrowserCore::open(&path).unwrap();
            let pages = core.pages().unwrap();
            assert_eq!(pages[0].title, "Archetype V3 Fixture");
            assert_eq!(pages[0].url, url.as_str());
            let reloaded = core.reload(&pages[0], 1280.0).unwrap();
            assert_eq!(reloaded.title, "Archetype V3 Fixture");
        }
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("db-shm"));
        let _ = fs::remove_file(path.with_extension("db-wal"));
    }

    #[test]
    fn application_core_navigates_back_and_forward() {
        let first_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pages/01-document/index.html")
            .canonicalize()
            .unwrap();
        let second_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pages/02-cascade/index.html")
            .canonicalize()
            .unwrap();
        let first_url = Url::from_file_path(first_path).unwrap();
        let second_url = Url::from_file_path(second_path).unwrap();
        let mut core = BrowserCore::in_memory().unwrap();
        let page = core.create_page(&first_url).unwrap();
        core.navigate(&page, &first_url, 1280.0).unwrap();
        core.navigate(&page, &second_url, 1280.0).unwrap();
        assert_eq!(
            core.back(&page, 1280.0).unwrap().title,
            "Archetype V3 Fixture"
        );
        assert_eq!(
            core.forward(&page, 1280.0).unwrap().title,
            "Cascade fixture"
        );
    }

    #[test]
    fn stopped_navigation_cannot_commit_a_late_result() {
        let mut core = BrowserCore::in_memory().unwrap();
        let url = Url::parse("file:///cancelled.html").unwrap();
        let page = core.create_page(&url).unwrap();
        let pending = core.start_navigation(&page, &url).unwrap();
        assert!(core.stop(&page).unwrap());
        let rendered = render_html(
            &url,
            "<title>Late result</title><p>must not commit</p>",
            1280.0,
        );
        assert!(!core.finish_navigation(&page, &pending, &rendered).unwrap());
        assert!(core.pages().unwrap()[0].title.is_empty());
    }

    #[test]
    fn discovers_only_stylesheet_links() {
        let url = Url::parse("https://example.com/docs/index.html").unwrap();
        let document = arch_html::parse(
            "<link rel='stylesheet' href='../style.css'><link rel='icon' href='icon.png'>",
        );
        assert_eq!(
            stylesheet_urls(&document, &url),
            vec![Url::parse("https://example.com/style.css").unwrap()]
        );
    }

    #[test]
    fn same_origin_includes_scheme_host_and_port() {
        let document = Url::parse("https://example.com/page").unwrap();
        assert!(same_origin(
            &document,
            &Url::parse("https://example.com/style.css").unwrap()
        ));
        assert!(!same_origin(
            &document,
            &Url::parse("http://example.com/style.css").unwrap()
        ));
        assert!(!same_origin(
            &document,
            &Url::parse("https://cdn.example.com/style.css").unwrap()
        ));
    }
}
