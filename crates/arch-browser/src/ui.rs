use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    io::Write as _,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use arch_browser::{
    AppearancePreference, BrowserCore, PendingNavigation, RenderError, RenderErrorKind,
    RenderedPage,
    runtime_broker::{
        BrokerRequest, load_favicon_with_cookies, load_form_submission_with_cookies,
        load_static_document_with_cookies,
    },
};
use arch_net::{LoadError, LoadErrorKind, Loader};
use arch_paint::{DisplayCommand, PaintColor, TextDecoration};
use arch_session::Viewport;
use arch_session::cookies::CookieJar;
use arch_session::forms::{ControlId, ControlKind, FormMethod, FormSubmission};
use arch_store::{Bookmark, BookmarkKind, HistoryEntry, Page, Space};
use arch_style::{FontStyle as PageFontStyle, FontWeight as PageFontWeight, TextAlign};
use archetype_sdk::runtime_client::RuntimeSupervisor;
use chrono::{DateTime, Local, Utc};
use gpui::{
    AnyElement, AppContext as _, Application, AssetSource, BoxShadow, Context, Entity, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement, Render, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, Styled, Subscription, Task, Window, WindowBounds,
    WindowOptions, div, img, point, prelude::FluentBuilder as _, px, rgba, size,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, IconNamed, Root, Sizable as _,
    StyledExt as _, Theme, ThemeMode, TitleBar,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenu, PopupMenuItem},
    radio::Radio,
    scroll::ScrollableElement as _,
    spinner::Spinner,
    v_flex,
};
use url::Url;

use crate::{i18n::Language, logging};

const ABOUT_BLANK: &str = "about:blank";
const ARCHETYPE_HISTORY: &str = "archetype://history";
const ARCHETYPE_SETTINGS_APPEARANCE: &str = "archetype://settings/appearance";
const ARCHETYPE_SETTINGS_ABOUT: &str = "archetype://settings/about";
const HISTORY_PAGE_LIMIT: usize = 1_000;
const MAX_RESIDENT_RENDERED_PAGES: usize = 8;

struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        let bytes: Option<&'static [u8]> = match path {
            "archetype-icons/arrow-left-line.svg" => Some(include_bytes!(
                "../../../assets/icons/system/arrow-left-line.svg"
            )),
            "archetype-icons/arrow-right-line.svg" => Some(include_bytes!(
                "../../../assets/icons/system/arrow-right-line.svg"
            )),
            "archetype-icons/refresh-line.svg" => Some(include_bytes!(
                "../../../assets/icons/system/refresh-line.svg"
            )),
            "archetype-icons/add-line.svg" => {
                Some(include_bytes!("../../../assets/icons/system/add-line.svg"))
            }
            "archetype-icons/close-line.svg" => Some(include_bytes!(
                "../../../assets/icons/system/close-line.svg"
            )),
            "archetype-icons/delete-bin-line.svg" => Some(include_bytes!(
                "../../../assets/icons/system/delete-bin-line.svg"
            )),
            "archetype-icons/find-replace-line.svg" => Some(include_bytes!(
                "../../../assets/icons/system/find-replace-line.svg"
            )),
            "archetype-icons/alert-line.svg" => Some(include_bytes!(
                "../../../assets/icons/system/alert-line.svg"
            )),
            "archetype-icons/star-line.svg" => {
                Some(include_bytes!("../../../assets/icons/system/star-line.svg"))
            }
            _ => None,
        };
        if let Some(bytes) = bytes {
            Ok(Some(Cow::Borrowed(bytes)))
        } else {
            gpui_component_assets::Assets.load(path)
        }
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        let mut assets = gpui_component_assets::Assets.list(path)?;
        if path.is_empty() || "archetype-icons".starts_with(path) {
            assets.extend(
                [
                    "archetype-icons/arrow-left-line.svg",
                    "archetype-icons/arrow-right-line.svg",
                    "archetype-icons/refresh-line.svg",
                    "archetype-icons/add-line.svg",
                    "archetype-icons/close-line.svg",
                    "archetype-icons/delete-bin-line.svg",
                    "archetype-icons/find-replace-line.svg",
                    "archetype-icons/alert-line.svg",
                    "archetype-icons/star-line.svg",
                ]
                .into_iter()
                .map(Into::into),
            );
        }
        Ok(assets)
    }
}

#[derive(Clone, Copy)]
enum AppIcon {
    Back,
    Forward,
    Refresh,
    Add,
    Close,
    Delete,
    Rename,
    Alert,
    Star,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TabIconMode {
    Default,
    Favicon,
    Loading,
}

impl IconNamed for AppIcon {
    fn path(self) -> SharedString {
        match self {
            Self::Back => "archetype-icons/arrow-left-line.svg",
            Self::Forward => "archetype-icons/arrow-right-line.svg",
            Self::Refresh => "archetype-icons/refresh-line.svg",
            Self::Add => "archetype-icons/add-line.svg",
            Self::Close => "archetype-icons/close-line.svg",
            Self::Delete => "archetype-icons/delete-bin-line.svg",
            Self::Rename => "archetype-icons/find-replace-line.svg",
            Self::Alert => "archetype-icons/alert-line.svg",
            Self::Star => "archetype-icons/star-line.svg",
        }
        .into()
    }
}

#[derive(Clone, Debug)]
struct ErrorView {
    title: &'static str,
    detail: String,
}

impl ErrorView {
    fn input(language: Language, detail: impl Into<String>) -> Self {
        Self {
            title: language.invalid_input(),
            detail: detail.into(),
        }
    }

    fn application(language: Language, error: &impl std::fmt::Display) -> Self {
        Self {
            title: language.application_error(),
            detail: error.to_string(),
        }
    }

    fn navigation(language: Language, error: &anyhow::Error) -> Self {
        Self {
            title: navigation_error_title(language, error),
            detail: format!("{error:#}"),
        }
    }
}

fn navigation_error_title(language: Language, error: &anyhow::Error) -> &'static str {
    if let Some(kind) = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<LoadError>())
        .map(LoadError::kind)
    {
        return load_error_title(language, kind);
    }
    match error
        .chain()
        .find_map(|cause| cause.downcast_ref::<RenderError>())
        .map(RenderError::kind)
    {
        Some(RenderErrorKind::Parse) => language.document_parsing_failed(),
        Some(RenderErrorKind::Load(_) | RenderErrorKind::Render) | None => {
            language.rendering_failed()
        }
    }
}

fn load_error_title(language: Language, kind: LoadErrorKind) -> &'static str {
    match kind {
        LoadErrorKind::UnsupportedScheme | LoadErrorKind::InvalidFileUrl => {
            language.unsupported_address()
        }
        LoadErrorKind::ResourceTooLarge => language.resource_too_large(),
        LoadErrorKind::File => language.file_unavailable(),
        LoadErrorKind::Timeout => language.request_timed_out(),
        LoadErrorKind::Tls => language.certificate_validation_failed(),
        LoadErrorKind::Connection => language.connection_failed(),
        LoadErrorKind::HttpStatus
        | LoadErrorKind::InvalidRedirect
        | LoadErrorKind::TooManyRedirects => language.http_request_failed(),
        LoadErrorKind::Network => language.secure_network_request_failed(),
    }
}

