use std::{borrow::Cow, path::PathBuf};

use arch_browser::{BrowserCore, RenderedPage};
use arch_net::{LoadError, LoadErrorKind};
use arch_paint::{DisplayCommand, PaintColor};
use arch_store::{Page, Space};
use arch_style::{FontStyle as PageFontStyle, FontWeight as PageFontWeight, TextAlign};
use directories::ProjectDirs;
use gpui::{
    AnyElement, AppContext as _, Application, AssetSource, Context, Entity, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement as _, Styled, Subscription, Window, WindowBounds, WindowOptions,
    div, img, prelude::FluentBuilder as _, px, rgba, size,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, IconNamed, Root, Sizable as _, StyledExt as _,
    TitleBar,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use url::Url;

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
    fn input(detail: impl Into<String>) -> Self {
        Self {
            title: "Invalid input",
            detail: detail.into(),
        }
    }

    fn application(error: &impl std::fmt::Display) -> Self {
        Self {
            title: "Application error",
            detail: error.to_string(),
        }
    }

    fn navigation(error: &anyhow::Error) -> Self {
        let kind = error
            .chain()
            .find_map(|cause| cause.downcast_ref::<LoadError>())
            .map(LoadError::kind);
        let title = match kind {
            Some(LoadErrorKind::UnsupportedScheme | LoadErrorKind::InvalidFileUrl) => {
                "Unsupported address"
            }
            Some(LoadErrorKind::ResourceTooLarge) => "Resource too large",
            Some(LoadErrorKind::File) => "File unavailable",
            Some(LoadErrorKind::Timeout) => "Request timed out",
            Some(LoadErrorKind::Connection) => "Connection failed",
            Some(LoadErrorKind::HttpStatus) => "HTTP request failed",
            Some(LoadErrorKind::Network) => "Secure network request failed",
            None => "Rendering failed",
        };
        Self {
            title,
            detail: format!("{error:#}"),
        }
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
    core: BrowserCore,
    spaces: Vec<Space>,
    pages: Vec<Page>,
    selected_space: Option<String>,
    selected_page: Option<String>,
    rendered: Option<RenderedPage>,
    error: Option<ErrorView>,
    address_input: Entity<InputState>,
    space_input: Entity<InputState>,
    subscriptions: Vec<Subscription>,
}

impl QuickBrowser {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let profile = profile_path();
        let mut core = BrowserCore::open(&profile).unwrap_or_else(|error| {
            eprintln!("profile unavailable at {}: {error}", profile.display());
            BrowserCore::in_memory().expect("in-memory profile must initialize")
        });
        let mut spaces = core.spaces().unwrap_or_default();
        if spaces.is_empty() {
            if let Ok(space) = core.create_space("Start") {
                spaces.push(space);
            }
        }
        let saved = core.selection().unwrap_or_default();
        let selected_space = saved
            .0
            .filter(|id| spaces.iter().any(|space| &space.id == id))
            .or_else(|| spaces.first().map(|space| space.id.clone()));
        let pages = selected_space
            .as_deref()
            .and_then(|id| core.pages(id).ok())
            .unwrap_or_default();
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
            cx.new(|cx| InputState::new(window, cx).placeholder("Search or enter an address"));
        address_input.update(cx, |input, cx| input.set_value(address, window, cx));
        let space_input = cx.new(|cx| InputState::new(window, cx).placeholder("Space name"));
        space_input.update(cx, |input, cx| input.set_value(space_name, window, cx));

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
        ];

        Self {
            core,
            spaces,
            pages,
            selected_space,
            selected_page,
            rendered: None,
            error: None,
            address_input,
            space_input,
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
        let name = format!("Space {}", self.spaces.len() + 1);
        match self.core.create_space(&name) {
            Ok(space) => {
                self.selected_space = Some(space.id.clone());
                self.selected_page = None;
                self.pages.clear();
                self.spaces.push(space);
                self.rendered = None;
                self.set_space_name(name, window, cx);
                self.set_address("", window, cx);
                self.persist_selection();
            }
            Err(error) => self.error = Some(ErrorView::application(&error)),
        }
        cx.notify();
    }

    fn select_space(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.error = None;
        self.selected_space = Some(id.to_owned());
        let name = self
            .spaces
            .iter()
            .find(|space| space.id == id)
            .map(|space| space.name.clone())
            .unwrap_or_default();
        self.pages = self.core.pages(id).unwrap_or_default();
        self.selected_page = self.pages.first().map(|page| page.id.clone());
        let address = self
            .selected_page_record()
            .map(|page| page.url.clone())
            .unwrap_or_default();
        self.rendered = None;
        self.set_space_name(name, window, cx);
        self.set_address(address, window, cx);
        self.persist_selection();
        cx.notify();
    }

    fn rename_selected_space(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.error = None;
        let name = self.space_input.read(cx).value().trim().to_owned();
        if name.is_empty() {
            self.error = Some(ErrorView::input("Space name cannot be empty"));
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
                self.set_space_name(name, window, cx);
            }
            Ok(false) => self.error = Some(ErrorView::input("Selected Space no longer exists")),
            Err(error) => self.error = Some(ErrorView::application(&error)),
        }
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
                self.pages = self
                    .selected_space
                    .as_deref()
                    .and_then(|space_id| self.core.pages(space_id).ok())
                    .unwrap_or_default();
                self.selected_page = self.pages.first().map(|page| page.id.clone());
                let address = self
                    .selected_page_record()
                    .map(|page| page.url.clone())
                    .unwrap_or_default();
                self.rendered = None;
                self.set_space_name(name, window, cx);
                self.set_address(address, window, cx);
                self.persist_selection();
            }
            Err(error) => self.error = Some(ErrorView::application(&error)),
        }
        cx.notify();
    }

    fn add_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(space_id) = self.selected_space.clone() else {
            return;
        };
        let url = fixture_url();
        match self.core.create_page(&space_id, &url) {
            Ok(page) => {
                self.selected_page = Some(page.id.clone());
                self.pages.push(page);
                self.set_address(url.to_string(), window, cx);
                self.persist_selection();
                self.navigate_to(&url, window, cx);
            }
            Err(error) => {
                self.error = Some(ErrorView::application(&error));
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
        let Some(page) = self.pages.iter().find(|page| page.id == id).cloned() else {
            return;
        };
        if let Err(error) = self.core.close_page(&page) {
            self.error = Some(ErrorView::application(&error));
            cx.notify();
            return;
        }
        self.pages.retain(|item| item.id != id);
        self.selected_page = self.pages.first().map(|item| item.id.clone());
        let address = self
            .selected_page_record()
            .map(|page| page.url.clone())
            .unwrap_or_default();
        self.rendered = None;
        self.set_address(address, window, cx);
        self.persist_selection();
        cx.notify();
    }

    fn navigate_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let address = self.address_input.read(cx).value();
        match parse_address(&address) {
            Ok(url) => self.navigate_to(&url, window, cx),
            Err(error) => {
                self.error = Some(ErrorView::input(error));
                cx.notify();
            }
        }
    }

    fn navigate_to(&mut self, url: &Url, window: &mut Window, cx: &mut Context<Self>) {
        self.error = None;
        if self.selected_page_record().is_none() {
            let Some(space_id) = self.selected_space.clone() else {
                return;
            };
            match self.core.create_page(&space_id, url) {
                Ok(page) => {
                    self.selected_page = Some(page.id.clone());
                    self.pages.push(page);
                    self.persist_selection();
                }
                Err(error) => {
                    self.error = Some(ErrorView::application(&error));
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
                self.error = Some(ErrorView::navigation(&error));
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
                self.error = Some(ErrorView::navigation(&error));
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

    fn persist_selection(&mut self) {
        if let Err(error) = self.core.save_selection(
            self.selected_space.as_deref(),
            self.selected_page.as_deref(),
        ) {
            self.error = Some(ErrorView::application(&error));
        }
    }

    fn space_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        self.spaces
            .iter()
            .map(|space| {
                let id = space.id.clone();
                let active = self.selected_space.as_deref() == Some(space.id.as_str());
                Button::new(SharedString::from(format!("space-{}", space.id)))
                    .ghost()
                    .label(space.name.clone())
                    .w_full()
                    .when(active, gpui_component::button::ButtonVariants::primary)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_space(&id, window, cx);
                    }))
                    .into_any_element()
            })
            .collect()
    }

    fn page_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        self.pages
            .iter()
            .map(|page| {
                let select_id = page.id.clone();
                let close_id = page.id.clone();
                let active = self.selected_page.as_deref() == Some(page.id.as_str());
                let label = if page.title.is_empty() {
                    page.url.clone()
                } else {
                    page.title.clone()
                };
                h_flex()
                    .w_full()
                    .gap_1()
                    .child(
                        Button::new(SharedString::from(format!("page-{}", page.id)))
                            .ghost()
                            .label(label)
                            .w_full()
                            .when(active, gpui_component::button::ButtonVariants::primary)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select_page(&select_id, window, cx);
                            })),
                    )
                    .child(
                        Button::new(SharedString::from(format!("close-page-{}", page.id)))
                            .ghost()
                            .icon(AppIcon::Close)
                            .xsmall()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.close_page(&close_id, window, cx);
                            })),
                    )
                    .into_any_element()
            })
            .collect()
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let space_rows = self.space_rows(cx);
        let page_rows = self.page_rows(cx);

        v_flex()
            .w(px(300.0))
            .h_full()
            .flex_shrink_0()
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().sidebar)
            .child(
                h_flex()
                    .px_3()
                    .pt_3()
                    .pb_2()
                    .justify_between()
                    .child(div().text_sm().font_semibold().child("SPACES"))
                    .child(
                        Button::new("add-space")
                            .ghost()
                            .icon(AppIcon::Add)
                            .small()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.add_space(window, cx);
                            })),
                    ),
            )
            .child(v_flex().px_2().gap_1().children(space_rows))
            .child(
                h_flex()
                    .px_2()
                    .py_2()
                    .gap_1()
                    .child(div().flex_1().child(Input::new(&self.space_input).small()))
                    .child(
                        Button::new("rename-space")
                            .ghost()
                            .icon(AppIcon::Rename)
                            .small()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.rename_selected_space(window, cx);
                            })),
                    )
                    .child(
                        Button::new("delete-space")
                            .ghost()
                            .icon(AppIcon::Delete)
                            .small()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.delete_selected_space(window, cx);
                            })),
                    ),
            )
            .child(
                h_flex()
                    .px_3()
                    .pt_3()
                    .pb_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .justify_between()
                    .child(div().text_sm().font_semibold().child("PAGES"))
                    .child(
                        Button::new("add-page")
                            .ghost()
                            .icon(AppIcon::Add)
                            .small()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.add_page(window, cx);
                            })),
                    ),
            )
            .child(
                v_flex()
                    .id("page-list")
                    .flex_1()
                    .overflow_y_scroll()
                    .px_2()
                    .gap_1()
                    .children(page_rows),
            )
            .into_any_element()
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let current = self.selected_page_record();
        let can_back = current.is_some_and(|page| self.core.can_go_back(page));
        let can_forward = current.is_some_and(|page| self.core.can_go_forward(page));
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
                    .disabled(!can_back)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate_history(HistoryDirection::Back, window, cx);
                    })),
            )
            .child(
                Button::new("forward")
                    .ghost()
                    .icon(AppIcon::Forward)
                    .disabled(!can_forward)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate_history(HistoryDirection::Forward, window, cx);
                    })),
            )
            .child(
                Button::new("reload")
                    .ghost()
                    .icon(AppIcon::Refresh)
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
                Button::new("navigate")
                    .primary()
                    .icon(AppIcon::Forward)
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
                .child("Open a page to begin")
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
                .child(div().font_semibold().text_sm().child("Diagnostics"))
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
        let bounds = match command {
            DisplayCommand::Box { bounds, .. }
            | DisplayCommand::Text { bounds, .. }
            | DisplayCommand::Image { bounds, .. } => *bounds,
        };
        match command {
            DisplayCommand::Box {
                background,
                border,
                border_width_px,
                ..
            } => div()
                .absolute()
                .left(px(bounds.x))
                .top(px(bounds.y))
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
                    .left(px(bounds.x))
                    .top(px(bounds.y))
                    .w(px(bounds.width))
                    .h(px(bounds.height))
                    .text_size(px(*size_px))
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
                        .left(px(bounds.x))
                        .top(px(bounds.y))
                        .w(px(bounds.width))
                        .h(px(bounds.height))
                        .into_any_element()
                } else {
                    div()
                        .absolute()
                        .left(px(bounds.x))
                        .top(px(bounds.y))
                        .w(px(bounds.width))
                        .h(px(bounds.height))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(alt.clone())
                        .into_any_element()
                }
            }
        }
    }
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
                        .w_full()
                        .justify_between()
                        .child(div().font_semibold().child("Archetype"))
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("V3 Developer Preview"),
                        ),
                ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .child(self.sidebar(cx))
                    .child(
                        v_flex()
                            .flex_1()
                            .h_full()
                            .overflow_hidden()
                            .child(self.toolbar(cx))
                            .child(self.content(cx)),
                    ),
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

