use std::collections::HashMap;

pub use archetype_types::{NavigationId, PageId};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BrowserCommand {
    Navigate { page_id: PageId, url: Url },
    Back { page_id: PageId },
    Forward { page_id: PageId },
    Reload { page_id: PageId },
    Stop { page_id: PageId },
    Resize { page_id: PageId, viewport: Viewport },
    Scroll { page_id: PageId, delta_y: f32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoadStage {
    Loading,
    Parsed,
    LaidOut,
    Ready,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BrowserEvent {
    NavigationStarted {
        page_id: PageId,
        navigation_id: NavigationId,
        url: Url,
    },
    LoadStageChanged {
        page_id: PageId,
        navigation_id: NavigationId,
        stage: LoadStage,
    },
    TitleChanged {
        page_id: PageId,
        navigation_id: NavigationId,
        title: String,
    },
    NavigationFinished {
        page_id: PageId,
        navigation_id: NavigationId,
        final_url: Url,
    },
    NavigationFailed {
        page_id: PageId,
        navigation_id: NavigationId,
        message: String,
    },
    ViewportChanged {
        page_id: PageId,
        viewport: Viewport,
    },
    ScrollChanged {
        page_id: PageId,
        offset_y: f32,
    },
    Ignored,
}

#[derive(Clone, Debug)]
struct PageState {
    history: Vec<Url>,
    cursor: usize,
    navigation_id: NavigationId,
    viewport: Viewport,
    scroll_y: f32,
}

impl Default for PageState {
    fn default() -> Self {
        Self {
            history: Vec::new(),
            cursor: 0,
            navigation_id: NavigationId::zero(),
            viewport: Viewport {
                width: 1280.0,
                height: 800.0,
            },
            scroll_y: 0.0,
        }
    }
}

#[derive(Default)]
pub struct Session {
    pages: HashMap<PageId, PageState>,
}

impl Session {
    pub fn open_page(&mut self, page_id: PageId) {
        self.pages.entry(page_id).or_default();
    }

    pub fn restore_page(&mut self, page_id: PageId, url: Url) {
        self.pages.insert(
            page_id,
            PageState {
                history: vec![url],
                ..PageState::default()
            },
        );
    }

    pub fn close_page(&mut self, page_id: &PageId) {
        self.pages.remove(page_id);
    }

    #[must_use]
    pub fn can_go_back(&self, page_id: &PageId) -> bool {
        self.pages
            .get(page_id)
            .is_some_and(|page| !page.history.is_empty() && page.cursor > 0)
    }

    #[must_use]
    pub fn can_go_forward(&self, page_id: &PageId) -> bool {
        self.pages
            .get(page_id)
            .is_some_and(|page| page.cursor + 1 < page.history.len())
    }

    #[must_use]
    pub fn handle(&mut self, command: BrowserCommand) -> BrowserEvent {
        match command {
            BrowserCommand::Navigate { page_id, url } => {
                let page = self.pages.entry(page_id.clone()).or_default();
                if !page.history.is_empty() {
                    page.history.truncate(page.cursor + 1);
                }
                page.history.push(url.clone());
                page.cursor = page.history.len() - 1;
                start(page_id, page, url)
            }
            BrowserCommand::Back { page_id } => {
                let Some(page) = self.pages.get_mut(&page_id) else {
                    return BrowserEvent::Ignored;
                };
                if page.cursor == 0 {
                    return BrowserEvent::Ignored;
                }
                page.cursor -= 1;
                start(page_id, page, page.history[page.cursor].clone())
            }
            BrowserCommand::Forward { page_id } => {
                let Some(page) = self.pages.get_mut(&page_id) else {
                    return BrowserEvent::Ignored;
                };
                if page.cursor + 1 >= page.history.len() {
                    return BrowserEvent::Ignored;
                }
                page.cursor += 1;
                start(page_id, page, page.history[page.cursor].clone())
            }
            BrowserCommand::Reload { page_id } => {
                let Some(page) = self.pages.get_mut(&page_id) else {
                    return BrowserEvent::Ignored;
                };
                let Some(url) = page.history.get(page.cursor).cloned() else {
                    return BrowserEvent::Ignored;
                };
                start(page_id, page, url)
            }
            BrowserCommand::Stop { page_id } => {
                let Some(page) = self.pages.get_mut(&page_id) else {
                    return BrowserEvent::Ignored;
                };
                let navigation_id = page.navigation_id;
                page.navigation_id = page.navigation_id.saturating_next();
                BrowserEvent::LoadStageChanged {
                    page_id,
                    navigation_id,
                    stage: LoadStage::Cancelled,
                }
            }
            BrowserCommand::Resize { page_id, viewport } => {
                let page = self.pages.entry(page_id.clone()).or_default();
                page.viewport = viewport;
                BrowserEvent::ViewportChanged { page_id, viewport }
            }
            BrowserCommand::Scroll { page_id, delta_y } => {
                let page = self.pages.entry(page_id.clone()).or_default();
                page.scroll_y = (page.scroll_y + delta_y).max(0.0);
                BrowserEvent::ScrollChanged {
                    page_id,
                    offset_y: page.scroll_y,
                }
            }
        }
    }

    #[must_use]
    pub fn accepts(&self, page_id: &PageId, navigation_id: NavigationId) -> bool {
        self.pages
            .get(page_id)
            .is_some_and(|page| page.navigation_id == navigation_id)
    }

    /// Replaces the active history entry with a navigation's final redirected URL.
    ///
    /// Returns `false` when the page is missing or the result belongs to an older navigation.
    pub fn commit_final_url(
        &mut self,
        page_id: &PageId,
        navigation_id: NavigationId,
        final_url: Url,
    ) -> bool {
        let Some(page) = self.pages.get_mut(page_id) else {
            return false;
        };
        if page.navigation_id != navigation_id {
            return false;
        }
        let Some(current) = page.history.get_mut(page.cursor) else {
            return false;
        };
        *current = final_url;
        true
    }
}

fn start(page_id: PageId, page: &mut PageState, url: Url) -> BrowserEvent {
    page.navigation_id = page.navigation_id.saturating_next();
    page.scroll_y = 0.0;
    BrowserEvent::NavigationStarted {
        page_id,
        navigation_id: page.navigation_id,
        url,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_ids_increase_and_stale_results_are_rejected() {
        let mut session = Session::default();
        let page_id = PageId::new();
        let first = session.handle(BrowserCommand::Navigate {
            page_id: page_id.clone(),
            url: Url::parse("https://example.com/one").unwrap(),
        });
        let second = session.handle(BrowserCommand::Navigate {
            page_id: page_id.clone(),
            url: Url::parse("https://example.com/two").unwrap(),
        });
        let id = |event| match event {
            BrowserEvent::NavigationStarted { navigation_id, .. } => navigation_id,
            _ => NavigationId::zero(),
        };
        assert!(!session.accepts(&page_id, id(first)));
        assert!(session.accepts(&page_id, id(second)));
    }

    #[test]
    fn back_and_forward_do_not_duplicate_history() {
        let mut session = Session::default();
        let page_id = PageId::new();
        for url in ["https://example.com/one", "https://example.com/two"] {
            let _ = session.handle(BrowserCommand::Navigate {
                page_id: page_id.clone(),
                url: Url::parse(url).unwrap(),
            });
        }
        let back = session.handle(BrowserCommand::Back {
            page_id: page_id.clone(),
        });
        let forward = session.handle(BrowserCommand::Forward { page_id });
        assert!(
            matches!(back, BrowserEvent::NavigationStarted { url, .. } if url.path() == "/one")
        );
        assert!(
            matches!(forward, BrowserEvent::NavigationStarted { url, .. } if url.path() == "/two")
        );
    }

    #[test]
    fn restored_page_can_reload_its_current_url() {
        let mut session = Session::default();
        let page_id = PageId::new();
        session.restore_page(
            page_id.clone(),
            Url::parse("https://example.com/restored").unwrap(),
        );
        let reload = session.handle(BrowserCommand::Reload { page_id });
        assert!(
            matches!(reload, BrowserEvent::NavigationStarted { url, .. } if url.path() == "/restored")
        );
    }

    #[test]
    fn final_redirect_url_replaces_current_history_entry() {
        let mut session = Session::default();
        let page_id = PageId::new();
        session.open_page(page_id.clone());
        let requested = Url::parse("https://example.test/old").unwrap();
        let final_url = Url::parse("https://example.test/new").unwrap();
        let started = session.handle(BrowserCommand::Navigate {
            page_id: page_id.clone(),
            url: requested,
        });
        let BrowserEvent::NavigationStarted { navigation_id, .. } = started else {
            panic!("navigation should start");
        };
        assert!(session.commit_final_url(&page_id, navigation_id, final_url.clone()));
        assert_eq!(
            session.handle(BrowserCommand::Reload {
                page_id: page_id.clone(),
            }),
            BrowserEvent::NavigationStarted {
                page_id,
                navigation_id: navigation_id.saturating_next(),
                url: final_url
            }
        );
    }

    #[test]
    fn history_availability_tracks_the_cursor() {
        let mut session = Session::default();
        let page_id = PageId::new();
        assert!(!session.can_go_back(&page_id));
        assert!(!session.can_go_forward(&page_id));

        for url in ["https://example.com/one", "https://example.com/two"] {
            let _ = session.handle(BrowserCommand::Navigate {
                page_id: page_id.clone(),
                url: Url::parse(url).unwrap(),
            });
        }
        assert!(session.can_go_back(&page_id));
        assert!(!session.can_go_forward(&page_id));

        let _ = session.handle(BrowserCommand::Back {
            page_id: page_id.clone(),
        });
        assert!(!session.can_go_back(&page_id));
        assert!(session.can_go_forward(&page_id));
    }

    #[test]
    fn stop_invalidates_the_active_navigation() {
        let mut session = Session::default();
        let page_id = PageId::new();
        let started = session.handle(BrowserCommand::Navigate {
            page_id: page_id.clone(),
            url: Url::parse("https://example.com/slow").unwrap(),
        });
        let BrowserEvent::NavigationStarted { navigation_id, .. } = started else {
            panic!("navigation should start");
        };
        let stopped = session.handle(BrowserCommand::Stop {
            page_id: page_id.clone(),
        });
        assert!(matches!(
            stopped,
            BrowserEvent::LoadStageChanged {
                navigation_id: stopped_id,
                stage: LoadStage::Cancelled,
                ..
            } if stopped_id == navigation_id
        ));
        assert!(!session.accepts(&page_id, navigation_id));
    }
}
