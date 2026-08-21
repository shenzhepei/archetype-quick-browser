use std::{borrow::Cow, path::PathBuf};

use arch_browser::{BrowserCore, RenderError, RenderErrorKind, RenderedPage};
use arch_net::{LoadError, LoadErrorKind};
use arch_paint::{DisplayCommand, PaintColor};
use arch_store::{Bookmark, BookmarkKind, Page, Space};
use arch_style::{FontStyle as PageFontStyle, FontWeight as PageFontWeight, TextAlign};
use directories::ProjectDirs;
use gpui::{
    AnyElement, AppContext as _, Application, AssetSource, Context, Entity, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement as _, Styled, Subscription, Window, WindowBounds, WindowOptions,
    div, img, prelude::FluentBuilder as _, px, rgba, size,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, IconNamed, Root, Sizable as _,
    StyledExt as _, TitleBar,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenu, PopupMenuItem},
    v_flex,
};
use url::Url;

use crate::i18n::Language;

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
        LoadErrorKind::HttpStatus => language.http_request_failed(),
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
                cx.new(|cx| Root::new(browser, window, cx))
            })?;
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}

struct QuickBrowser {
    language: Language,
    core: BrowserCore,
    spaces: Vec<Space>,
    bookmarks: Vec<Bookmark>,
    pages: Vec<Page>,
    selected_space: Option<String>,
    selected_page: Option<String>,
    rendered: Option<RenderedPage>,
    error: Option<ErrorView>,
    address_input: Entity<InputState>,
    space_input: Entity<InputState>,
    folder_input: Entity<InputState>,
    renaming_space: bool,
    creating_bookmark_folder: bool,
    bookmark_folder_parent: Option<String>,
    renaming_bookmark: Option<String>,
    subscriptions: Vec<Subscription>,
}

impl QuickBrowser {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let language = Language::system();
        let profile = profile_path();
        let mut core = BrowserCore::open(&profile).unwrap_or_else(|error| {
            eprintln!("profile unavailable at {}: {error}", profile.display());
            BrowserCore::in_memory().expect("in-memory profile must initialize")
        });
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
        let pages = core.pages().unwrap_or_default();
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

