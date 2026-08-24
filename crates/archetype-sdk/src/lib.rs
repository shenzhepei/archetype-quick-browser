#![doc = "UI-framework-independent client API for Archetype Runtime."]

mod api;
mod future;

pub use api::{
    Engine, EngineBuilder, Frame, Navigation, Page, PageEvent, PageOptions, Resource, ResourceKind,
    SdkError, StaticDocument,
};
pub use archetype_types::{ArchetypeUrl, NavigationId, PageId};
pub use future::SdkFuture;

#[doc(hidden)]
pub mod runtime_client;
