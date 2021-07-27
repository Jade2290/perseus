// This file contains logic to define how pages are rendered

use crate::errors::*;
use serde::{Serialize, de::DeserializeOwned};

// A series of closure types that should not be typed out more than once
// TODO maybe make these public?
type TemplateFnReturn = sycamore::prelude::Template<sycamore::prelude::SsrNode>;
type TemplateFn<Props> = Box<dyn Fn(Option<Props>) -> TemplateFnReturn>;
type GetBuildPathsFn = Box<dyn Fn() -> Vec<String>>;
type GetBuildStateFn<Props> = Box<dyn Fn(String) -> Props>;
type GetRequestStateFn<Props> = Box<dyn Fn(String) -> Props>;
type ShouldRevalidateFn = Box<dyn Fn() -> bool>;

/// This allows the specification of all the page templates in an app and how to render them. If no rendering logic is provided at all,
/// the page will be prerendered at build-time with no state. All closures are stored on the heap to avoid hellish lifetime specification.
pub struct Page<Props: Serialize + DeserializeOwned>
{
    /// The path to the root of the template. Any build paths will be inserted under this.
    path: String,
    /// A function that will render your page. This will be provided the rendered properties, and will be used whenever your page needs
    /// to be prerendered in some way. This should be very similar to the function that hydrates your page on the client side.
    /// This will be executed inside `sycamore::render_to_string`, and should return a `Template<SsrNode>`. This takes an `Option<Props>`
    /// because otherwise efficient typing is almost impossible for pages without any properties (solutions welcome in PRs!).
    template: TemplateFn<Props>,
    /// A function that gets the paths to render for at built-time. This is equivalent to `get_static_paths` in NextJS. If
    /// `incremental_path_rendering` is `true`, more paths can be rendered at request time on top of these.
    get_build_paths: Option<GetBuildPathsFn>,
    /// Defiens whether or not any new paths that match this template will be prerendered and cached in production. This allows you to
    /// have potentially billions of pages and retain a super-fast build process. The first user will have an ever-so-slightly slower
    /// experience, and everyone else gets the beneftis afterwards.
    incremental_path_rendering: bool,
    /// A function that gets the initial state to use to prerender the page at build time. This will be passed the path of the page, and
    /// will be run for any sub-paths. This is equivalent to `get_static_props` in NextJS.
    get_build_state: Option<GetBuildStateFn<Props>>,
    /// A function that will run on every request to generate a state for that request. This allows server-side-rendering. This is equivalent
    /// to `get_server_side_props` in NextJS. This can be used with `get_build_state`, though custom amalgamation logic must be provided.
    // TODO add request data to be passed in here
    get_request_state: Option<GetRequestStateFn<Props>>,
    /// A function to be run on every request to check if a page prerendered at build-time should be prerendered again. This is equivalent
    /// to incremental static rendering (ISR) in NextJS. If used with `revalidate_after`, this function will only be run after that time
    /// period. This function will not be parsed anything specific to the request that invoked it.
    should_revalidate: Option<ShouldRevalidateFn>,
    /// A length of time after which to prerender the page again. This is equivalent to ISR in NextJS.
    revalidate_after: Option<String>,
}
impl<Props: Serialize + DeserializeOwned> Page<Props> {
    /// Creates a new page definition.
    pub fn new(path: impl Into<String> + std::fmt::Display) -> Self {
        Self {
            path: path.to_string(),
            // We only need the `Props` generic here
            template: Box::new(|_: Option<Props>| sycamore::template! {}),
            get_build_paths: None,
            incremental_path_rendering: false,
            get_build_state: None,
            get_request_state: None,
            should_revalidate: None,
            revalidate_after: None,
        }
    }

    // Render executors
    /// Executes the user-given function that renders the page on the server-side (build or request time).
    pub fn render_for_template(&self, props: Option<Props>) -> TemplateFnReturn {
        (self.template)(props)
    }
    /// Gets the list of pages that should be prerendered for at build-time.
    pub fn get_build_paths(&self) -> Result<Vec<String>> {
        if let Some(get_build_paths) = &self.get_build_paths {
            // TODO support error handling for render functions
            Ok(get_build_paths())
        } else {
            bail!(ErrorKind::PageFeatureNotEnabled(self.path.clone(), "build_paths".to_string()))
        }
    }
    /// Gets the initial state for a page. This needs to be passed the full path of the page, which may be one of those generated by
    /// `.get_build_paths()`.
    pub fn get_build_state(&self, path: String) -> Result<Props> {
        if let Some(get_build_state) = &self.get_build_state {
            // TODO support error handling for render functions
            Ok(get_build_state(path))
        } else {
            bail!(ErrorKind::PageFeatureNotEnabled(self.path.clone(), "build_state".to_string()))
        }
    }

    // Value getters
    /// Gets the path of the page.
    pub fn get_path(&self) -> String {
        self.path.clone()
    }

    // Render characteristic checkers
    /// Checks if this page can revalidate existing prerendered pages.
    pub fn revalidates(&self) -> bool {
        self.should_revalidate.is_some() || self.revalidate_after.is_some()
    }
    /// Checks if this page can render more pages beyond those paths it explicitly defines.
    pub fn uses_incremental(&self) -> bool {
        self.incremental_path_rendering
    }
    /// Checks if this page is a template to generate paths beneath it.
    pub fn uses_build_paths(&self) -> bool {
        self.get_build_paths.is_some()
    }
    /// Checks if this page needs to do anything on requests for it.
    pub fn uses_request_state(&self) -> bool {
        self.get_request_state.is_some()
    }
    /// Checks if this page needs to do anything at build time.
    pub fn uses_build_state(&self) -> bool {
        self.get_build_state.is_some()
    }
    /// Checks if this page defines no rendering logic whatsoever. Such pages will be rendered using SSG.
    pub fn is_basic(&self) -> bool {
        !self.uses_build_paths() &&
        !self.uses_build_state() &&
        !self.uses_request_state() &&
        !self.revalidates() &&
        !self.uses_incremental()
    }

    // Builder setters
    pub fn template(mut self, val: TemplateFn<Props>) -> Page<Props> {
        self.template = val;
        self
    }
    pub fn build_paths_fn(mut self, val: GetBuildPathsFn) -> Page<Props> {
        self.get_build_paths = Some(val);
        self
    }
    pub fn incremental_path_rendering(mut self, val: bool) -> Page<Props> {
        self.incremental_path_rendering = val;
        self
    }
    pub fn build_state_fn(mut self, val: GetBuildStateFn<Props>) -> Page<Props> {
        self.get_build_state = Some(val);
        self
    }
    pub fn request_state_fn(mut self, val: GetRequestStateFn<Props>) -> Page<Props> {
        self.get_request_state = Some(val);
        self
    }
    pub fn should_revalidate(mut self, val: ShouldRevalidateFn) -> Page<Props> {
        self.should_revalidate = Some(val);
        self
    }
    pub fn revalidate_after(mut self, val: String) -> Page<Props> {
        self.revalidate_after = Some(val);
        self
    }
}
