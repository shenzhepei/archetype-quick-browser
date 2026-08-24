use std::{collections::HashMap, path::Path, str};

use anyhow::{Context, Result};
use arch_dom::NodeKind;
use arch_net::{LoadError, LoadErrorKind, Loader, ResponseBytes};
use arch_paint::DisplayList;
use arch_session::{
    BrowserCommand, BrowserEvent, HibernationSnapshot, Session, Viewport,
    cookies::{CookieJar, CookieRequest},
    forms::{
        ControlId, ControlKind, FormControl, FormMethod, FormState, FormSubmission, SelectOption,
    },
};
use arch_store::{Bookmark, Page, Space, Store};
use archetype_types::{ArchetypeUrl, LoadStage, NavigationId, PageId};
use thiserror::Error;
use url::Url;

use crate::profile_cookies::CookieCipher;

pub mod runtime_broker;
pub mod snapshot;

mod profile_cookies;

const IMAGE_LIMIT: usize = 20 * 1024 * 1024;
const PAGE_LIMIT: usize = 50 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct RenderedPage {
    pub final_url: Url,
    pub title: String,
    pub display_list: DisplayList,
    pub diagnostics: Vec<String>,
    pub image_resources: HashMap<String, Vec<u8>>,
    pub forms: Vec<FormState>,
    pub form_controls: Vec<PositionedFormControl>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PositionedFormControl {
    pub form_index: usize,
    pub control_id: ControlId,
    pub bounds: arch_layout::Rect,
    pub clip: Option<arch_layout::Rect>,
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
    cookie_jar: CookieJar,
    cookie_cipher: CookieCipher,
}

#[derive(Clone, Debug)]
pub struct PendingNavigation {
    page_id: PageId,
    navigation_id: NavigationId,
    url: Url,
    top_level_url: Url,
}

impl PendingNavigation {
    #[must_use]
    pub fn url(&self) -> &Url {
        &self.url
    }

    #[must_use]
    pub fn page_id(&self) -> &PageId {
        &self.page_id
    }

    #[must_use]
    pub const fn navigation_id(&self) -> NavigationId {
        self.navigation_id
    }

    #[must_use]
    pub fn top_level_url(&self) -> &Url {
        &self.top_level_url
    }
}

impl BrowserCore {
    /// Opens a persistent browser profile and restores its page identities.
    ///
    /// # Errors
    /// Returns an error when the profile database or network client cannot be initialized.
    pub fn open(profile: impl AsRef<Path>) -> Result<Self> {
        let profile = profile.as_ref();
        let cookie_cipher = CookieCipher::for_profile(profile)
            .context("could not open profile Cookie encryption key")?;
        let store = Store::open(profile).context("could not open browser profile")?;
        Self::with_store(store, cookie_cipher)
    }

    /// Opens an encrypted persistent profile with an injected key for process-level probes.
    ///
    /// Production callers should use [`Self::open`], which obtains the profile key from Keychain.
    ///
    /// # Errors
    /// Returns an error when the profile database or network client cannot be initialized.
    #[doc(hidden)]
    pub fn open_with_cookie_key_for_testing(
        profile: impl AsRef<Path>,
        key: [u8; 32],
    ) -> Result<Self> {
        let store = Store::open(profile).context("could not open browser profile")?;
        Self::with_store(store, CookieCipher::from_key(key))
    }

    /// Creates an in-memory browser profile for tests.
    ///
    /// # Errors
    /// Returns an error when the profile database or network client cannot be initialized.
    pub fn in_memory() -> Result<Self> {
        Self::with_store(Store::in_memory()?, CookieCipher::ephemeral()?)
    }

    fn with_store(store: Store, cookie_cipher: CookieCipher) -> Result<Self> {
        let cookie_jar = match store.cookie_state()? {
            Some(state) => {
                let plaintext = cookie_cipher
                    .decrypt(&state)
                    .context("could not decrypt profile Cookie state")?;
                CookieJar::from_persistent_json(&plaintext)
                    .context("could not restore profile Cookie state")?
            }
            None => CookieJar::new(),
        };
        let mut core = Self {
            store,
            session: Session::default(),
            loader: Loader::new()?,
            cookie_jar,
            cookie_cipher,
        };
        for page in core.store.pages()? {
            if let Ok(id) = page.id.parse::<PageId>() {
                let restored = core
                    .store
                    .page_hibernation(&page.id)?
                    .and_then(|value| serde_json::from_str::<HibernationSnapshot>(&value).ok())
                    .is_some_and(|snapshot| {
                        snapshot.page_id == id && core.session.restore_hibernation(snapshot).is_ok()
                    });
                if restored {
                    continue;
                }
                if let Ok(url) = page.url.parse::<ArchetypeUrl>() {
                    core.session.restore_page(id, url);
                } else {
                    core.session.open_page(id);
                }
            }
        }
        Ok(core)
    }

    #[must_use]
    pub fn cookie_header(&self, request: CookieRequest<'_>) -> Option<String> {
        self.cookie_jar.request_header(request)
    }

    /// Applies one HTTP `Set-Cookie` header and persists the resulting profile state.
    ///
    /// # Errors
    /// Returns an error when the Cookie is invalid, encryption fails, or the database update
    /// cannot be committed.
    pub fn store_response_cookie(&mut self, response_url: &Url, header: &str) -> Result<()> {
        self.cookie_jar
            .store_response_header(response_url, header)
            .context("could not apply response Cookie")?;
        self.persist_cookie_jar()
    }

    fn persist_cookie_jar(&self) -> Result<()> {
        let plaintext = self
            .cookie_jar
            .persistent_json()
            .context("could not serialize profile Cookie state")?;
        let encrypted = self
            .cookie_cipher
            .encrypt(&plaintext)
            .context("could not encrypt profile Cookie state")?;
        self.store
            .save_cookie_state(&encrypted)
            .context("could not persist profile Cookie state")
    }

    #[must_use]
    pub fn cookie_jar_snapshot(&self) -> CookieJar {
        self.cookie_jar.clone()
    }

    /// Commits Cookie changes produced by a Browser-side background broker.
    ///
    /// # Errors
    /// Returns an error when encryption or profile persistence fails.
    pub fn commit_cookie_jar_snapshot(&mut self, cookie_jar: CookieJar) -> Result<()> {
        self.cookie_jar = cookie_jar;
        self.persist_cookie_jar()
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

    /// Persists a page's bounded recovery metadata and invalidates active work.
    ///
    /// # Errors
    /// Returns an error when the page is invalid, automatic hibernation is blocked by a dirty
    /// form, or the snapshot cannot be persisted.
    pub fn hibernate_page(
        &mut self,
        page: &Page,
        rendered: &RenderedPage,
        viewport: Viewport,
        scroll_y: f32,
        automatic: bool,
    ) -> Result<()> {
        let page_id = parsed_page_id(page).context("page has invalid UUID")?;
        let form_dirty = rendered.forms.iter().any(FormState::is_dirty);
        let snapshot = self
            .session
            .hibernation_snapshot(
                &page_id,
                rendered.title.clone(),
                viewport,
                scroll_y,
                form_dirty,
                automatic,
            )
            .context("page cannot hibernate")?;
        let encoded = serde_json::to_string(&snapshot).context("could not encode page snapshot")?;
        self.store
            .save_page_hibernation(&page.id, &encoded)
            .context("could not persist page snapshot")?;
        let _ = self.stop(page)?;
        Ok(())
    }

    /// Restores hibernation metadata and recreates page content through navigation.
    ///
    /// # Errors
    /// Returns an error when metadata is invalid or re-navigation fails.
    pub fn wake_page(&mut self, page: &Page, viewport_width: f32) -> Result<RenderedPage> {
        if let Some(encoded) = self.store.page_hibernation(&page.id)? {
            let snapshot: HibernationSnapshot =
                serde_json::from_str(&encoded).context("could not decode page snapshot")?;
            let page_id = parsed_page_id(page).context("page has invalid UUID")?;
            if snapshot.page_id != page_id {
                anyhow::bail!("page snapshot identity does not match");
            }
            self.session
                .restore_hibernation(snapshot)
                .context("could not restore page snapshot")?;
        }
        let rendered = self.reload(page, viewport_width)?;
        self.store.delete_page_hibernation(&page.id)?;
        Ok(rendered)
    }

    /// Reads validated hibernation metadata without waking the page.
    ///
    /// # Errors
    /// Returns an error when storage or snapshot validation fails.
    pub fn page_hibernation(&self, page: &Page) -> Result<Option<HibernationSnapshot>> {
        let Some(encoded) = self.store.page_hibernation(&page.id)? else {
            return Ok(None);
        };
        let snapshot: HibernationSnapshot =
            serde_json::from_str(&encoded).context("could not decode page snapshot")?;
        let page_id = parsed_page_id(page).context("page has invalid UUID")?;
        if snapshot.page_id != page_id {
            anyhow::bail!("page snapshot identity does not match");
        }
        Ok(Some(snapshot))
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

    /// Executes a user-initiated GET or form-urlencoded POST submission.
    ///
    /// # Errors
    /// Returns an error when loading, rendering, Cookie persistence, or navigation persistence
    /// fails.
    pub fn submit_form(
        &mut self,
        page: &Page,
        submission: &FormSubmission,
        viewport_width: f32,
    ) -> Result<RenderedPage> {
        let pending = self.start_navigation(page, &submission.target)?;
        if submission.method == FormMethod::Get {
            return self.execute_navigation(page, &pending, viewport_width);
        }
        let render_result = render_post_with_cookies(
            &self.loader,
            &mut self.cookie_jar,
            submission,
            &pending.top_level_url,
            viewport_width,
        );
        self.persist_cookie_jar()?;
        let rendered = render_result?;
        if !self.finish_navigation(page, &pending, &rendered)? {
            anyhow::bail!("form submission result became stale");
        }
        Ok(rendered)
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
        let url = url
            .as_str()
            .parse::<ArchetypeUrl>()
            .context("navigation URL is invalid")?;
        self.start_command(BrowserCommand::Navigate { page_id, url })
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
        let final_url = rendered
            .final_url
            .as_str()
            .parse::<ArchetypeUrl>()
            .context("final navigation URL is invalid")?;
        if !self
            .session
            .commit_final_url(&pending.page_id, pending.navigation_id, final_url)
        {
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
                stage: LoadStage::Cancelled,
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
        let render_result = render_url_with_cookies(
            &self.loader,
            &mut self.cookie_jar,
            &pending.url,
            &pending.top_level_url,
            viewport_width,
        );
        self.persist_cookie_jar()?;
        let rendered = render_result?;
        if !self.finish_navigation(page, pending, &rendered)? {
            anyhow::bail!("navigation result became stale");
        }
        Ok(rendered)
    }

    fn start_command(&mut self, command: BrowserCommand) -> Result<PendingNavigation> {
        let page_id = match &command {
            BrowserCommand::Navigate { page_id, .. }
            | BrowserCommand::Back { page_id }
            | BrowserCommand::Forward { page_id }
            | BrowserCommand::Reload { page_id }
            | BrowserCommand::Stop { page_id }
            | BrowserCommand::Resize { page_id, .. }
            | BrowserCommand::Scroll { page_id, .. } => page_id,
        };
        let top_level_url = self
            .session
            .current_url(page_id)
            .and_then(|url| Url::parse(url.as_str()).ok());
        let event = self.session.handle(command);
        let BrowserEvent::NavigationStarted {
            page_id,
            navigation_id,
            url,
        } = event
        else {
            anyhow::bail!("navigation command was ignored");
        };
        let url = Url::parse(url.as_str()).context("session returned invalid navigation URL")?;
        let top_level_url = top_level_url.unwrap_or_else(|| url.clone());
        Ok(PendingNavigation {
            page_id,
            navigation_id,
            url,
            top_level_url,
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

#[derive(Clone, Copy)]
enum ResourceContext {
    Document,
    Subresource,
}

trait PageResourceLoader {
    fn load(
        &mut self,
        url: &Url,
        limit: usize,
        context: ResourceContext,
    ) -> Result<ResponseBytes, LoadError>;
}

struct StatelessPageLoader<'a>(&'a Loader);

impl PageResourceLoader for StatelessPageLoader<'_> {
    fn load(
        &mut self,
        url: &Url,
        limit: usize,
        _context: ResourceContext,
    ) -> Result<ResponseBytes, LoadError> {
        self.0.load_with_limit(url, limit)
    }
}

struct CookiePageLoader<'a> {
    loader: &'a Loader,
    cookie_jar: &'a mut CookieJar,
    top_level_url: Url,
}

impl PageResourceLoader for CookiePageLoader<'_> {
    fn load(
        &mut self,
        url: &Url,
        limit: usize,
        context: ResourceContext,
    ) -> Result<ResponseBytes, LoadError> {
        let response = self.loader.load_with_cookies(
            url,
            limit,
            self.cookie_jar,
            &self.top_level_url,
            matches!(context, ResourceContext::Document),
        )?;
        if matches!(context, ResourceContext::Document) {
            self.top_level_url.clone_from(&response.final_url);
        }
        Ok(response)
    }
}

/// Loads and renders a UTF-8 static document into a V3 display list.
///
/// # Errors
/// Returns a typed error when loading fails, the document is not valid UTF-8, or the viewport is
/// invalid.
pub fn render_url(loader: &Loader, url: &Url, viewport_width: f32) -> Result<RenderedPage> {
    let mut loader = StatelessPageLoader(loader);
    render_url_with_loader(&mut loader, url, viewport_width)
}

fn render_url_with_cookies(
    loader: &Loader,
    cookie_jar: &mut CookieJar,
    url: &Url,
    top_level_url: &Url,
    viewport_width: f32,
) -> Result<RenderedPage> {
    let mut loader = CookiePageLoader {
        loader,
        cookie_jar,
        top_level_url: top_level_url.clone(),
    };
    render_url_with_loader(&mut loader, url, viewport_width)
}

fn render_post_with_cookies(
    loader: &Loader,
    cookie_jar: &mut CookieJar,
    submission: &FormSubmission,
    top_level_url: &Url,
    viewport_width: f32,
) -> Result<RenderedPage> {
    validate_viewport(&submission.target, viewport_width)?;
    let response = loader
        .submit_with_cookies(
            &submission.target,
            arch_net::DOCUMENT_LIMIT,
            cookie_jar,
            top_level_url,
            &submission.encoded,
        )
        .map_err(|source| RenderError::Load {
            url: submission.target.clone(),
            source,
        })?;
    let mut loader = CookiePageLoader {
        loader,
        cookie_jar,
        top_level_url: response.final_url.clone(),
    };
    render_response_with_loader(&mut loader, &response, viewport_width)
}

fn render_url_with_loader(
    loader: &mut impl PageResourceLoader,
    url: &Url,
    viewport_width: f32,
) -> Result<RenderedPage> {
    validate_viewport(url, viewport_width)?;
    let response = loader
        .load(url, arch_net::DOCUMENT_LIMIT, ResourceContext::Document)
        .map_err(|source| RenderError::Load {
            url: url.clone(),
            source,
        })?;
    render_response_with_loader(loader, &response, viewport_width)
}

fn render_response_with_loader(
    loader: &mut impl PageResourceLoader,
    response: &ResponseBytes,
    viewport_width: f32,
) -> Result<RenderedPage> {
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
        match loader.load(
            &stylesheet_url,
            arch_net::DOCUMENT_LIMIT,
            ResourceContext::Subresource,
        ) {
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

fn validate_viewport(url: &Url, viewport_width: f32) -> Result<()> {
    if !viewport_width.is_finite() || viewport_width <= 0.0 {
        return Err(RenderError::Render { url: url.clone() }.into());
    }
    Ok(())
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
    let forms = extract_forms(document, url);
    let form_by_control: HashMap<_, _> = forms
        .iter()
        .enumerate()
        .flat_map(|(form_index, form)| {
            form.controls
                .iter()
                .map(move |control| (control.id, form_index))
        })
        .collect();
    let form_controls = layout
        .boxes
        .iter()
        .filter_map(|layout_box| {
            let control_id = ControlId(layout_box.node_id.0);
            form_by_control
                .get(&control_id)
                .map(|form_index| PositionedFormControl {
                    form_index: *form_index,
                    control_id,
                    bounds: layout_box.bounds,
                    clip: layout_box.clip,
                })
        })
        .collect();
    let mut diagnostics = stylesheet.diagnostics;
    diagnostics.extend(document_diagnostics(document));
    RenderedPage {
        final_url: url.clone(),
        title,
        display_list,
        diagnostics,
        image_resources: HashMap::new(),
        forms,
        form_controls,
    }
}

fn extract_forms(document: &arch_dom::Document, base: &Url) -> Vec<FormState> {
    document
        .descendants(document.root())
        .filter_map(|node| {
            let NodeKind::Element(element) = &node.kind else {
                return None;
            };
            if element.name != "form" {
                return None;
            }
            let action = element
                .attribute("action")
                .and_then(|action| base.join(action).ok())
                .unwrap_or_else(|| base.clone());
            let method = if element
                .attribute("method")
                .is_some_and(|method| method.eq_ignore_ascii_case("post"))
            {
                FormMethod::Post
            } else {
                FormMethod::Get
            };
            let controls = document
                .descendants(node.id)
                .filter_map(|control| extract_form_control(document, control))
                .collect();
            Some(FormState::new(action, method, controls))
        })
        .collect()
}

fn extract_form_control(
    document: &arch_dom::Document,
    node: &arch_dom::Node,
) -> Option<FormControl> {
    let NodeKind::Element(element) = &node.kind else {
        return None;
    };
    if element.attribute("disabled").is_some() {
        return None;
    }
    let name = element.attribute("name").map(str::to_owned);
    let (kind, value, checked, options, selected_index) = match element.name.as_str() {
        "input" => {
            let input_type = element.attribute("type").unwrap_or("text");
            let kind = match input_type.to_ascii_lowercase().as_str() {
                "text" => ControlKind::Text,
                "password" => ControlKind::Password,
                "checkbox" => ControlKind::Checkbox,
                "radio" => ControlKind::Radio,
                "submit" => ControlKind::Submit,
                "button" => ControlKind::Button,
                _ => return None,
            };
            let default = if matches!(kind, ControlKind::Checkbox | ControlKind::Radio) {
                "on"
            } else if kind == ControlKind::Submit {
                "Submit"
            } else {
                ""
            };
            (
                kind,
                element.attribute("value").unwrap_or(default).to_owned(),
                element.attribute("checked").is_some(),
                Vec::new(),
                None,
            )
        }
        "select" => {
            let options: Vec<_> = document
                .descendants(node.id)
                .filter_map(|option| {
                    let NodeKind::Element(option_element) = &option.kind else {
                        return None;
                    };
                    if option_element.name != "option"
                        || option_element.attribute("disabled").is_some()
                    {
                        return None;
                    }
                    let label = document.text_content(option.id);
                    Some((
                        SelectOption {
                            value: option_element
                                .attribute("value")
                                .unwrap_or(&label)
                                .to_owned(),
                            label,
                        },
                        option_element.attribute("selected").is_some(),
                    ))
                })
                .collect();
            let selected_index = options
                .iter()
                .position(|(_, selected)| *selected)
                .or((!options.is_empty()).then_some(0));
            (
                ControlKind::Select,
                String::new(),
                false,
                options.into_iter().map(|(option, _)| option).collect(),
                selected_index,
            )
        }
        "button" => {
            let kind = match element.attribute("type").unwrap_or("submit") {
                value if value.eq_ignore_ascii_case("submit") => ControlKind::Submit,
                value if value.eq_ignore_ascii_case("button") => ControlKind::Button,
                _ => return None,
            };
            (
                kind,
                element
                    .attribute("value")
                    .map_or_else(|| document.text_content(node.id), str::to_owned),
                false,
                Vec::new(),
                None,
            )
        }
        _ => return None,
    };
    Some(FormControl {
        id: ControlId(node.id.0),
        name,
        kind,
        value,
        checked,
        options,
        selected_index,
    })
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
    loader: &mut impl PageResourceLoader,
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
        match loader.load(&source, IMAGE_LIMIT, ResourceContext::Subresource) {
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
    use std::{
        collections::BTreeMap,
        fs,
        io::{Read as _, Write as _},
        net::TcpListener,
        thread,
    };

    use super::*;
    use arch_session::cookies::RequestMethod;
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
            50,
            "fixture manifest must contain 50 pages"
        );

        let mut fixture_directories = fs::read_dir(&pages)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_dir())
            .collect::<Vec<_>>();
        fixture_directories.sort();
        assert_eq!(
            fixture_directories.len(),
            50,
            "fixture corpus must contain 50 directories"
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
    fn hibernated_page_restores_history_and_wakes_by_navigation() {
        let path = std::env::temp_dir().join(format!("archetype-hibernate-{}.db", Uuid::now_v7()));
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
        {
            let mut core = BrowserCore::open(&path).unwrap();
            let page = core.create_page(&first_url).unwrap();
            core.navigate(&page, &first_url, 960.0).unwrap();
            core.navigate(&page, &second_url, 960.0).unwrap();
            let rendered = core.back(&page, 960.0).unwrap();
            core.hibernate_page(
                &page,
                &rendered,
                Viewport {
                    width: 960.0,
                    height: 640.0,
                },
                128.0,
                true,
            )
            .unwrap();
        }
        {
            let mut core = BrowserCore::open(&path).unwrap();
            let page = core.pages().unwrap().remove(0);
            assert!(core.can_go_forward(&page));
            let rendered = core.wake_page(&page, 960.0).unwrap();
            assert_eq!(rendered.title, "Archetype V3 Fixture");
            assert!(core.can_go_forward(&page));
            assert!(core.store.page_hibernation(&page.id).unwrap().is_none());
        }
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("db-shm"));
        let _ = fs::remove_file(path.with_extension("db-wal"));
    }

    #[test]
    fn automatic_hibernation_never_persists_dirty_form_values() {
        let directory =
            std::env::temp_dir().join(format!("archetype-dirty-form-{}", Uuid::now_v7()));
        fs::create_dir(&directory).unwrap();
        let html = directory.join("form.html");
        fs::write(
            &html,
            "<title>Form</title><form><input name='secret'><button>Send</button></form>",
        )
        .unwrap();
        let database = directory.join("profile.db");
        let url = Url::from_file_path(&html).unwrap();
        let mut core = BrowserCore::open(&database).unwrap();
        let page = core.create_page(&url).unwrap();
        let mut rendered = core.navigate(&page, &url, 960.0).unwrap();
        let control_id = rendered.forms[0].controls[0].id;
        rendered.forms[0]
            .set_text(control_id, "super-secret".to_owned())
            .unwrap();
        assert!(
            core.hibernate_page(
                &page,
                &rendered,
                Viewport {
                    width: 960.0,
                    height: 640.0,
                },
                0.0,
                true,
            )
            .is_err()
        );
        assert!(core.store.page_hibernation(&page.id).unwrap().is_none());
        drop(core);
        let bytes = fs::read(&database).unwrap();
        assert!(!bytes.windows(12).any(|window| window == b"super-secret"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn cookie_profiles_are_isolated_by_store_and_key() {
        let origin = Url::parse("https://example.com/").unwrap();
        let mut first = BrowserCore::with_store(
            Store::in_memory().unwrap(),
            profile_cookies::CookieCipher::from_key([1; 32]),
        )
        .unwrap();
        let second = BrowserCore::with_store(
            Store::in_memory().unwrap(),
            profile_cookies::CookieCipher::from_key([2; 32]),
        )
        .unwrap();
        first
            .store_response_cookie(
                &origin,
                "profile=first; Secure; Expires=Tue, 03 Aug 2100 00:38:37 GMT",
            )
            .unwrap();
        let request = CookieRequest {
            url: &origin,
            top_level_url: &origin,
            method: RequestMethod::Get,
            is_top_level_navigation: true,
        };
        assert_eq!(
            first.cookie_header(request),
            Some("profile=first".to_owned())
        );
        assert_eq!(second.cookie_header(request), None);
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
    fn application_core_encrypts_and_restores_persistent_cookies() {
        let path = std::env::temp_dir().join(format!("archetype-cookies-{}.db", Uuid::now_v7()));
        let origin = Url::parse("https://example.com/account").unwrap();
        let key = [9; 32];
        {
            let store = Store::open(&path).unwrap();
            let mut core =
                BrowserCore::with_store(store, profile_cookies::CookieCipher::from_key(key))
                    .unwrap();
            core.store_response_cookie(
                &origin,
                "session=plain-secret; Secure; HttpOnly; Expires=Tue, 03 Aug 2100 00:38:37 GMT",
            )
            .unwrap();
        }
        let database = fs::read(&path).unwrap();
        assert!(
            !database
                .windows(b"plain-secret".len())
                .any(|window| window == b"plain-secret")
        );

        let restored = BrowserCore::with_store(
            Store::open(&path).unwrap(),
            profile_cookies::CookieCipher::from_key(key),
        )
        .unwrap();
        assert_eq!(
            restored.cookie_header(CookieRequest {
                url: &origin,
                top_level_url: &origin,
                method: RequestMethod::Get,
                is_top_level_navigation: true,
            }),
            Some("session=plain-secret".to_owned())
        );
        drop(restored);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("db-shm"));
        let _ = fs::remove_file(path.with_extension("db-wal"));
    }

    #[test]
    fn application_core_applies_cookies_to_following_navigation() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            for expected_path in ["/set", "/check"] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 2048];
                let length = stream.read(&mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..length]);
                assert!(request.starts_with(&format!("GET {expected_path} ")));
                if expected_path == "/set" {
                    assert!(!request.to_ascii_lowercase().contains("cookie:"));
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nSet-Cookie: session=ready; Path=/; HttpOnly; Expires=Tue, 03 Aug 2100 00:38:37 GMT\r\nContent-Length: 18\r\nConnection: close\r\n\r\n<title>Set</title>"
                    )
                    .unwrap();
                } else {
                    assert!(
                        request
                            .to_ascii_lowercase()
                            .contains("cookie: session=ready")
                    );
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: 20\r\nConnection: close\r\n\r\n<title>Check</title>"
                    )
                    .unwrap();
                }
            }
        });
        let first_url = Url::parse(&format!("http://{address}/set")).unwrap();
        let second_url = Url::parse(&format!("http://{address}/check")).unwrap();
        let mut core = BrowserCore::in_memory().unwrap();
        let page = core.create_page(&first_url).unwrap();
        assert_eq!(
            core.navigate(&page, &first_url, 1280.0).unwrap().title,
            "Set"
        );
        assert_eq!(
            core.navigate(&page, &second_url, 1280.0).unwrap().title,
            "Check"
        );
        server.join().unwrap();
    }

    #[test]
    fn extracts_basic_form_controls_and_successful_values() {
        let page = render_html(
            &Url::parse("https://example.com/account/page").unwrap(),
            "<form action='../submit' method='post'>
               <input name='user' value='Ada'>
               <input type='password' name='password'>
               <input type='checkbox' name='remember' value='yes' checked>
               <input type='radio' name='mode' value='one'>
               <input type='radio' name='mode' value='two' checked>
               <select name='size'><option value='s'>Small</option><option value='l' selected>Large</option></select>
               <button name='submit' value='go'>Send</button>
               <button type='button'>No action</button>
             </form>",
            1280.0,
        );
        assert_eq!(page.forms.len(), 1);
        let form = &page.forms[0];
        assert_eq!(form.action.as_str(), "https://example.com/submit");
        assert_eq!(form.method, FormMethod::Post);
        assert_eq!(form.controls.len(), 8);
        assert_eq!(page.form_controls.len(), form.controls.len());
        assert!(page.form_controls.iter().all(|positioned| {
            positioned.form_index == 0
                && form
                    .controls
                    .iter()
                    .any(|control| control.id == positioned.control_id)
                && positioned.bounds.width > 0.0
                && positioned.bounds.height > 0.0
        }));
        let submitter = form
            .controls
            .iter()
            .find(|control| control.kind == ControlKind::Submit)
            .unwrap()
            .id;
        let submission = form.submission(Some(submitter)).unwrap();
        assert_eq!(
            submission.encoded,
            "user=Ada&password=&remember=yes&mode=two&size=l&submit=go"
        );
    }

    #[test]
    fn application_core_submits_form_urlencoded_post() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let length = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..length]);
            assert!(request.starts_with("POST /submit "));
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("content-type: application/x-www-form-urlencoded")
            );
            assert!(request.ends_with("query=rust+browser"));
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: 21\r\nConnection: close\r\n\r\n<title>Posted</title>"
            )
            .unwrap();
        });
        let target = Url::parse(&format!("http://{address}/submit")).unwrap();
        let form = FormState::new(
            target.clone(),
            FormMethod::Post,
            vec![FormControl {
                id: ControlId(1),
                name: Some("query".to_owned()),
                kind: ControlKind::Text,
                value: "rust browser".to_owned(),
                checked: false,
                options: Vec::new(),
                selected_index: None,
            }],
        );
        let submission = form.submission(None).unwrap();
        let mut core = BrowserCore::in_memory().unwrap();
        let page = core.create_page(&target).unwrap();
        let rendered = core.submit_form(&page, &submission, 1280.0).unwrap();
        server.join().unwrap();
        assert_eq!(rendered.title, "Posted");
    }

    #[test]
    fn form_post_sends_same_site_cookie_and_suppresses_it_cross_site() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            for (method, expect_cookie) in
                [("POST", Some(true)), ("GET", None), ("POST", Some(false))]
            {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 4096];
                let length = stream.read(&mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..length]).to_ascii_lowercase();
                assert!(request.starts_with(&method.to_ascii_lowercase()));
                if let Some(expect_cookie) = expect_cookie {
                    assert_eq!(request.contains("cookie: form=allowed"), expect_cookie);
                }
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: 21\r\nConnection: close\r\n\r\n<title>Posted</title>"
                )
                .unwrap();
            }
        });
        let target = Url::parse(&format!("http://{address}/submit")).unwrap();
        let cross_site = Url::parse(&format!("http://localhost:{}/", address.port())).unwrap();
        let mut core = BrowserCore::in_memory().unwrap();
        core.store_response_cookie(&target, "form=allowed; Path=/; SameSite=Lax")
            .unwrap();
        let same_site_page = core.create_page(&target).unwrap();
        let cross_site_page = core.create_page(&cross_site).unwrap();
        let submission = FormSubmission {
            method: FormMethod::Post,
            target,
            encoded: "field=value".to_owned(),
        };
        core.submit_form(&same_site_page, &submission, 1280.0)
            .unwrap();
        core.navigate(&cross_site_page, &cross_site, 1280.0)
            .unwrap();
        core.submit_form(&cross_site_page, &submission, 1280.0)
            .unwrap();
        server.join().unwrap();
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