fn parse_address(address: &str) -> Result<Url, String> {
    let address = address.trim();
    if address.is_empty() {
        return Err("address cannot be empty".to_owned());
    }
    if address.contains("://") {
        return Url::parse(address).map_err(|error| format!("invalid URL: {error}"));
    }
    if let Ok(path) = PathBuf::from(address).canonicalize() {
        return Url::from_file_path(path)
            .map_err(|()| "path cannot be represented as a file URL".to_owned());
    }
    if looks_like_host(address) {
        return Url::parse(&format!("https://{address}"))
            .map_err(|error| format!("invalid URL: {error}"));
    }
    if let Ok(url) = Url::parse(address) {
        return Ok(url);
    }
    Err(format!("invalid address or missing path: {address}"))
}

fn looks_like_host(address: &str) -> bool {
    !address.chars().any(char::is_whitespace)
        && !address.starts_with('.')
        && !address.starts_with('/')
        && (address == "localhost" || address.starts_with("localhost:") || address.contains('.'))
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
    fn address_parser_adds_https_to_hostnames() {
        assert_eq!(
            parse_address("baidu.com").unwrap().as_str(),
            "https://baidu.com/"
        );
        assert_eq!(
            parse_address("localhost:8080").unwrap().as_str(),
            "https://localhost:8080/"
        );
    }

    #[test]
    fn address_parser_preserves_explicit_urls() {
        assert_eq!(
            parse_address(" http://example.com/docs ").unwrap().as_str(),
            "http://example.com/docs"
        );
    }

    #[test]
    fn address_parser_prefers_existing_local_files() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pages/01-document/index.html")
            .canonicalize()
            .unwrap();
        let parsed = parse_address(path.to_str().unwrap()).unwrap();
        assert_eq!(parsed.to_file_path().unwrap(), path);
    }

    #[test]
    fn address_parser_rejects_empty_input() {
        assert_eq!(parse_address("  ").unwrap_err(), "address cannot be empty");
    }
}