pub fn run() {
    Application::new().with_assets(Assets).run(|cx| {
        gpui_component::init(cx);
        cx.activate(true);
        let options = WindowOptions {
            titlebar: Some(TitleBar::title_bar_options()),
            window_bounds: Some(WindowBounds::centered(size(px(1280.0), px(800.0)), cx)),
            ..WindowOptions::default()
        };
        cx.spawn(async move |cx| {
            cx.open_window(options, |window, cx| {
                let browser = cx.new(|cx| QuickBrowser::new(window, cx));
                browser.update(cx, |browser, cx| {
                    browser.restore_selected_page(window, cx);
                });
                cx.new(|cx| Root::new(browser, window, cx))
            })?;
            if std::env::var_os("ARCHETYPE_STARTUP_PROBE").is_some() {
                println!("ARCHETYPE_READY");
                std::io::stdout().flush()?;
            }
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}

struct QuickBrowser {
    language: Language,
    appearance: AppearancePreference,
    core: BrowserCore,
    runtime: Option<Arc<RuntimeSupervisor>>,
    spaces: Vec<Space>,
    bookmarks: Vec<Bookmark>,
    history_entries: Vec<HistoryEntry>,
    pages: Vec<Page>,
    selected_space: Option<String>,
    selected_page: Option<String>,
    rendered_pages: HashMap<String, RenderedPage>,
    form_inputs: HashMap<FormControlKey, Entity<InputState>>,
    form_subscriptions: HashMap<String, Vec<Subscription>>,
    error: Option<ErrorView>,
    loading_pages: HashSet<String>,
    navigation_tasks: HashMap<String, Task<()>>,
    tab_scroll: ScrollHandle,
    content_scroll: ScrollHandle,
    hibernated_pages: HashSet<String>,
    address_input: Entity<InputState>,
    space_input: Entity<InputState>,
    folder_input: Entity<InputState>,
    history_filter: Entity<InputState>,
    renaming_space: bool,
    creating_bookmark_folder: bool,
    bookmark_folder_parent: Option<String>,
    renaming_bookmark: Option<String>,
    subscriptions: Vec<Subscription>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct FormControlKey {
    page_id: String,
    form_index: usize,
    control_id: ControlId,
}

struct CompletedRender {
    rendered: RenderedPage,
    cookie_jar: Option<CookieJar>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SettingsSection {
    Appearance,
    About,
}

impl SettingsSection {
    const fn url(self) -> &'static str {
        match self {
            Self::Appearance => ARCHETYPE_SETTINGS_APPEARANCE,
            Self::About => ARCHETYPE_SETTINGS_ABOUT,
        }
    }
}

impl QuickBrowser {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let language = Language::system();
        let profile = profile_path();
        let mut core = BrowserCore::open(&profile).unwrap_or_else(|error| {
            logging::profile_fallback(&profile, &error.to_string());
            eprintln!("profile unavailable at {}: {error}", profile.display());
            BrowserCore::in_memory().expect("in-memory profile must initialize")
        });
        let appearance = core.appearance_preference().unwrap_or_default();
        apply_appearance(appearance, window, cx);
        let runtime = start_runtime();
        let mut spaces = core.spaces().unwrap_or_default();
        if spaces.is_empty() {
            if let Ok(space) = core.create_space(language.default_space_name()) {
                spaces.push(space);
            }
        }
        let saved = core.selection().unwrap_or_default();
        let selected_space = saved
            .0
            .filter(|id| spaces.iter().any(|space| &space.id == id))
            .or_else(|| spaces.first().map(|space| space.id.clone()));
        let bookmarks = selected_space
            .as_deref()
            .and_then(|id| core.bookmarks(id, None).ok())
            .unwrap_or_default();
        let history_entries = core.history_entries(HISTORY_PAGE_LIMIT).unwrap_or_default();
        let pages = core.pages().unwrap_or_default();
        let hibernated_pages = pages
            .iter()
            .filter(|page| core.page_hibernation(page).ok().flatten().is_some())
            .map(|page| page.id.clone())
            .collect();
        let selected_page = saved
            .1
            .filter(|id| pages.iter().any(|page| &page.id == id))
            .or_else(|| pages.first().map(|page| page.id.clone()));
        let address = selected_page
            .as_deref()
            .and_then(|id| pages.iter().find(|page| page.id == id))
            .map(|page| page.url.clone())
            .unwrap_or_default();
        let space_name = selected_space
            .as_deref()
            .and_then(|id| spaces.iter().find(|space| space.id == id))
            .map(|space| space.name.clone())
            .unwrap_or_default();

        let address_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(language.address_placeholder()));
        address_input.update(cx, |input, cx| input.set_value(address, window, cx));
        let space_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(language.space_name_placeholder()));
        space_input.update(cx, |input, cx| input.set_value(space_name, window, cx));
        let folder_input = cx
            .new(|cx| InputState::new(window, cx).placeholder(language.folder_name_placeholder()));
        let history_filter =
            cx.new(|cx| InputState::new(window, cx).placeholder(language.search_history()));

        let subscriptions = Self::input_subscriptions(
            window,
            cx,
            &address_input,
            &space_input,
            &folder_input,
            &history_filter,
        );

        Self {
            language,
            appearance,
            core,
            runtime,
            spaces,
            bookmarks,
            history_entries,
            pages,
            selected_space,
            selected_page,
            rendered_pages: HashMap::new(),
            form_inputs: HashMap::new(),
            form_subscriptions: HashMap::new(),
            error: None,
            loading_pages: HashSet::new(),
            navigation_tasks: HashMap::new(),
            tab_scroll: ScrollHandle::new(),
            content_scroll: ScrollHandle::new(),
            hibernated_pages,
            address_input,
            space_input,
            folder_input,
            history_filter,
            renaming_space: false,
            creating_bookmark_folder: false,
            bookmark_folder_parent: None,
            renaming_bookmark: None,
            subscriptions,
        }
    }

    fn input_subscriptions(
        window: &mut Window,
        cx: &mut Context<Self>,
        address_input: &Entity<InputState>,
        space_input: &Entity<InputState>,
        folder_input: &Entity<InputState>,
        history_filter: &Entity<InputState>,
    ) -> Vec<Subscription> {
        vec![
            cx.observe_window_appearance(window, |this, window, cx| {
                if this.appearance == AppearancePreference::System {
                    Theme::sync_system_appearance(Some(window), cx);
                }
            }),
            cx.subscribe_in(address_input, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.navigate_current(window, cx);
                }
            }),
            cx.subscribe_in(space_input, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.rename_selected_space(window, cx);
                }
            }),
            cx.subscribe_in(folder_input, window, |this, _, event, _window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.save_bookmark_editor(cx);
                }
            }),
            cx.subscribe_in(history_filter, window, |_, _, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
        ]
    }

    fn set_address(
        &self,
        value: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.address_input
            .update(cx, |input, cx| input.set_value(value, window, cx));
    }

    fn set_appearance(
        &mut self,
        appearance: AppearancePreference,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.core.set_appearance_preference(appearance) {
            self.error = Some(ErrorView::application(self.language, &error));
            cx.notify();
            return;
        }
        self.appearance = appearance;
        apply_appearance(appearance, window, cx);
        cx.notify();
    }

    fn set_space_name(
        &self,
        value: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.space_input
            .update(cx, |input, cx| input.set_value(value, window, cx));
    }

    fn add_space(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.error = None;
        let name = self.language.new_space_name(self.spaces.len() + 1);
        match self.core.create_space(&name) {
            Ok(space) => {
                self.selected_space = Some(space.id.clone());
                self.spaces.push(space);
                self.bookmarks.clear();
                self.renaming_space = false;
                self.set_space_name(name, window, cx);
                self.persist_selection();
            }
            Err(error) => self.error = Some(ErrorView::application(self.language, &error)),
        }
        cx.notify();
    }

    fn select_space(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.error = None;
        self.creating_bookmark_folder = false;
        self.bookmark_folder_parent = None;
        self.renaming_bookmark = None;
        self.selected_space = Some(id.to_owned());
        let name = self
            .spaces
            .iter()
            .find(|space| space.id == id)
            .map(|space| space.name.clone())
            .unwrap_or_default();
        self.bookmarks = self.core.bookmarks(id, None).unwrap_or_default();
        self.set_space_name(name, window, cx);
        self.persist_selection();
        cx.notify();
    }

    fn rename_selected_space(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.error = None;
        let name = self.space_input.read(cx).value().trim().to_owned();
        if name.is_empty() {
            self.error = Some(ErrorView::input(
                self.language,
                self.language.space_name_empty(),
            ));
            cx.notify();
            return;
        }
        let Some(id) = self.selected_space.clone() else {
            return;
        };
        match self.core.rename_space(&id, &name) {
            Ok(true) => {
                if let Some(space) = self.spaces.iter_mut().find(|space| space.id == id) {
                    space.name.clone_from(&name);
                }
                self.renaming_space = false;
                self.set_space_name(name, window, cx);
            }
            Ok(false) => {
                self.error = Some(ErrorView::input(
                    self.language,
                    self.language.selected_space_missing(),
                ));
            }
            Err(error) => self.error = Some(ErrorView::application(self.language, &error)),
        }
        cx.notify();
    }

    fn begin_rename_space(&mut self, cx: &mut Context<Self>) {
        self.renaming_space = true;
        cx.notify();
    }

    fn cancel_rename_space(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.renaming_space = false;
        let name = self
            .selected_space
            .as_deref()
            .and_then(|id| self.spaces.iter().find(|space| space.id == id))
            .map(|space| space.name.clone())
            .unwrap_or_default();
        self.set_space_name(name, window, cx);
        cx.notify();
    }

    fn delete_selected_space(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.error = None;
        let Some(id) = self.selected_space.clone() else {
            return;
        };
        match self.core.delete_space(&id) {
            Ok(_) => {
                self.spaces.retain(|space| space.id != id);
                self.selected_space = self.spaces.first().map(|space| space.id.clone());
                let name = self
                    .selected_space
                    .as_deref()
                    .and_then(|space_id| self.spaces.iter().find(|space| space.id == space_id))
                    .map(|space| space.name.clone())
                    .unwrap_or_default();
                self.bookmarks = self
                    .selected_space
                    .as_deref()
                    .and_then(|space_id| self.core.bookmarks(space_id, None).ok())
                    .unwrap_or_default();
                self.set_space_name(name, window, cx);
                self.persist_selection();
            }
            Err(error) => self.error = Some(ErrorView::application(self.language, &error)),
        }
        cx.notify();
    }

    fn add_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let url = blank_url();
        match self.core.create_page(&url) {
            Ok(page) => {
                self.selected_page = Some(page.id.clone());
                self.pages.push(page);
                self.scroll_to_selected_tab();
                self.set_address(url.to_string(), window, cx);
                self.persist_selection();
                cx.notify();
            }
            Err(error) => {
                self.error = Some(ErrorView::application(self.language, &error));
                cx.notify();
            }
        }
    }

    fn open_history_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(page_id) = self
            .pages
            .iter()
            .find(|page| is_history_page(page))
            .map(|page| page.id.clone())
        {
            self.select_page(&page_id, window, cx);
            self.refresh_history(cx);
            return;
        }

        let url = history_url();
        match self.core.create_page(&url) {
            Ok(page) => {
                self.selected_page = Some(page.id.clone());
                self.pages.push(page);
                self.scroll_to_selected_tab();
                self.set_address(url.to_string(), window, cx);
                self.persist_selection();
                self.refresh_history(cx);
            }
            Err(error) => {
                self.error = Some(ErrorView::application(self.language, &error));
                cx.notify();
            }
        }
    }

    fn open_settings_page(
        &mut self,
        section: SettingsSection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let url = Url::parse(section.url()).expect("settings route must be valid");
        if let Some(index) = self.pages.iter().position(is_settings_page) {
            let page = self.pages[index].clone();
            match self
                .core
                .update_internal_page(&page, &url, self.language.settings())
            {
                Ok(true) => {
                    self.pages[index].url = url.to_string();
                    self.language
                        .settings()
                        .clone_into(&mut self.pages[index].title);
                    let page_id = self.pages[index].id.clone();
                    self.select_page(&page_id, window, cx);
                }
                Ok(false) => {
                    self.error = Some(ErrorView::input(
                        self.language,
                        self.language.selected_page_missing(),
                    ));
                    cx.notify();
                }
                Err(error) => {
                    self.error = Some(ErrorView::application(self.language, &error));
                    cx.notify();
                }
            }
            return;
        }

        match self.core.create_page(&url) {
            Ok(mut page) => {
                if let Err(error) =
                    self.core
                        .update_internal_page(&page, &url, self.language.settings())
                {
                    self.error = Some(ErrorView::application(self.language, &error));
                    cx.notify();
                    return;
                }
                self.language.settings().clone_into(&mut page.title);
                self.selected_page = Some(page.id.clone());
                self.pages.push(page);
                self.scroll_to_selected_tab();
                self.set_address(url.to_string(), window, cx);
                self.persist_selection();
                cx.notify();
            }
            Err(error) => {
                self.error = Some(ErrorView::application(self.language, &error));
                cx.notify();
            }
        }
    }

    fn refresh_history(&mut self, cx: &mut Context<Self>) {
        match self.core.history_entries(HISTORY_PAGE_LIMIT) {
            Ok(entries) => self.history_entries = entries,
            Err(error) => self.error = Some(ErrorView::application(self.language, &error)),
        }
        cx.notify();
    }

    fn delete_history_entry(&mut self, id: &str, cx: &mut Context<Self>) {
        match self.core.delete_history_entry(id) {
            Ok(true) => self.history_entries.retain(|entry| entry.id != id),
            Ok(false) => {}
            Err(error) => self.error = Some(ErrorView::application(self.language, &error)),
        }
        cx.notify();
    }

    fn clear_history(&mut self, cx: &mut Context<Self>) {
        match self.core.clear_history() {
            Ok(_) => self.history_entries.clear(),
            Err(error) => self.error = Some(ErrorView::application(self.language, &error)),
        }
        cx.notify();
    }

    fn open_url_in_new_page(&mut self, url: &Url, window: &mut Window, cx: &mut Context<Self>) {
        match self.core.create_page(url) {
            Ok(page) => {
                self.selected_page = Some(page.id.clone());
                self.pages.push(page);
                self.scroll_to_selected_tab();
                self.set_address(url.to_string(), window, cx);
                self.persist_selection();
                self.navigate_to(url, window, cx);
            }
            Err(error) => {
                self.error = Some(ErrorView::application(self.language, &error));
                cx.notify();
            }
        }
    }

    fn select_page(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.error = None;
        if should_hibernate_on_switch(self.selected_page.as_deref(), id, self.rendered_pages.len())
        {
            self.hibernate_selected_page();
        }
        self.selected_page = Some(id.to_owned());
        self.scroll_to_selected_tab();
        let page = self.selected_page_record().cloned();
        let address = page
            .as_ref()
            .map(|page| page.url.clone())
            .unwrap_or_default();
        self.set_address(address, window, cx);
        self.persist_selection();
        if let Some(page) = page {
            if self.hibernated_pages.contains(&page.id) {
                self.wake_hibernated_page(&page, window, cx);
            } else if !self.rendered_pages.contains_key(&page.id)
                && !self.loading_pages.contains(&page.id)
                && !is_internal_page(&page)
            {
                self.reload_page(page, window, cx);
            } else {
                cx.notify();
            }
        }
    }

    fn hibernate_selected_page(&mut self) {
        let Some(page) = self.selected_page_record().cloned() else {
            return;
        };
        let Some(rendered) = self.rendered_pages.get(&page.id) else {
            return;
        };
        let scroll_y = f32::from(-self.content_scroll.offset().y).max(0.0);
        if self
            .core
            .hibernate_page(
                &page,
                rendered,
                Viewport {
                    width: 960.0,
                    height: 640.0,
                },
                scroll_y,
                true,
            )
            .is_ok()
        {
            self.rendered_pages.remove(&page.id);
            self.form_inputs.retain(|key, _| key.page_id != page.id);
            self.form_subscriptions.remove(&page.id);
            self.hibernated_pages.insert(page.id);
        }
    }

    fn wake_hibernated_page(&mut self, page: &Page, window: &mut Window, cx: &mut Context<Self>) {
        match self.core.resume_page(page) {
            Ok(snapshot) => {
                self.hibernated_pages.remove(&page.id);
                self.reload_page(page.clone(), window, cx);
                self.content_scroll.set_offset(point(
                    px(0.0),
                    px(-snapshot.map_or(0.0, |item| item.scroll_y)),
                ));
            }
            Err(error) => {
                self.error = Some(ErrorView::navigation(self.language, &error));
                cx.notify();
            }
        }
    }

    fn close_page(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(closing_index) = self.pages.iter().position(|page| page.id == id) else {
            return;
        };
        let page = self.pages[closing_index].clone();
        let closing_selected = self.selected_page.as_deref() == Some(id);
        let adjacent_page = adjacent_page_id_after_close(&self.pages, closing_index);
        if let Err(error) = self.core.close_page(&page) {
            self.error = Some(ErrorView::application(self.language, &error));
            cx.notify();
            return;
        }
        self.navigation_tasks.remove(id);
        self.loading_pages.remove(id);
        self.rendered_pages.remove(id);
        self.form_inputs.retain(|key, _| key.page_id != id);
        self.form_subscriptions.remove(id);
        self.hibernated_pages.remove(id);
        self.pages.retain(|item| item.id != id);
        if closing_selected {
            self.selected_page = adjacent_page;
            self.scroll_to_selected_tab();
            let next_page = self.selected_page_record().cloned();
            let address = next_page
                .as_ref()
                .map(|page| page.url.clone())
                .unwrap_or_default();
            self.set_address(address, window, cx);
            if let Some(page) = next_page
                && !self.rendered_pages.contains_key(&page.id)
                && !self.loading_pages.contains(&page.id)
                && !is_internal_page(&page)
            {
                self.reload_page(page, window, cx);
            }
        }
        self.persist_selection();
        cx.notify();
    }

    fn navigate_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let address = self.address_input.read(cx).value();
        match parse_address(&address, self.language) {
            Ok(url) if self.selected_page_record().is_some_and(is_internal_page) => {
                self.open_url_in_new_page(&url, window, cx);
            }
            Ok(url) => self.navigate_to(&url, window, cx),
            Err(error) => {
                self.error = Some(ErrorView::input(self.language, error));
                cx.notify();
            }
        }
    }

    fn navigate_to(&mut self, url: &Url, window: &mut Window, cx: &mut Context<Self>) {
        self.error = None;
        if self.selected_page_record().is_none() {
            match self.core.create_page(url) {
                Ok(page) => {
                    self.selected_page = Some(page.id.clone());
                    self.pages.push(page);
                    self.persist_selection();
                }
                Err(error) => {
                    self.error = Some(ErrorView::application(self.language, &error));
                    cx.notify();
                    return;
                }
            }
        }
        let page = self
            .selected_page_record()
            .cloned()
            .expect("page was created");
        match self.core.start_navigation(&page, url) {
            Ok(pending) => self.start_render(page, pending, window, cx),
            Err(error) => {
                logging::navigation_failed(&page.id, url.as_str(), &format!("{error:#}"));
                self.error = Some(ErrorView::navigation(self.language, &error));
                cx.notify();
            }
        }
    }

    fn navigate_history(
        &mut self,
        direction: HistoryDirection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.error = None;
        let Some(page) = self.selected_page_record().cloned() else {
            return;
        };
        let result = match direction {
            HistoryDirection::Back => self.core.start_back(&page),
            HistoryDirection::Forward => self.core.start_forward(&page),
            HistoryDirection::Reload => self.core.start_reload(&page),
        };
        match result {
            Ok(pending) => self.start_render(page, pending, window, cx),
            Err(error) => {
                logging::history_navigation_failed(
                    &page.id,
                    direction.as_str(),
                    &format!("{error:#}"),
                );
                self.error = Some(ErrorView::navigation(self.language, &error));
                cx.notify();
            }
        }
    }

    fn restore_selected_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.scroll_to_selected_tab();
        if let Some(page) = self.selected_page_record().cloned() {
            if is_internal_page(&page) {
                cx.notify();
            } else if self.hibernated_pages.contains(&page.id) {
                self.wake_hibernated_page(&page, window, cx);
            } else {
                self.reload_page(page, window, cx);
            }
        }
    }

    fn reload_page(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        match self.core.start_reload(&page) {
            Ok(pending) => self.start_render(page, pending, window, cx),
            Err(error) => {
                self.error = Some(ErrorView::navigation(self.language, &error));
                cx.notify();
            }
        }
    }

    fn start_render(
        &mut self,
        page: Page,
        pending: PendingNavigation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.start_render_request(page, pending, None, window, cx);
    }

    fn start_render_request(
        &mut self,
        page: Page,
        pending: PendingNavigation,
        submission: Option<FormSubmission>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let page_id = page.id.clone();
        let url = pending.url().clone();
        logging::navigation_started(&page_id, url.as_str());
        self.loading_pages.insert(page_id.clone());
        let browser = cx.entity();
        let runtime = self.runtime.clone();
        let runtime_page_id = pending.page_id().clone();
        let navigation_id = pending.navigation_id();
        let top_level_url = pending.top_level_url().clone();
        let cookie_jar = self.core.cookie_jar_snapshot();
        let task = window.spawn(cx, async move |cx| {
            let result = cx
                .background_spawn(async move {
                    let loader = Loader::new()?;
                    if let Some(runtime) = runtime {
                        let mut cookie_jar = cookie_jar;
                        let request = BrokerRequest {
                            page_id: runtime_page_id,
                            navigation_id,
                            url: url.clone(),
                            viewport_width_px: 960,
                            viewport_height_px: 700,
                        };
                        let document = if let Some(submission) = submission.as_ref() {
                            load_form_submission_with_cookies(
                                &loader,
                                &request,
                                submission,
                                &mut cookie_jar,
                                &top_level_url,
                            )?
                        } else {
                            load_static_document_with_cookies(
                                &loader,
                                &request,
                                &mut cookie_jar,
                                &top_level_url,
                            )?
                        };
                        let final_url = Url::parse(document.url.as_str())?;
                        let favicon_png = load_favicon_with_cookies(
                            &loader,
                            &document.html,
                            &final_url,
                            &mut cookie_jar,
                            &top_level_url,
                        );
                        let metadata = arch_browser::render_html(&final_url, &document.html, 960.0);
                        let rendered = runtime
                            .render_document(document)
                            .recv_timeout(Duration::from_secs(6))??;
                        Ok(CompletedRender {
                            rendered: RenderedPage {
                                final_url: Url::parse(rendered.final_url.as_str())?,
                                title: rendered.title,
                                display_list: rendered.display_list,
                                diagnostics: rendered.diagnostics,
                                image_resources: rendered.image_resources,
                                favicon_png,
                                forms: metadata.forms,
                                form_controls: metadata.form_controls,
                            },
                            cookie_jar: Some(cookie_jar),
                        })
                    } else {
                        arch_browser::render_url(&loader, &url, 960.0).map(|rendered| {
                            CompletedRender {
                                rendered,
                                cookie_jar: None,
                            }
                        })
                    }
                })
                .await;
            let _ = cx.update(|window, cx| {
                browser.update(cx, |browser, cx| {
                    browser.finish_render(&page, &pending, result, window, cx);
                });
            });
        });
        self.navigation_tasks.insert(page_id, task);
        cx.notify();
    }

    fn finish_render(
        &mut self,
        page: &Page,
        pending: &PendingNavigation,
        result: anyhow::Result<CompletedRender>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.navigation_tasks.remove(&page.id);
        self.loading_pages.remove(&page.id);
        if !self.core.accepts_navigation(pending) {
            cx.notify();
            return;
        }
        match result {
            Ok(completed) => {
                if let Some(cookie_jar) = completed.cookie_jar
                    && let Err(error) = self.core.commit_cookie_jar_snapshot(cookie_jar)
                {
                    logging::render_diagnostic(
                        Some(&page.id),
                        &format!("could not persist Cookie state: {error:#}"),
                    );
                }
                let rendered = completed.rendered;
                match self.core.finish_navigation(page, pending, &rendered) {
                    Ok(true) => {
                        if let Ok(entries) = self.core.history_entries(HISTORY_PAGE_LIMIT) {
                            self.history_entries = entries;
                        }
                        self.apply_rendered(page, rendered, window, cx);
                    }
                    Ok(false) => cx.notify(),
                    Err(error) => {
                        self.error = Some(ErrorView::application(self.language, &error));
                        cx.notify();
                    }
                }
            }
            Err(error) => {
                logging::navigation_failed(&page.id, pending.url().as_str(), &format!("{error:#}"));
                if self.selected_page.as_deref() == Some(page.id.as_str()) {
                    self.error = Some(ErrorView::navigation(self.language, &error));
                }
                cx.notify();
            }
        }
    }

    fn stop_loading(&mut self, cx: &mut Context<Self>) {
        let Some(page) = self.selected_page_record().cloned() else {
            return;
        };
        if !self.loading_pages.remove(&page.id) {
            return;
        }
        self.navigation_tasks.remove(&page.id);
        if let Err(error) = self.core.stop(&page) {
            self.error = Some(ErrorView::application(self.language, &error));
        }
        cx.notify();
    }

    fn apply_rendered(
        &mut self,
        page: &Page,
        rendered: RenderedPage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        logging::navigation_completed(
            &page.id,
            rendered.final_url.as_str(),
            &rendered.title,
            rendered.display_list.commands.len(),
            rendered.diagnostics.len(),
        );
        for diagnostic in &rendered.diagnostics {
            logging::render_diagnostic(Some(&page.id), diagnostic);
        }
        if self.selected_page.as_deref() == Some(page.id.as_str()) {
            self.set_address(rendered.final_url.to_string(), window, cx);
            self.content_scroll.set_offset(point(px(0.0), px(0.0)));
        }
        if let Some(current) = self.pages.iter_mut().find(|item| item.id == page.id) {
            current.url = rendered.final_url.to_string();
            current.title.clone_from(&rendered.title);
        }
        self.prepare_form_inputs(&page.id, &rendered, window, cx);
        self.rendered_pages.insert(page.id.clone(), rendered);
        cx.notify();
    }

    fn prepare_form_inputs(
        &mut self,
        page_id: &str,
        rendered: &RenderedPage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.form_inputs.retain(|key, _| key.page_id != page_id);
        self.form_subscriptions.remove(page_id);
        let mut subscriptions = Vec::new();
        for (form_index, form) in rendered.forms.iter().enumerate() {
            for control in &form.controls {
                if !matches!(control.kind, ControlKind::Text | ControlKind::Password) {
                    continue;
                }
                let key = FormControlKey {
                    page_id: page_id.to_owned(),
                    form_index,
                    control_id: control.id,
                };
                let masked = control.kind == ControlKind::Password;
                let input = cx.new(|cx| InputState::new(window, cx).masked(masked));
                input.update(cx, |input, cx| {
                    input.set_value(control.value.clone(), window, cx);
                });
                let event_key = key.clone();
                subscriptions.push(cx.subscribe_in(
                    &input,
                    window,
                    move |this, input, event, _window, cx| {
                        if matches!(event, InputEvent::Change) {
                            let value = input.read(cx).value().to_string();
                            this.update_form_text(&event_key, value, cx);
                        }
                    },
                ));
                self.form_inputs.insert(key, input);
            }
        }
        self.form_subscriptions
            .insert(page_id.to_owned(), subscriptions);
    }

    fn update_form_text(&mut self, key: &FormControlKey, value: String, cx: &mut Context<Self>) {
        if let Some(form) = self
            .rendered_pages
            .get_mut(&key.page_id)
            .and_then(|rendered| rendered.forms.get_mut(key.form_index))
        {
            let _ = form.set_text(key.control_id, value);
            cx.notify();
        }
    }

    fn update_form_checked(&mut self, key: &FormControlKey, checked: bool, cx: &mut Context<Self>) {
        if let Some(form) = self
            .rendered_pages
            .get_mut(&key.page_id)
            .and_then(|rendered| rendered.forms.get_mut(key.form_index))
        {
            let _ = form.set_checked(key.control_id, checked);
            cx.notify();
        }
    }

    fn update_form_select(
        &mut self,
        key: &FormControlKey,
        option_index: usize,
        cx: &mut Context<Self>,
    ) {
        if let Some(form) = self
            .rendered_pages
            .get_mut(&key.page_id)
            .and_then(|rendered| rendered.forms.get_mut(key.form_index))
        {
            let _ = form.select(key.control_id, option_index);
            cx.notify();
        }
    }

    fn submit_page_form(
        &mut self,
        key: &FormControlKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.error = None;
        let Some(page) = self
            .pages
            .iter()
            .find(|page| page.id == key.page_id)
            .cloned()
        else {
            return;
        };
        let submission = self
            .rendered_pages
            .get(&key.page_id)
            .and_then(|rendered| rendered.forms.get(key.form_index))
            .and_then(|form| form.submission(Some(key.control_id)).ok());
        let Some(submission) = submission else {
            return;
        };
        match self.core.start_navigation(&page, &submission.target) {
            Ok(pending) => {
                let submission = (submission.method == FormMethod::Post).then_some(submission);
                self.start_render_request(page, pending, submission, window, cx);
            }
            Err(error) => {
                logging::navigation_failed(
                    &page.id,
                    submission.target.as_str(),
                    &format!("{error:#}"),
                );
                self.error = Some(ErrorView::navigation(self.language, &error));
                cx.notify();
            }
        }
    }

    fn selected_page_record(&self) -> Option<&Page> {
        let id = self.selected_page.as_deref()?;
        self.pages.iter().find(|page| page.id == id)
    }

    fn scroll_to_selected_tab(&self) {
        if let Some(index) = self
            .pages
            .iter()
            .position(|page| self.selected_page.as_deref() == Some(page.id.as_str()))
        {
            self.tab_scroll.scroll_to_item(index);
        }
    }

    fn bookmark_current_page(&mut self, parent_id: Option<&str>, cx: &mut Context<Self>) {
        self.error = None;
        let Some(space_id) = self.selected_space.clone() else {
            return;
        };
        let Some(page) = self.selected_page_record().cloned() else {
            return;
        };
        let Ok(url) = Url::parse(&page.url) else {
            return;
        };
        let title = if page.title.is_empty() {
            page.url.clone()
        } else {
            page.title
        };
        match self
            .core
            .create_bookmark(&space_id, parent_id, &title, &url)
        {
            Ok(bookmark) => {
                if parent_id.is_none() {
                    self.bookmarks.push(bookmark);
                }
            }
            Err(error) => self.error = Some(ErrorView::application(self.language, &error)),
        }
        cx.notify();
    }

    fn open_bookmark(&mut self, url: &str, window: &mut Window, cx: &mut Context<Self>) {
        match Url::parse(url) {
            Ok(url) => self.navigate_to(&url, window, cx),
            Err(error) => {
                self.error = Some(ErrorView::input(
                    self.language,
                    self.language.invalid_url(error),
                ));
                cx.notify();
            }
        }
    }

    fn delete_bookmark(&mut self, id: &str, cx: &mut Context<Self>) {
        match self.core.delete_bookmark(id) {
            Ok(true) => self.bookmarks.retain(|bookmark| bookmark.id != id),
            Ok(false) => {}
            Err(error) => self.error = Some(ErrorView::application(self.language, &error)),
        }
        cx.notify();
    }

    fn begin_rename_bookmark(
        &mut self,
        id: String,
        title: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.creating_bookmark_folder = false;
        self.bookmark_folder_parent = None;
        self.renaming_bookmark = Some(id);
        self.folder_input.update(cx, |input, cx| {
            input.set_value(title, window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    fn rename_bookmark(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        let title = self.folder_input.read(cx).value().trim().to_owned();
        if title.is_empty() {
            self.error = Some(ErrorView::input(
                self.language,
                self.language.bookmark_name_empty(),
            ));
            cx.notify();
            return;
        }
        let Some(id) = self.renaming_bookmark.clone() else {
            return;
        };
        match self.core.rename_bookmark(&id, &title) {
            Ok(true) => {
                if let Some(bookmark) = self.bookmarks.iter_mut().find(|item| item.id == id) {
                    bookmark.title.clone_from(&title);
                }
                self.renaming_bookmark = None;
            }
            Ok(false) => self.renaming_bookmark = None,
            Err(error) => self.error = Some(ErrorView::application(self.language, &error)),
        }
        cx.notify();
    }

    fn begin_create_bookmark_folder(
        &mut self,
        parent_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let number = self
            .bookmarks
            .iter()
            .filter(|bookmark| bookmark.kind == BookmarkKind::Folder)
            .count()
            + 1;
        let name = self.language.new_folder_name(number);
        self.creating_bookmark_folder = true;
        self.bookmark_folder_parent = parent_id;
        self.renaming_bookmark = None;
        self.folder_input.update(cx, |input, cx| {
            input.set_value(name, window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    fn create_bookmark_folder(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        let title = self.folder_input.read(cx).value().trim().to_owned();
        if title.is_empty() {
            self.error = Some(ErrorView::input(
                self.language,
                self.language.folder_name_empty(),
            ));
            cx.notify();
            return;
        }
        let Some(space_id) = self.selected_space.clone() else {
            return;
        };
        let parent_id = self.bookmark_folder_parent.as_deref();
        match self
            .core
            .create_bookmark_folder(&space_id, parent_id, &title)
        {
            Ok(folder) => {
                if parent_id.is_none() {
                    self.bookmarks.push(folder);
                }
                self.creating_bookmark_folder = false;
                self.bookmark_folder_parent = None;
            }
            Err(error) => self.error = Some(ErrorView::application(self.language, &error)),
        }
        cx.notify();
    }

    fn cancel_create_bookmark_folder(&mut self, cx: &mut Context<Self>) {
        self.creating_bookmark_folder = false;
        self.bookmark_folder_parent = None;
        self.renaming_bookmark = None;
        cx.notify();
    }

    fn save_bookmark_editor(&mut self, cx: &mut Context<Self>) {
        if self.renaming_bookmark.is_some() {
            self.rename_bookmark(cx);
        } else {
            self.create_bookmark_folder(cx);
        }
    }

    fn persist_selection(&mut self) {
        if let Err(error) = self.core.save_selection(
            self.selected_space.as_deref(),
            self.selected_page.as_deref(),
        ) {
            self.error = Some(ErrorView::application(self.language, &error));
        }
    }

    fn space_switcher(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.renaming_space {
            return h_flex()
                .w(px(230.0))
                .gap_1()
                .child(div().flex_1().child(Input::new(&self.space_input).small()))
                .child(
                    Button::new("save-space-name")
                        .ghost()
                        .icon(AppIcon::Rename)
                        .xsmall()
                        .tooltip(self.language.save())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.rename_selected_space(window, cx);
                        })),
                )
                .child(
                    Button::new("cancel-space-name")
                        .ghost()
                        .icon(AppIcon::Close)
                        .xsmall()
                        .tooltip(self.language.cancel())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.cancel_rename_space(window, cx);
                        })),
                )
                .into_any_element();
        }

        let selected_name = self
            .selected_space
            .as_deref()
            .and_then(|id| self.spaces.iter().find(|space| space.id == id))
            .map_or_else(
                || self.language.default_space_name().to_owned(),
                |space| space.name.clone(),
            );
        let spaces = self.spaces.clone();
        let selected_space = self.selected_space.clone();
        let browser = cx.entity();
        let language = self.language;
        let can_delete = self.spaces.len() > 1;

        Button::new("space-switcher")
            .ghost()
            .small()
            .label(selected_name)
            .dropdown_caret(true)
            .tooltip(language.switch_space())
            .dropdown_menu(move |mut menu, _, _| {
                for space in &spaces {
                    let id = space.id.clone();
                    let browser = browser.clone();
                    menu = menu.item(
                        PopupMenuItem::new(space.name.clone())
                            .checked(selected_space.as_deref() == Some(space.id.as_str()))
                            .on_click(move |_, window, cx| {
                                browser.update(cx, |this, cx| {
                                    this.select_space(&id, window, cx);
                                });
                            }),
                    );
                }
                let add_browser = browser.clone();
                let rename_browser = browser.clone();
                let delete_browser = browser.clone();
                menu.separator()
                    .item(
                        PopupMenuItem::new(language.new_space())
                            .icon(Icon::new(AppIcon::Add))
                            .on_click(move |_, window, cx| {
                                add_browser.update(cx, |this, cx| {
                                    this.add_space(window, cx);
                                });
                            }),
                    )
                    .item(
                        PopupMenuItem::new(language.rename_space())
                            .icon(Icon::new(AppIcon::Rename))
                            .disabled(selected_space.is_none())
                            .on_click(move |_, _, cx| {
                                rename_browser.update(cx, |this, cx| {
                                    this.begin_rename_space(cx);
                                });
                            }),
                    )
                    .item(
                        PopupMenuItem::new(language.delete_space())
                            .icon(Icon::new(AppIcon::Delete))
                            .disabled(!can_delete)
                            .on_click(move |_, window, cx| {
                                delete_browser.update(cx, |this, cx| {
                                    this.delete_selected_space(window, cx);
                                });
                            }),
                    )
            })
            .into_any_element()
    }

    fn tab_strip(&self, cx: &mut Context<Self>) -> AnyElement {
        let tabs = self.pages.iter().map(|page| {
            let select_id = page.id.clone();
            let close_id = page.id.clone();
            let active = self.selected_page.as_deref() == Some(page.id.as_str());
            let label = if is_history_page(page) {
                self.language.history().to_owned()
            } else if is_settings_page(page) {
                self.language.settings().to_owned()
            } else if page.title.is_empty() {
                page.url.clone()
            } else {
                page.title.clone()
            };
            let favicon = self
                .rendered_pages
                .get(&page.id)
                .and_then(|rendered| rendered.favicon_png.as_deref())
                .map(image_source);
            let icon_mode = tab_icon_mode(self.loading_pages.contains(&page.id), favicon.is_some());
            h_flex()
                .id(SharedString::from(format!("tab-{}", page.id)))
                .h(px(28.0))
                .min_w(px(72.0))
                .max_w(px(220.0))
                .flex_1()
                .px_2()
                .gap_1()
                .cursor_pointer()
                .overflow_hidden()
                .border_1()
                .border_color(if active {
                    cx.theme().border
                } else {
                    cx.theme().transparent
                })
                .bg(if active {
                    cx.theme().background
                } else {
                    cx.theme().tab_bar
                })
                .rounded(cx.theme().radius)
                .when_some(tab_site_icon(icon_mode, favicon, cx), |tab, icon| {
                    tab.child(icon)
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_sm()
                        .child(label),
                )
                .child(
                    Button::new(SharedString::from(format!("close-tab-{}", page.id)))
                        .ghost()
                        .icon(AppIcon::Close)
                        .xsmall()
                        .tooltip(self.language.close_tab())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.close_page(&close_id, window, cx);
                        })),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.select_page(&select_id, window, cx);
                }))
        });

        h_flex()
            .h_full()
            .flex_1()
            .min_w_0()
            .px_1()
            .gap_1()
            .child(
                h_flex()
                    .id("tab-list")
                    .track_scroll(&self.tab_scroll)
                    .flex_1()
                    .min_w_0()
                    .overflow_x_scroll()
                    .gap_1()
                    .children(tabs)
                    .child(
                        Button::new("new-tab")
                            .ghost()
                            .icon(AppIcon::Add)
                            .xsmall()
                            .tooltip(self.language.new_tab())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.add_page(window, cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    fn bookmark_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        self.bookmarks
            .iter()
            .map(|bookmark| self.bookmark_row(bookmark, cx))
            .collect()
    }

    fn bookmark_row(&self, bookmark: &Bookmark, cx: &mut Context<Self>) -> AnyElement {
        match (&bookmark.kind, &bookmark.url) {
            (BookmarkKind::Bookmark, Some(url)) => self.bookmark_link_row(bookmark, url, cx),
            (BookmarkKind::Folder, None) => self.bookmark_folder_row(bookmark, cx),
            _ => div().into_any_element(),
        }
    }

    fn bookmark_link_row(
        &self,
        bookmark: &Bookmark,
        url: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let target = url.to_owned();
        let browser = cx.entity();
        let context_id = bookmark.id.clone();
        let context_title = bookmark.title.clone();
        let language = self.language;
        h_flex()
            .flex_shrink_0()
            .child(
                Button::new(SharedString::from(format!("bookmark-{}", bookmark.id)))
                    .ghost()
                    .xsmall()
                    .label(bookmark.title.clone())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_bookmark(&target, window, cx);
                    })),
            )
            .context_menu(move |menu, _, _| {
                bookmark_context_menu(menu, &context_id, &context_title, &browser, language)
            })
            .into_any_element()
    }

    fn bookmark_folder_row(&self, bookmark: &Bookmark, cx: &mut Context<Self>) -> AnyElement {
        let browser = cx.entity();
        let menu_browser = browser.clone();
        let language = self.language;
        let folder = bookmark.clone();
        let context_id = bookmark.id.clone();
        let context_title = bookmark.title.clone();
        h_flex()
            .flex_shrink_0()
            .child(
                Button::new(SharedString::from(format!(
                    "bookmark-folder-{}",
                    bookmark.id
                )))
                .ghost()
                .xsmall()
                .icon(IconName::FolderClosed)
                .label(bookmark.title.clone())
                .dropdown_caret(true)
                .tooltip(language.bookmark_folder())
                .dropdown_menu(move |menu, window, cx| {
                    populate_bookmark_folder_menu(
                        menu,
                        &folder,
                        &menu_browser,
                        language,
                        window,
                        cx,
                    )
                }),
            )
            .context_menu(move |menu, _, _| {
                bookmark_context_menu(menu, &context_id, &context_title, &browser, language)
            })
            .into_any_element()
    }

    fn bookmark_folder_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self.creating_bookmark_folder && self.renaming_bookmark.is_none() {
            return Button::new("new-bookmark-folder")
                .ghost()
                .xsmall()
                .icon(IconName::Folder)
                .tooltip(self.language.new_bookmark_folder())
                .disabled(self.selected_space.is_none())
                .on_click(cx.listener(|this, _, window, cx| {
                    this.begin_create_bookmark_folder(None, window, cx);
                }))
                .into_any_element();
        }
        h_flex()
            .w(px(230.0))
            .gap_1()
            .child(
                div()
                    .flex_1()
                    .child(Input::new(&self.folder_input).xsmall()),
            )
            .child(
                Button::new("save-bookmark-folder")
                    .ghost()
                    .xsmall()
                    .icon(AppIcon::Rename)
                    .tooltip(self.language.save())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.save_bookmark_editor(cx);
                    })),
            )
            .child(
                Button::new("cancel-bookmark-folder")
                    .ghost()
                    .xsmall()
                    .icon(AppIcon::Close)
                    .tooltip(self.language.cancel())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.cancel_create_bookmark_folder(cx);
                    })),
            )
            .into_any_element()
    }

    fn bookmark_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let bookmarks = self.bookmark_rows(cx);
        h_flex()
            .id("bookmark-bar")
            .h(px(32.0))
            .w_full()
            .px_3()
            .gap_1()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                h_flex()
                    .id("bookmark-list")
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .overflow_x_scroll()
                    .when(self.bookmarks.is_empty(), |list| {
                        list.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(self.language.bookmarks()),
                        )
                    })
                    .children(bookmarks),
            )
            .child(self.bookmark_folder_editor(cx))
            .into_any_element()
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let current = self.selected_page_record();
        let current_is_internal = current.is_some_and(is_internal_page);
        let loading = current.is_some_and(|page| self.loading_pages.contains(&page.id));
        let can_back =
            !current_is_internal && current.is_some_and(|page| self.core.can_go_back(page));
        let can_forward =
            !current_is_internal && current.is_some_and(|page| self.core.can_go_forward(page));
        h_flex()
            .h(px(54.0))
            .w_full()
            .px_3()
            .gap_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                Button::new("back")
                    .ghost()
                    .icon(AppIcon::Back)
                    .tooltip(self.language.go_back())
                    .disabled(!can_back)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate_history(HistoryDirection::Back, window, cx);
                    })),
            )
            .child(
                Button::new("forward")
                    .ghost()
                    .icon(AppIcon::Forward)
                    .tooltip(self.language.go_forward())
                    .disabled(!can_forward)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate_history(HistoryDirection::Forward, window, cx);
                    })),
            )
            .child(if loading {
                Button::new("stop")
                    .ghost()
                    .icon(AppIcon::Close)
                    .tooltip(self.language.stop_loading())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.stop_loading(cx);
                    }))
            } else {
                Button::new("reload")
                    .ghost()
                    .icon(AppIcon::Refresh)
                    .tooltip(self.language.reload())
                    .disabled(current.is_none() || current_is_internal)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate_history(HistoryDirection::Reload, window, cx);
                    }))
            })
            .child(self.address_control(current.is_some() && !current_is_internal, cx))
            .child(self.appearance_control(cx))
            .child(self.main_menu_control(cx))
            .into_any_element()
    }

    fn address_control(&self, has_page: bool, cx: &mut Context<Self>) -> AnyElement {
        let bookmark_folders = self
            .bookmarks
            .iter()
            .filter(|bookmark| bookmark.kind == BookmarkKind::Folder)
            .cloned()
            .collect::<Vec<_>>();
        let bookmark_browser = cx.entity();
        let language = self.language;
        div()
            .flex_1()
            .rounded_lg()
            .bg(cx.theme().secondary)
            .child(
                Input::new(&self.address_input)
                    .appearance(false)
                    .cleanable(false)
                    .suffix(
                        Button::new("bookmark-current-page")
                            .ghost()
                            .icon(AppIcon::Star)
                            .tooltip(language.bookmark_current_page())
                            .disabled(!has_page || self.selected_space.is_none())
                            .dropdown_menu(move |menu, window, cx| {
                                let root_browser = bookmark_browser.clone();
                                let mut menu = menu.item(
                                    PopupMenuItem::new(language.bookmark_bar())
                                        .icon(Icon::new(AppIcon::Star))
                                        .on_click(move |_, _, cx| {
                                            root_browser.update(cx, |this, cx| {
                                                this.bookmark_current_page(None, cx);
                                            });
                                        }),
                                );
                                for folder in &bookmark_folders {
                                    menu = add_bookmark_destination_menu_item(
                                        menu,
                                        folder,
                                        &bookmark_browser,
                                        language,
                                        window,
                                        cx,
                                    );
                                }
                                menu
                            }),
                    ),
            )
            .into_any_element()
    }

    fn appearance_control(&self, cx: &mut Context<Self>) -> AnyElement {
        let browser = cx.entity();
        let language = self.language;
        Button::new("profile-settings")
            .ghost()
            .icon(IconName::CircleUser)
            .tooltip(language.settings())
            .on_click(move |_, window, cx| {
                browser.update(cx, |this, cx| {
                    this.open_settings_page(SettingsSection::Appearance, window, cx);
                });
            })
            .into_any_element()
    }

    fn main_menu_control(&self, cx: &mut Context<Self>) -> AnyElement {
        let browser = cx.entity();
        let language = self.language;
        Button::new("main-menu")
            .ghost()
            .icon(IconName::EllipsisVertical)
            .tooltip(language.main_menu())
            .dropdown_menu(move |menu, _, _| {
                let settings_browser = browser.clone();
                menu.item(
                    PopupMenuItem::new(language.history())
                        .icon(Icon::new(IconName::BookOpen))
                        .on_click({
                            let browser = browser.clone();
                            move |_, window, cx| {
                                browser.update(cx, |this, cx| {
                                    this.open_history_page(window, cx);
                                });
                            }
                        }),
                )
                .item(
                    PopupMenuItem::new(language.settings())
                        .icon(Icon::new(IconName::Settings))
                        .on_click(move |_, window, cx| {
                            settings_browser.update(cx, |this, cx| {
                                this.open_settings_page(SettingsSection::Appearance, window, cx);
                            });
                        }),
                )
            })
            .into_any_element()
    }

    fn content(&self, cx: &mut Context<Self>) -> AnyElement {
        if let Some(error) = &self.error {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_3()
                .child(Icon::new(AppIcon::Alert))
                .child(div().text_2xl().font_semibold().child(error.title))
                .child(
                    div()
                        .max_w(px(720.0))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(error.detail.clone()),
                )
                .into_any_element();
        }
        if self.selected_page_record().is_some_and(is_history_page) {
            return self.history_content(cx);
        }
        if self.selected_page_record().is_some_and(is_settings_page) {
            return self.settings_content(cx);
        }
        let Some(rendered) = self
            .selected_page
            .as_ref()
            .and_then(|page_id| self.rendered_pages.get(page_id))
        else {
            if self.selected_page_record().is_some_and(is_blank_page) {
                return div().size_full().into_any_element();
            }
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_3()
                .text_color(cx.theme().muted_foreground)
                .child(self.language.open_page_to_begin())
                .into_any_element();
        };

        let mut layers =
            Vec::with_capacity(rendered.display_list.commands.len() + rendered.form_controls.len());
        for command in &rendered.display_list.commands {
            layers.push(Self::display_command(
                command,
                &rendered.image_resources,
                cx,
            ));
        }
        let page_id = self
            .selected_page
            .as_ref()
            .expect("rendered page has a selected page")
            .clone();
        for positioned in &rendered.form_controls {
            if let Some(control) = self.form_control(&page_id, *positioned, cx) {
                layers.push(control);
            }
        }
        let canvas = div()
            .relative()
            .w(px(960.0))
            .h(px(rendered.display_list.content_height.max(1.0)))
            .children(layers);
        let diagnostics = (!rendered.diagnostics.is_empty()).then(|| {
            v_flex()
                .mt_4()
                .p_3()
                .gap_1()
                .border_1()
                .border_color(cx.theme().border)
                .rounded_lg()
                .child(
                    div()
                        .font_semibold()
                        .text_sm()
                        .child(self.language.diagnostics()),
                )
                .children(rendered.diagnostics.iter().cloned().map(|item| {
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(item)
                }))
        });
        v_flex()
            .id("browser-content")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.content_scroll)
            .bg(gpui::white())
            .p_5()
            .child(canvas)
            .when_some(diagnostics, |content, diagnostics| {
                content.child(diagnostics)
            })
            .into_any_element()
    }

    fn history_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.history_filter.read(cx).value().trim().to_owned();
        let entries = self
            .history_entries
            .iter()
            .filter(|entry| history_entry_matches(entry, &query))
            .cloned()
            .collect::<Vec<_>>();
        let has_filtered_entries = !entries.is_empty();
        let has_entries = !self.history_entries.is_empty();
        let empty_message = if has_entries {
            self.language.no_history_matches()
        } else {
            self.language.no_history()
        };
        let browser = cx.entity();
        let rows = entries
            .into_iter()
            .map(|entry| self.history_entry_row(entry, cx))
            .collect::<Vec<_>>();
        let clear_browser = browser.clone();

        v_flex()
            .size_full()
            .items_center()
            .bg(cx.theme().background)
            .child(
                v_flex()
                    .size_full()
                    .max_w(px(960.0))
                    .child(
                        h_flex()
                            .h(px(64.0))
                            .px_4()
                            .justify_between()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(
                                div()
                                    .text_xl()
                                    .font_semibold()
                                    .child(self.language.history()),
                            )
                            .child(
                                Button::new("clear-history")
                                    .ghost()
                                    .icon(AppIcon::Delete)
                                    .label(self.language.clear_history())
                                    .disabled(!has_entries)
                                    .on_click(move |_, _, cx| {
                                        clear_browser.update(cx, |this, cx| {
                                            this.clear_history(cx);
                                        });
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .px_4()
                            .py_3()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(
                                div().rounded_lg().bg(cx.theme().secondary).child(
                                    Input::new(&self.history_filter)
                                        .appearance(false)
                                        .cleanable(true)
                                        .prefix(Icon::new(IconName::Search)),
                                ),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .w_full()
                            .overflow_y_scrollbar()
                            .when(!has_filtered_entries, |list| {
                                list.items_center().justify_center().child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(empty_message),
                                )
                            })
                            .children(rows),
                    ),
            )
            .into_any_element()
    }

    fn history_entry_row(&self, entry: HistoryEntry, cx: &mut Context<Self>) -> AnyElement {
        let open_url = entry.url.clone();
        let delete_id = entry.id.clone();
        let open_browser = cx.entity();
        let delete_browser = open_browser.clone();
        let title = if entry.title.trim().is_empty() {
            entry.url.clone()
        } else {
            entry.title.clone()
        };
        h_flex()
            .id(SharedString::from(format!("history-entry-{}", entry.id)))
            .w_full()
            .min_h(px(64.0))
            .px_3()
            .gap_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .cursor_pointer()
            .hover(|this| this.bg(cx.theme().accent.opacity(0.45)))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(
                        div()
                            .w_full()
                            .truncate()
                            .text_sm()
                            .font_medium()
                            .child(title),
                    )
                    .child(
                        div()
                            .w_full()
                            .truncate()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(entry.url),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format_history_time(entry.visited_at)),
            )
            .child(
                Button::new(SharedString::from(format!(
                    "delete-history-entry-{}",
                    entry.id
                )))
                .ghost()
                .xsmall()
                .icon(AppIcon::Delete)
                .tooltip(self.language.delete_history_entry())
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    delete_browser.update(cx, |this, cx| {
                        this.delete_history_entry(&delete_id, cx);
                    });
                }),
            )
            .on_click(move |_, window, cx| {
                open_browser.update(cx, |this, cx| {
                    if let Ok(url) = Url::parse(&open_url) {
                        this.open_url_in_new_page(&url, window, cx);
                    }
                });
            })
            .into_any_element()
    }

    fn settings_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let section = self
            .selected_page_record()
            .and_then(settings_section)
            .unwrap_or(SettingsSection::Appearance);
        let content = match section {
            SettingsSection::Appearance => self.appearance_settings(cx),
            SettingsSection::About => self.about_settings(cx),
        };
        h_flex()
            .size_full()
            .items_start()
            .bg(cx.theme().background)
            .child(self.settings_navigation(section, cx))
            .child(
                v_flex()
                    .flex_1()
                    .h_full()
                    .overflow_y_scrollbar()
                    .p_8()
                    .items_center()
                    .child(content),
            )
            .into_any_element()
    }

    fn settings_navigation(&self, section: SettingsSection, cx: &mut Context<Self>) -> AnyElement {
        let browser = cx.entity();
        let appearance_browser = browser.clone();
        v_flex()
            .h_full()
            .w(px(220.0))
            .flex_shrink_0()
            .p_3()
            .gap_1()
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().sidebar)
            .child(
                div()
                    .h(px(44.0))
                    .px_2()
                    .flex()
                    .items_center()
                    .text_lg()
                    .font_semibold()
                    .child(self.language.settings()),
            )
            .child(
                Button::new("settings-appearance")
                    .ghost()
                    .w_full()
                    .icon(IconName::Palette)
                    .label(self.language.appearance())
                    .when(section == SettingsSection::Appearance, |button| {
                        button.bg(cx.theme().accent)
                    })
                    .on_click(move |_, window, cx| {
                        appearance_browser.update(cx, |this, cx| {
                            this.open_settings_page(SettingsSection::Appearance, window, cx);
                        });
                    }),
            )
            .child(
                Button::new("settings-about")
                    .ghost()
                    .w_full()
                    .icon(IconName::Info)
                    .label(self.language.about_archetype())
                    .when(section == SettingsSection::About, |button| {
                        button.bg(cx.theme().accent)
                    })
                    .on_click(move |_, window, cx| {
                        browser.update(cx, |this, cx| {
                            this.open_settings_page(SettingsSection::About, window, cx);
                        });
                    }),
            )
            .into_any_element()
    }

    fn appearance_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .w_full()
            .max_w(px(720.0))
            .gap_5()
            .child(
                div()
                    .text_xl()
                    .font_semibold()
                    .child(self.language.appearance()),
            )
            .child(
                v_flex()
                    .gap_4()
                    .child(self.appearance_radio(
                        "appearance-system",
                        self.language.system_appearance(),
                        AppearancePreference::System,
                        cx,
                    ))
                    .child(self.appearance_radio(
                        "appearance-light",
                        self.language.light_appearance(),
                        AppearancePreference::Light,
                        cx,
                    ))
                    .child(self.appearance_radio(
                        "appearance-dark",
                        self.language.dark_appearance(),
                        AppearancePreference::Dark,
                        cx,
                    )),
            )
            .into_any_element()
    }

    fn appearance_radio(
        &self,
        id: &'static str,
        label: &'static str,
        preference: AppearancePreference,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let browser = cx.entity();
        Radio::new(id)
            .label(label)
            .checked(self.appearance == preference)
            .on_click(move |checked, window, cx| {
                if *checked {
                    browser.update(cx, |this, cx| {
                        this.set_appearance(preference, window, cx);
                    });
                }
            })
            .into_any_element()
    }

    fn about_settings(&self, _: &mut Context<Self>) -> AnyElement {
        v_flex()
            .w_full()
            .max_w(px(720.0))
            .gap_5()
            .child(
                div()
                    .text_xl()
                    .font_semibold()
                    .child(self.language.about_archetype()),
            )
            .child(div().text_2xl().font_semibold().child("Archetype"))
            .child(
                h_flex()
                    .gap_2()
                    .text_sm()
                    .child(self.language.version())
                    .child(env!("CARGO_PKG_VERSION")),
            )
            .into_any_element()
    }

    fn display_command(
        command: &DisplayCommand,
        image_resources: &HashMap<String, Vec<u8>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (bounds, clip) = match command {
            DisplayCommand::Box { bounds, clip, .. }
            | DisplayCommand::Text { bounds, clip, .. }
            | DisplayCommand::Image { bounds, clip, .. } => (*bounds, *clip),
        };
        let (x, y) = relative_position(bounds, clip);
        let element = match command {
            DisplayCommand::Box { .. } => Self::box_display_command(command, bounds, x, y),
            DisplayCommand::Text {
                content,
                size_px,
                font_family,
                link,
                color,
                line_height_px,
                font_weight,
                font_style,
                text_align,
                text_decoration,
                ..
            } => {
                let text = div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(bounds.width))
                    .h(px(bounds.height))
                    .text_size(px(*size_px))
                    .when_some(font_family.clone(), Styled::font_family)
                    .line_height(px(*line_height_px))
                    .when_some(*color, |text, color| text.text_color(gpui_color(color)))
                    .when(*font_weight == PageFontWeight::Bold, |text| {
                        text.font_weight(FontWeight::BOLD)
                    })
                    .when(*font_style == PageFontStyle::Italic, Styled::italic)
                    .when(*text_align == TextAlign::Center, Styled::text_center)
                    .when(*text_align == TextAlign::End, Styled::text_right)
                    .when(
                        *text_decoration == TextDecoration::Underline,
                        Styled::underline,
                    )
                    .when(
                        *text_decoration == TextDecoration::LineThrough,
                        Styled::line_through,
                    )
                    .child(content.clone());
                if let Some(target) = link.clone() {
                    text.id(SharedString::from(format!(
                        "link-{}-{}-{target}",
                        bounds.x, bounds.y
                    )))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.set_address(target.clone(), window, cx);
                        if let Ok(url) = Url::parse(&target) {
                            this.navigate_to(&url, window, cx);
                        }
                    }))
                    .into_any_element()
                } else {
                    text.into_any_element()
                }
            }
            DisplayCommand::Image {
                source,
                alt,
                loaded,
                opacity,
                ..
            } => image_element(
                bounds,
                x,
                y,
                source,
                alt,
                *loaded,
                *opacity,
                image_resources,
                cx,
            ),
        };
        clipped_element(element, clip)
    }

    fn box_display_command(
        command: &DisplayCommand,
        bounds: arch_layout::Rect,
        x: f32,
        y: f32,
    ) -> AnyElement {
        let DisplayCommand::Box {
            background,
            border,
            border_width_px,
            border_radius_px,
            shadow,
            ..
        } = command
        else {
            unreachable!("box display helper received a non-box command")
        };
        div()
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(bounds.width))
            .h(px(bounds.height))
            .rounded(px(*border_radius_px))
            .when_some(*background, |layer, color| layer.bg(gpui_color(color)))
            .when_some(*shadow, |layer, shadow| {
                layer.shadow(vec![BoxShadow {
                    offset: point(px(shadow.offset_x_px), px(shadow.offset_y_px)),
                    blur_radius: px(shadow.blur_px),
                    spread_radius: px(0.0),
                    color: gpui_color(shadow.color),
                }])
            })
            .when(*border_width_px > 0.0, |layer| {
                layer
                    .border(px(*border_width_px))
                    .border_color(border.map_or_else(gpui::transparent_black, gpui_color))
            })
            .into_any_element()
    }

    fn form_control(
        &self,
        page_id: &str,
        positioned: arch_browser::PositionedFormControl,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let key = FormControlKey {
            page_id: page_id.to_owned(),
            form_index: positioned.form_index,
            control_id: positioned.control_id,
        };
        let control = self
            .rendered_pages
            .get(page_id)?
            .forms
            .get(positioned.form_index)?
            .controls
            .iter()
            .find(|control| control.id == positioned.control_id)?
            .clone();
        let element = match control.kind {
            ControlKind::Text | ControlKind::Password => {
                let input = self.form_inputs.get(&key)?;
                Input::new(input).small().into_any_element()
            }
            ControlKind::Checkbox => {
                let browser = cx.entity();
                let event_key = key.clone();
                Checkbox::new(SharedString::from(form_control_element_id(&key)))
                    .checked(control.checked)
                    .on_click(move |checked, _, cx| {
                        browser.update(cx, |this, cx| {
                            this.update_form_checked(&event_key, *checked, cx);
                        });
                    })
                    .into_any_element()
            }
            ControlKind::Radio => {
                let browser = cx.entity();
                let event_key = key.clone();
                Radio::new(SharedString::from(form_control_element_id(&key)))
                    .checked(control.checked)
                    .on_click(move |_, _, cx| {
                        browser.update(cx, |this, cx| {
                            this.update_form_checked(&event_key, true, cx);
                        });
                    })
                    .into_any_element()
            }
            ControlKind::Select => {
                let browser = cx.entity();
                let selected_index = control.selected_index;
                let label = selected_index
                    .and_then(|index| control.options.get(index))
                    .map_or_else(String::new, |option| option.label.clone());
                let options = control.options.clone();
                let event_key = key.clone();
                Button::new(SharedString::from(form_control_element_id(&key)))
                    .small()
                    .label(label)
                    .dropdown_caret(true)
                    .dropdown_menu(move |mut menu, _, _| {
                        for (option_index, option) in options.iter().enumerate() {
                            let browser = browser.clone();
                            let event_key = event_key.clone();
                            menu = menu.item(
                                PopupMenuItem::new(option.label.clone())
                                    .checked(selected_index == Some(option_index))
                                    .on_click(move |_, _, cx| {
                                        browser.update(cx, |this, cx| {
                                            this.update_form_select(&event_key, option_index, cx);
                                        });
                                    }),
                            );
                        }
                        menu
                    })
                    .into_any_element()
            }
            ControlKind::Submit => {
                let event_key = key.clone();
                Button::new(SharedString::from(form_control_element_id(&key)))
                    .small()
                    .label(control.value)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.submit_page_form(&event_key, window, cx);
                    }))
                    .into_any_element()
            }
            ControlKind::Button => Button::new(SharedString::from(form_control_element_id(&key)))
                .small()
                .label(control.value)
                .into_any_element(),
        };
        let (x, y) = relative_position(positioned.bounds, positioned.clip);
        let positioned_element = div()
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(positioned.bounds.width))
            .h(px(positioned.bounds.height))
            .overflow_hidden()
            .child(element)
            .into_any_element();
        Some(clipped_element(positioned_element, positioned.clip))
    }
}