        let subscriptions = vec![
            cx.subscribe_in(&address_input, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.navigate_current(window, cx);
                }
            }),
            cx.subscribe_in(&space_input, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.rename_selected_space(window, cx);
                }
            }),
            cx.subscribe_in(&folder_input, window, |this, _, event, _window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.save_bookmark_editor(cx);
                }
            }),
        ];

        Self {
            language,
            core,
            spaces,
            bookmarks,
            pages,
            selected_space,
            selected_page,
            rendered: None,
            error: None,
            address_input,
            space_input,
            folder_input,
            renaming_space: false,
            creating_bookmark_folder: false,
            bookmark_folder_parent: None,
            renaming_bookmark: None,
            subscriptions,
        }
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
        let url = fixture_url();
        match self.core.create_page(&url) {
            Ok(page) => {
                self.selected_page = Some(page.id.clone());
                self.pages.push(page);
                self.set_address(url.to_string(), window, cx);
                self.persist_selection();
                self.navigate_to(&url, window, cx);
            }
            Err(error) => {
                self.error = Some(ErrorView::application(self.language, &error));
                cx.notify();
            }
        }
    }

    fn select_page(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.error = None;
        self.selected_page = Some(id.to_owned());
        let address = self
            .selected_page_record()
            .map(|page| page.url.clone())
            .unwrap_or_default();
        self.rendered = None;
        self.set_address(address, window, cx);
        self.persist_selection();
        cx.notify();
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
        self.pages.retain(|item| item.id != id);
        if closing_selected {
            self.selected_page = adjacent_page;
            let address = self
                .selected_page_record()
                .map(|page| page.url.clone())
                .unwrap_or_default();
            self.rendered = None;
            self.set_address(address, window, cx);
        }
        self.persist_selection();
        cx.notify();
    }

    fn navigate_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let address = self.address_input.read(cx).value();
        match parse_address(&address, self.language) {
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
        match self.core.navigate(&page, url, 960.0) {
            Ok(rendered) => self.apply_rendered(&page, rendered, window, cx),
            Err(error) => {
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
            HistoryDirection::Back => self.core.back(&page, 960.0),
            HistoryDirection::Forward => self.core.forward(&page, 960.0),
            HistoryDirection::Reload => self.core.reload(&page, 960.0),
        };
        match result {
            Ok(rendered) => self.apply_rendered(&page, rendered, window, cx),
            Err(error) => {
                self.error = Some(ErrorView::navigation(self.language, &error));
                cx.notify();
            }
        }
    }

    fn apply_rendered(
        &mut self,
        page: &Page,
        rendered: RenderedPage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_address(rendered.final_url.to_string(), window, cx);
        if let Some(current) = self.pages.iter_mut().find(|item| item.id == page.id) {
            current.url = rendered.final_url.to_string();
            current.title.clone_from(&rendered.title);
        }
        self.rendered = Some(rendered);
        cx.notify();
    }

    fn selected_page_record(&self) -> Option<&Page> {
        let id = self.selected_page.as_deref()?;
        self.pages.iter().find(|page| page.id == id)
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
            let label = if page.title.is_empty() {
                page.url.clone()
            } else {
                page.title.clone()
            };
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
                    .flex_1()
                    .min_w_0()
                    .overflow_x_scroll()
                    .gap_1()
                    .children(tabs),
            )
            .child(
                Button::new("new-tab")
                    .ghost()
                    .icon(AppIcon::Add)
                    .xsmall()
                    .tooltip(self.language.new_tab())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.add_page(window, cx);
                    })),
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
        let can_back = current.is_some_and(|page| self.core.can_go_back(page));
        let can_forward = current.is_some_and(|page| self.core.can_go_forward(page));
        let bookmark_folders = self
            .bookmarks
            .iter()
            .filter(|bookmark| bookmark.kind == BookmarkKind::Folder)
            .cloned()
            .collect::<Vec<_>>();
        let browser = cx.entity();
        let language = self.language;
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
            .child(
                Button::new("reload")
                    .ghost()
                    .icon(AppIcon::Refresh)
                    .tooltip(self.language.reload())
                    .disabled(current.is_none())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate_history(HistoryDirection::Reload, window, cx);
                    })),
            )
            .child(
                div().flex_1().rounded_lg().bg(cx.theme().secondary).child(
                    Input::new(&self.address_input)
                        .appearance(false)
                        .cleanable(true),
                ),
            )
            .child(
                Button::new("bookmark-current-page")
                    .ghost()
                    .icon(AppIcon::Star)
                    .tooltip(self.language.bookmark_current_page())
                    .disabled(current.is_none() || self.selected_space.is_none())
                    .dropdown_menu(move |menu, window, cx| {
                        let root_browser = browser.clone();
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
                                menu, folder, &browser, language, window, cx,
                            );
                        }
                        menu
                    }),
            )
            .child(
                Button::new("navigate")
                    .primary()
                    .icon(AppIcon::Forward)
                    .tooltip(self.language.navigate())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate_current(window, cx);
                    })),
            )
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
        let Some(rendered) = &self.rendered else {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_3()
                .text_color(cx.theme().muted_foreground)
                .child(self.language.open_page_to_begin())
                .into_any_element();
        };

        let mut layers = Vec::with_capacity(rendered.display_list.commands.len());
        for command in &rendered.display_list.commands {
            layers.push(Self::display_command(command, cx));
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
            .bg(gpui::white())
            .p_5()
            .child(canvas)
            .when_some(diagnostics, |content, diagnostics| {
                content.child(diagnostics)
            })
            .into_any_element()
    }

    fn display_command(command: &DisplayCommand, cx: &mut Context<Self>) -> AnyElement {
        let (bounds, clip) = match command {
            DisplayCommand::Box { bounds, clip, .. }
            | DisplayCommand::Text { bounds, clip, .. }
            | DisplayCommand::Image { bounds, clip, .. } => (*bounds, *clip),
        };
        let (x, y) = relative_position(bounds, clip);
        let element = match command {
            DisplayCommand::Box {
                background,
                border,
                border_width_px,
                ..
            } => div()
                .absolute()
                .left(px(x))
                .top(px(y))
                .w(px(bounds.width))
                .h(px(bounds.height))
                .when_some(*background, |layer, color| layer.bg(gpui_color(color)))
                .when(*border_width_px > 0.0, |layer| {
                    layer
                        .border(px(*border_width_px))
                        .border_color(border.map_or_else(gpui::transparent_black, gpui_color))
                })
                .into_any_element(),
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
                ..
            } => {
                if *loaded {
                    img(image_source(source))
                        .absolute()
                        .left(px(x))
                        .top(px(y))
                        .w(px(bounds.width))
                        .h(px(bounds.height))
                        .into_any_element()
                } else {
                    div()
                        .absolute()
                        .left(px(x))
                        .top(px(y))
                        .w(px(bounds.width))
                        .h(px(bounds.height))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(alt.clone())
                        .into_any_element()
                }
            }
        };
        clipped_element(element, clip)
    }
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

fn profile_path() -> PathBuf {
    let base = ProjectDirs::from("org", "Archetype", "Archetype")
        .map_or_else(std::env::temp_dir, |dirs| {
            dirs.data_local_dir().to_path_buf()
        });
    let _ = std::fs::create_dir_all(&base);
    base.join("profile.db")
}

fn fixture_url() -> Url {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/pages/01-document/index.html")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("fixtures/pages/01-document/index.html"));
    Url::from_file_path(path).expect("fixture path must be representable as a URL")
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

fn image_source(source: &str) -> gpui::ImageSource {
    Url::parse(source)
        .ok()
        .and_then(|url| {
            (url.scheme() == "file")
                .then(|| url.to_file_path().ok())
                .flatten()
        })
        .map_or_else(|| source.to_owned().into(), Into::into)
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
