use std::{collections::HashMap, path::PathBuf};

#[cfg(feature = "cef-runtime")]
use std::ptr;

use thiserror::Error;
use url::Url;

pub const CEF_VERSION: &str = "151.8.0+151.3.24";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeConfig {
    pub profile_root: PathBuf,
    pub cache_path: PathBuf,
    pub locale: String,
}

impl RuntimeConfig {
    #[must_use]
    pub fn persistent(profile_root: PathBuf, locale: impl Into<String>) -> Self {
        let cache_path = profile_root.join("chromium").join("Default");
        Self {
            profile_root,
            cache_path,
            locale: locale.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ContentBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChromiumCommand {
    Create { tab_id: String, url: Url },
    Show { tab_id: String },
    Hide { tab_id: String },
    Close { tab_id: String },
    Navigate { tab_id: String, url: Url },
    Back { tab_id: String },
    Forward { tab_id: String },
    Reload { tab_id: String },
    Stop { tab_id: String },
    Resize { bounds: ContentBounds },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChromiumEvent {
    Loading(bool),
    UrlChanged(Url),
    TitleChanged(String),
    FaviconChanged(Vec<u8>),
    NavigationFailed { code: i32, description: String },
    RendererTerminated { status: String },
    PopupRequested(Url),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventEnvelope {
    pub tab_id: String,
    pub generation: u64,
    pub event: ChromiumEvent,
}

#[derive(Clone, Debug, Default)]
pub struct TabGenerations {
    generations: HashMap<String, u64>,
}

impl TabGenerations {
    #[must_use]
    pub fn register(&mut self, tab_id: impl Into<String>) -> u64 {
        let tab_id = tab_id.into();
        *self.generations.entry(tab_id).or_insert(0)
    }

    #[must_use]
    pub fn begin_navigation(&mut self, tab_id: &str) -> Option<u64> {
        let generation = self.generations.get_mut(tab_id)?;
        *generation = generation.saturating_add(1);
        Some(*generation)
    }

    pub fn close(&mut self, tab_id: &str) -> bool {
        self.generations.remove(tab_id).is_some()
    }

    #[must_use]
    pub fn accepts(&self, event: &EventEnvelope) -> bool {
        self.generations.get(&event.tab_id) == Some(&event.generation)
    }
}

#[must_use]
pub fn is_content_url(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https" | "file" | "about")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CefDistribution {
    pub root: PathBuf,
    pub framework_binary: PathBuf,
    pub resources: PathBuf,
    pub locales: PathBuf,
}

impl CefDistribution {
    /// Locates the pinned CEF distribution downloaded at build time or supplied through
    /// `CEF_PATH`.
    ///
    /// # Errors
    /// Returns an error when the distribution or required macOS resources are unavailable.
    pub fn discover() -> Result<Self, ChromiumError> {
        #[cfg(feature = "cef-runtime")]
        let root = cef::sys::get_cef_dir().ok_or(ChromiumError::DistributionMissing)?;
        #[cfg(not(feature = "cef-runtime"))]
        let root = std::env::var_os("CEF_PATH")
            .map(PathBuf::from)
            .ok_or(ChromiumError::DistributionMissing)?;
        Self::from_root(root)
    }

    /// Validates a CEF distribution root.
    ///
    /// # Errors
    /// Returns an error when the framework, resources, or locales are missing.
    pub fn from_root(root: PathBuf) -> Result<Self, ChromiumError> {
        let framework = root.join("Chromium Embedded Framework.framework");
        let framework_binary = framework.join("Chromium Embedded Framework");
        let resources = framework.join("Resources");
        let locales = resources.join("locales");
        for path in [&framework_binary, &resources, &locales] {
            if !path.exists() {
                return Err(ChromiumError::RequiredPathMissing(path.clone()));
            }
        }
        Ok(Self {
            root,
            framework_binary,
            resources,
            locales,
        })
    }
}

#[derive(Debug, Error)]
pub enum ChromiumError {
    #[error("CEF distribution is unavailable; build the bundled Chromium application first")]
    DistributionMissing,
    #[error("CEF distribution is missing required path: {0}")]
    RequiredPathMissing(PathBuf),
    #[error("Chromium Framework could not be loaded from the application bundle")]
    FrameworkLoadFailed,
    #[error("CEF helper was launched without a valid application bundle")]
    InvalidHelperBundle,
    #[error("this binary was built without the chromium-runtime feature")]
    RuntimeFeatureDisabled,
}

/// Executes the current process as a CEF Renderer/GPU/Utility helper.
///
/// # Errors
/// Returns an error when the process is not inside a valid CEF application bundle or the
/// Chromium Framework cannot be loaded.
pub fn run_subprocess() -> Result<i32, ChromiumError> {
    #[cfg(not(feature = "cef-runtime"))]
    return Err(ChromiumError::RuntimeFeatureDisabled);

    #[cfg(feature = "cef-runtime")]
    {
        let executable = std::env::current_exe().map_err(|_| ChromiumError::InvalidHelperBundle)?;
        let loader =
            std::panic::catch_unwind(|| cef::library_loader::LibraryLoader::new(&executable, true))
                .map_err(|_| ChromiumError::InvalidHelperBundle)?;
        if !loader.load() {
            return Err(ChromiumError::FrameworkLoadFailed);
        }
        let args = cef::args::Args::new();
        Ok(cef::execute_process(
            Some(args.as_main_args()),
            None,
            ptr::null_mut(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_an_incomplete_distribution() {
        let root = std::env::temp_dir().join("archetype-missing-cef-distribution");
        let error = CefDistribution::from_root(root).unwrap_err();
        assert!(matches!(error, ChromiumError::RequiredPathMissing(_)));
    }

    #[test]
    fn persistent_profile_keeps_chromium_site_data_outside_sqlite() {
        let config = RuntimeConfig::persistent(PathBuf::from("/profiles/main"), "zh-CN");
        assert_eq!(
            config.cache_path,
            PathBuf::from("/profiles/main/chromium/Default")
        );
        assert_eq!(config.locale, "zh-CN");
    }

    #[test]
    fn rejects_internal_routes_from_chromium_content() {
        assert!(is_content_url(&Url::parse("https://example.com").unwrap()));
        assert!(is_content_url(&Url::parse("about:blank").unwrap()));
        assert!(!is_content_url(
            &Url::parse("archetype://settings/appearance").unwrap()
        ));
    }

    #[test]
    fn ignores_stale_and_closed_tab_events() {
        let mut tabs = TabGenerations::default();
        assert_eq!(tabs.register("tab-1"), 0);
        let first = tabs.begin_navigation("tab-1").unwrap();
        let second = tabs.begin_navigation("tab-1").unwrap();
        let event = |generation| EventEnvelope {
            tab_id: "tab-1".to_owned(),
            generation,
            event: ChromiumEvent::Loading(false),
        };

        assert!(!tabs.accepts(&event(first)));
        assert!(tabs.accepts(&event(second)));
        assert!(tabs.close("tab-1"));
        assert!(!tabs.accepts(&event(second)));
    }

    #[cfg(feature = "cef-runtime")]
    #[test]
    fn build_time_distribution_contains_framework_resources_and_locales() {
        let distribution = CefDistribution::discover().unwrap();
        assert!(distribution.framework_binary.is_file());
        assert!(distribution.resources.is_dir());
        assert!(distribution.locales.is_dir());
    }
}