fn form_control_element_id(key: &FormControlKey) -> String {
    format!(
        "form-control-{}-{}-{}",
        key.page_id, key.form_index, key.control_id.0
    )
}

#[allow(clippy::too_many_arguments)]
fn image_element(
    bounds: arch_layout::Rect,
    x: f32,
    y: f32,
    source: &str,
    alt: &str,
    loaded: bool,
    opacity: f32,
    image_resources: &HashMap<String, Vec<u8>>,
    cx: &mut Context<QuickBrowser>,
) -> AnyElement {
    if loaded && let Some(bytes) = image_resources.get(source) {
        return img(image_source(bytes))
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(bounds.width))
            .h(px(bounds.height))
            .opacity(opacity)
            .into_any_element();
    }
    div()
        .absolute()
        .left(px(x))
        .top(px(y))
        .w(px(bounds.width))
        .h(px(bounds.height))
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(alt.to_owned())
        .into_any_element()
}

fn clipped_element(element: AnyElement, clip: Option<arch_layout::Rect>) -> AnyElement {
    if let Some(clip) = clip {
        div()
            .absolute()
            .left(px(clip.x))
            .top(px(clip.y))
            .w(px(clip.width))
            .h(px(clip.height))
            .overflow_hidden()
            .child(element)
            .into_any_element()
    } else {
        element
    }
}

fn relative_position(bounds: arch_layout::Rect, clip: Option<arch_layout::Rect>) -> (f32, f32) {
    clip.map_or((bounds.x, bounds.y), |clip| {
        (bounds.x - clip.x, bounds.y - clip.y)
    })
}

impl Render for QuickBrowser {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _ = &self.subscriptions;
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(
                TitleBar::new().child(
                    h_flex()
                        .size_full()
                        .min_w_0()
                        .gap_1()
                        .child(self.space_switcher(cx))
                        .child(self.tab_strip(cx)),
                ),
            )
            .child(self.toolbar(cx))
            .child(self.bookmark_bar(cx))
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .child(self.content(cx)),
            )
    }
}

#[derive(Clone, Copy)]
enum HistoryDirection {
    Back,
    Forward,
    Reload,
}

impl HistoryDirection {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Back => "back",
            Self::Forward => "forward",
            Self::Reload => "reload",
        }
    }
}

fn profile_path() -> PathBuf {
    let base = logging::data_dir();
    let _ = std::fs::create_dir_all(&base);
    base.join("profile.db")
}

fn apply_appearance(appearance: AppearancePreference, window: &mut Window, cx: &mut gpui::App) {
    match appearance {
        AppearancePreference::System => Theme::sync_system_appearance(Some(window), cx),
        AppearancePreference::Light => Theme::change(ThemeMode::Light, Some(window), cx),
        AppearancePreference::Dark => Theme::change(ThemeMode::Dark, Some(window), cx),
    }
}

fn start_runtime() -> Option<Arc<RuntimeSupervisor>> {
    let executable = std::env::var_os("ARCHETYPE_RUNTIME_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|path| path.parent().map(|parent| parent.join("archetype-runtime")))
        })?;
    if !executable.is_file() {
        eprintln!(
            "renderer runtime unavailable at {}; using development in-process fallback",
            executable.display()
        );
        return None;
    }
    let (runtime, ready) = RuntimeSupervisor::spawn(&executable).ok()?;
    match ready.recv_timeout(Duration::from_secs(6)) {
        Ok(Ok(())) => Some(Arc::new(runtime)),
        Ok(Err(error)) => {
            eprintln!("renderer runtime unavailable: {error}");
            None
        }
        Err(error) => {
            eprintln!("renderer runtime readiness failed: {error}");
            None
        }
    }
}

fn blank_url() -> Url {
    Url::parse(ABOUT_BLANK).expect("about:blank must be a valid URL")
}

fn history_url() -> Url {
    Url::parse(ARCHETYPE_HISTORY).expect("history URL must be valid")
}

fn is_blank_page(page: &Page) -> bool {
    page.url == ABOUT_BLANK
}

fn is_history_page(page: &Page) -> bool {
    page.url == ARCHETYPE_HISTORY
}

fn is_settings_page(page: &Page) -> bool {
    settings_section(page).is_some()
}

fn settings_section(page: &Page) -> Option<SettingsSection> {
    match page.url.as_str() {
        ARCHETYPE_SETTINGS_APPEARANCE => Some(SettingsSection::Appearance),
        ARCHETYPE_SETTINGS_ABOUT => Some(SettingsSection::About),
        _ => None,
    }
}

fn is_internal_page(page: &Page) -> bool {
    is_blank_page(page) || page.url.starts_with("archetype://")
}

fn history_entry_matches(entry: &HistoryEntry, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || entry.title.to_lowercase().contains(&query)
        || entry.url.to_lowercase().contains(&query)
}

fn format_history_time(visited_at: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(visited_at).map_or_else(
        || visited_at.to_string(),
        |timestamp| {
            timestamp
                .with_timezone(&Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        },
    )
}

fn tab_icon_mode(is_loading: bool, has_favicon: bool) -> TabIconMode {
    match (is_loading, has_favicon) {
        (true, _) => TabIconMode::Loading,
        (false, true) => TabIconMode::Favicon,
        (false, false) => TabIconMode::Default,
    }
}

fn tab_site_icon(
    mode: TabIconMode,
    favicon: Option<gpui::ImageSource>,
    cx: &mut Context<QuickBrowser>,
) -> Option<AnyElement> {
    let has_favicon = favicon.is_some();
    match mode {
        TabIconMode::Default => Some(
            div()
                .flex_none()
                .size(px(16.0))
                .child(
                    Icon::new(IconName::Globe)
                        .with_size(px(16.0))
                        .text_color(cx.theme().muted_foreground),
                )
                .into_any_element(),
        ),
        TabIconMode::Favicon => favicon.map(|favicon| {
            div()
                .flex_none()
                .size(px(16.0))
                .child(img(favicon).size(px(16.0)))
                .into_any_element()
        }),
        TabIconMode::Loading => Some(
            div()
                .relative()
                .flex_none()
                .size(px(16.0))
                .child(
                    div().absolute().top_0().left_0().size(px(16.0)).child(
                        Spinner::new()
                            .with_size(px(16.0))
                            .color(cx.theme().progress_bar),
                    ),
                )
                .when_some(favicon, |this, favicon| {
                    this.child(
                        img(favicon)
                            .absolute()
                            .top(px(2.0))
                            .left(px(2.0))
                            .size(px(12.0)),
                    )
                })
                .when(!has_favicon, |this| {
                    this.child(
                        div()
                            .absolute()
                            .top(px(2.0))
                            .left(px(2.0))
                            .size(px(12.0))
                            .child(
                                Icon::new(IconName::Globe)
                                    .with_size(px(12.0))
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    )
                })
                .into_any_element(),
        ),
    }
}

fn should_hibernate_on_switch(
    selected_page: Option<&str>,
    next_page: &str,
    resident_pages: usize,
) -> bool {
    selected_page.is_some_and(|selected| selected != next_page)
        && resident_pages >= MAX_RESIDENT_RENDERED_PAGES
}

fn add_bookmark_menu_item(
    menu: PopupMenu,
    bookmark: &Bookmark,
    browser: &Entity<QuickBrowser>,
    language: Language,
    window: &mut Window,
    cx: &mut gpui::App,
) -> PopupMenu {
    match (&bookmark.kind, &bookmark.url) {
        (BookmarkKind::Bookmark, Some(url)) => {
            let target = url.clone();
            let browser = browser.clone();
            menu.item(
                PopupMenuItem::new(bookmark.title.clone())
                    .icon(Icon::new(AppIcon::Star))
                    .on_click(move |_, window, cx| {
                        browser.update(cx, |this, cx| {
                            this.open_bookmark(&target, window, cx);
                        });
                    }),
            )
        }
        (BookmarkKind::Folder, None) => {
            let folder = bookmark.clone();
            let browser = browser.clone();
            let submenu = PopupMenu::build(window, cx, move |menu, window, cx| {
                populate_bookmark_folder_menu(menu, &folder, &browser, language, window, cx)
            });
            menu.item(
                PopupMenuItem::submenu(bookmark.title.clone(), submenu)
                    .icon(Icon::new(IconName::FolderClosed)),
            )
        }
        _ => menu,
    }
}

fn bookmark_context_menu(
    menu: PopupMenu,
    id: &str,
    title: &str,
    browser: &Entity<QuickBrowser>,
    language: Language,
) -> PopupMenu {
    let rename_id = id.to_owned();
    let rename_title = title.to_owned();
    let rename_browser = browser.clone();
    let delete_id = id.to_owned();
    let delete_browser = browser.clone();
    menu.item(
        PopupMenuItem::new(language.rename_bookmark())
            .icon(Icon::new(AppIcon::Rename))
            .on_click(move |_, window, cx| {
                rename_browser.update(cx, |this, cx| {
                    this.begin_rename_bookmark(rename_id.clone(), rename_title.clone(), window, cx);
                });
            }),
    )
    .item(
        PopupMenuItem::new(language.remove_bookmark())
            .icon(Icon::new(AppIcon::Delete))
            .on_click(move |_, _, cx| {
                delete_browser.update(cx, |this, cx| {
                    this.delete_bookmark(&delete_id, cx);
                });
            }),
    )
}

fn populate_bookmark_folder_menu(
    menu: PopupMenu,
    folder: &Bookmark,
    browser: &Entity<QuickBrowser>,
    language: Language,
    window: &mut Window,
    cx: &mut gpui::App,
) -> PopupMenu {
    let children = browser
        .read(cx)
        .core
        .bookmarks(&folder.space_id, Some(&folder.id))
        .unwrap_or_default();
    let folder_id = folder.id.clone();
    let folder_browser = browser.clone();
    let mut menu = bookmark_context_menu(menu, &folder.id, &folder.title, browser, language)
        .item(PopupMenuItem::separator())
        .item(
            PopupMenuItem::new(language.new_bookmark_folder())
                .icon(Icon::new(IconName::Folder))
                .on_click(move |_, window, cx| {
                    folder_browser.update(cx, |this, cx| {
                        this.begin_create_bookmark_folder(Some(folder_id.clone()), window, cx);
                    });
                }),
        );
    if children.is_empty() {
        return menu.item(PopupMenuItem::label(language.empty_folder()));
    }
    menu = menu.item(PopupMenuItem::separator());
    for child in &children {
        menu = add_bookmark_menu_item(menu, child, browser, language, window, cx);
    }
    menu
}

fn add_bookmark_destination_menu_item(
    menu: PopupMenu,
    folder: &Bookmark,
    browser: &Entity<QuickBrowser>,
    language: Language,
    window: &mut Window,
    cx: &mut gpui::App,
) -> PopupMenu {
    let child_folders = browser
        .read(cx)
        .core
        .bookmarks(&folder.space_id, Some(&folder.id))
        .unwrap_or_default()
        .into_iter()
        .filter(|bookmark| bookmark.kind == BookmarkKind::Folder)
        .collect::<Vec<_>>();
    let folder_id = folder.id.clone();
    let folder_browser = browser.clone();
    let browser = browser.clone();
    let submenu = PopupMenu::build(window, cx, move |menu, window, cx| {
        let mut menu = menu.item(
            PopupMenuItem::new(language.save_to_this_folder())
                .icon(Icon::new(AppIcon::Star))
                .on_click(move |_, _, cx| {
                    folder_browser.update(cx, |this, cx| {
                        this.bookmark_current_page(Some(&folder_id), cx);
                    });
                }),
        );
        if !child_folders.is_empty() {
            menu = menu.item(PopupMenuItem::separator());
        }
        for child in &child_folders {
            menu = add_bookmark_destination_menu_item(menu, child, &browser, language, window, cx);
        }
        menu
    });
    menu.item(
        PopupMenuItem::submenu(folder.title.clone(), submenu)
            .icon(Icon::new(IconName::FolderClosed)),
    )
}

fn parse_address(address: &str, language: Language) -> Result<Url, String> {
    let address = address.trim();
    if address.is_empty() {
        return Err(language.address_empty().to_owned());
    }
    if address.contains("://") {
        return Url::parse(address).map_err(|error| language.invalid_url(error));
    }
    if let Ok(path) = PathBuf::from(address).canonicalize() {
        return Url::from_file_path(path).map_err(|()| language.invalid_file_path().to_owned());
    }
    if looks_like_host(address) {
        return Url::parse(&format!("https://{address}"))
            .map_err(|error| language.invalid_url(error));
    }
    if let Ok(url) = Url::parse(address) {
        return Ok(url);
    }
    Err(language.invalid_address(address))
}

fn looks_like_host(address: &str) -> bool {
    !address.chars().any(char::is_whitespace)
        && !address.starts_with('.')
        && !address.starts_with('/')
        && (address == "localhost" || address.starts_with("localhost:") || address.contains('.'))
}

fn adjacent_page_id_after_close(pages: &[Page], closing_index: usize) -> Option<String> {
    pages
        .get(closing_index + 1)
        .or_else(|| {
            closing_index
                .checked_sub(1)
                .and_then(|index| pages.get(index))
        })
        .map(|page| page.id.clone())
}

fn gpui_color(color: PaintColor) -> gpui::Hsla {
    rgba(
        (u32::from(color.red) << 24)
            | (u32::from(color.green) << 16)
            | (u32::from(color.blue) << 8)
            | u32::from(color.alpha),
    )
    .into()
}

fn image_source(bytes: &[u8]) -> gpui::ImageSource {
    let format = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        gpui::ImageFormat::Png
    } else if std::str::from_utf8(bytes).ok().is_some_and(|source| {
        let source = source.trim_start();
        source.starts_with("<svg") || source.starts_with("<?xml") && source.contains("<svg")
    }) {
        gpui::ImageFormat::Svg
    } else {
        gpui::ImageFormat::Jpeg
    };
    Arc::new(gpui::Image::from_bytes(format, bytes.to_vec())).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_errors_have_specific_localized_titles() {
        assert_eq!(
            load_error_title(Language::English, LoadErrorKind::Tls),
            "Certificate validation failed"
        );
        assert_eq!(
            load_error_title(Language::Chinese, LoadErrorKind::Tls),
            "证书验证失败"
        );

        let invalid = std::hint::black_box([0xff]);
        let parse_error = anyhow::Error::new(RenderError::Parse {
            url: Url::parse("file:///invalid.html").unwrap(),
            source: str::from_utf8(&invalid).unwrap_err(),
        });
        assert_eq!(
            navigation_error_title(Language::English, &parse_error),
            "Document parsing failed"
        );

        let render_error = anyhow::Error::new(RenderError::Render {
            url: Url::parse("file:///invalid.html").unwrap(),
        });
        assert_eq!(
            navigation_error_title(Language::Chinese, &render_error),
            "渲染失败"
        );
    }

    #[test]
    fn clipping_helpers_preserve_global_and_relative_positions() {
        let bounds = arch_layout::Rect {
            x: 40.0,
            y: 30.0,
            width: 80.0,
            height: 20.0,
        };
        let clip = arch_layout::Rect {
            x: 10.0,
            y: 5.0,
            width: 60.0,
            height: 15.0,
        };
        let global = relative_position(bounds, None);
        assert!((global.0 - 40.0).abs() < f32::EPSILON);
        assert!((global.1 - 30.0).abs() < f32::EPSILON);
        let relative = relative_position(bounds, Some(clip));
        assert!((relative.0 - 30.0).abs() < f32::EPSILON);
        assert!((relative.1 - 25.0).abs() < f32::EPSILON);

        let _unclipped = clipped_element(div().into_any_element(), None);
        let _clipped = clipped_element(div().into_any_element(), Some(clip));
    }

    #[test]
    fn address_parser_adds_https_to_hostnames() {
        assert_eq!(
            parse_address("baidu.com", Language::English)
                .unwrap()
                .as_str(),
            "https://baidu.com/"
        );
        assert_eq!(
            parse_address("localhost:8080", Language::English)
                .unwrap()
                .as_str(),
            "https://localhost:8080/"
        );
    }

    #[test]
    fn address_parser_preserves_explicit_urls() {
        assert_eq!(
            parse_address(" http://example.com/docs ", Language::English)
                .unwrap()
                .as_str(),
            "http://example.com/docs"
        );
    }

    #[test]
    fn blank_page_is_explicit_and_does_not_require_loading() {
        let mut page = test_page("blank");
        page.url = blank_url().to_string();

        assert!(is_blank_page(&page));
        assert_eq!(page.url, ABOUT_BLANK);
    }

    #[test]
    fn history_page_is_internal_and_filterable() {
        let mut page = test_page("history");
        page.url = history_url().to_string();
        let entry = HistoryEntry {
            id: "visit".to_owned(),
            url: "https://example.com/docs".to_owned(),
            title: "Rust documentation".to_owned(),
            visited_at: 0,
        };

        assert!(is_history_page(&page));
        assert!(is_internal_page(&page));
        assert!(history_entry_matches(&entry, "rust"));
        assert!(history_entry_matches(&entry, "EXAMPLE.COM"));
        assert!(history_entry_matches(&entry, ""));
        assert!(!history_entry_matches(&entry, "browser"));
    }

    #[test]
    fn settings_routes_map_to_trusted_internal_sections() {
        let mut page = test_page("settings");
        page.url = ARCHETYPE_SETTINGS_APPEARANCE.to_owned();
        assert_eq!(settings_section(&page), Some(SettingsSection::Appearance));
        assert!(is_settings_page(&page));
        assert!(is_internal_page(&page));

        page.url = ARCHETYPE_SETTINGS_ABOUT.to_owned();
        assert_eq!(settings_section(&page), Some(SettingsSection::About));

        page.url = "archetype://settings/unknown".to_owned();
        assert_eq!(settings_section(&page), None);
        assert!(is_internal_page(&page));
    }

    #[test]
    fn two_tab_switch_keeps_rendered_pages_resident() {
        assert!(!should_hibernate_on_switch(Some("first"), "second", 2));
        assert!(!should_hibernate_on_switch(Some("first"), "first", 8));
        assert!(should_hibernate_on_switch(Some("first"), "ninth", 8));
    }

    #[test]
    fn tab_icon_mode_combines_loading_and_favicon_state() {
        assert_eq!(tab_icon_mode(false, false), TabIconMode::Default);
        assert_eq!(tab_icon_mode(false, true), TabIconMode::Favicon);
        assert_eq!(tab_icon_mode(true, false), TabIconMode::Loading);
        assert_eq!(tab_icon_mode(true, true), TabIconMode::Loading);
    }

    #[test]
    fn address_parser_prefers_existing_local_files() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pages/01-document/index.html")
            .canonicalize()
            .unwrap();
        let parsed = parse_address(path.to_str().unwrap(), Language::English).unwrap();
        assert_eq!(parsed.to_file_path().unwrap(), path);
    }

    #[test]
    fn address_parser_rejects_empty_input() {
        assert_eq!(
            parse_address("  ", Language::English).unwrap_err(),
            "Address cannot be empty"
        );
        assert_eq!(
            parse_address("  ", Language::Chinese).unwrap_err(),
            "地址不能为空"
        );
    }

    #[test]
    fn closing_selected_tab_prefers_right_then_left_neighbor() {
        let pages = [test_page("a"), test_page("b"), test_page("c")];
        assert_eq!(
            adjacent_page_id_after_close(&pages, 1).as_deref(),
            Some("c")
        );
        assert_eq!(
            adjacent_page_id_after_close(&pages, 2).as_deref(),
            Some("b")
        );
        assert_eq!(adjacent_page_id_after_close(&pages[..1], 0), None);
    }

    fn test_page(id: &str) -> Page {
        Page {
            id: id.to_owned(),
            url: String::new(),
            title: String::new(),
            position: 0,
        }
    }
}
