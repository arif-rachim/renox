//! The pages: the repository's Markdown, compiled into the binary, so the
//! site always shows the docs of the commit it was built from.

/// Where a page sits in the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Section {
    Start,
    Guides,
    Project,
}

impl Section {
    pub const ALL: [Section; 3] = [Section::Start, Section::Guides, Section::Project];

    pub fn title(self) -> &'static str {
        match self {
            Section::Start => "Start here",
            Section::Guides => "Guides",
            Section::Project => "Project",
        }
    }
}

/// One page of the site.
#[derive(Debug)]
pub struct Page {
    /// Its address: `/docs/{slug}`.
    pub slug: &'static str,
    /// The name in the sidebar (the page's own title is its first heading).
    pub nav: &'static str,
    pub section: Section,
    /// The file in the repository, for "Edit on GitHub" and link rewriting.
    pub path: &'static str,
    pub markdown: &'static str,
}

macro_rules! page {
    ($slug:literal, $nav:literal, $section:ident, $path:literal) => {
        Page {
            slug: $slug,
            nav: $nav,
            section: Section::$section,
            path: $path,
            markdown: include_str!(concat!("../../", $path)),
        }
    };
}

/// Every page, in sidebar order.
pub static PAGES: &[Page] = &[
    page!("overview", "Overview", Start, "README.md"),
    page!("tutorial", "Tutorial", Start, "docs/tutorial.md"),
    page!("laravel", "Coming from Laravel", Start, "docs/laravel.md"),
    page!("cheatsheet", "Cheat sheet", Start, "CHEATSHEET.md"),
    page!(
        "routing",
        "Routing and middleware",
        Guides,
        "docs/routing.md"
    ),
    page!(
        "validation",
        "Forms and validation",
        Guides,
        "docs/validation.md"
    ),
    page!("ui", "Views and the UI kit", Guides, "docs/ui.md"),
    page!("grid", "The data grid", Guides, "docs/grid.md"),
    page!(
        "relations",
        "Models and relations",
        Guides,
        "docs/relations.md"
    ),
    page!("types", "Field types", Guides, "docs/types.md"),
    page!(
        "authorization",
        "Authorization and tenants",
        Guides,
        "docs/authorization.md"
    ),
    page!("queue", "The queue", Guides, "docs/queue.md"),
    page!("mail", "Mail and notifications", Guides, "docs/mail.md"),
    page!(
        "scheduling",
        "Scheduler, events, cache",
        Guides,
        "docs/scheduling.md"
    ),
    page!("testing", "Testing", Guides, "docs/testing.md"),
    page!("postgresql", "PostgreSQL", Guides, "docs/postgresql.md"),
    page!(
        "operations",
        "Running in production",
        Guides,
        "docs/operations.md"
    ),
    page!(
        "development",
        "Faster builds",
        Guides,
        "docs/development.md"
    ),
    page!(
        "stability",
        "Stability and versions",
        Project,
        "docs/stability.md"
    ),
    page!("changelog", "Changelog", Project, "CHANGELOG.md"),
    page!("contributing", "Contributing", Project, "CONTRIBUTING.md"),
    page!("security", "Security", Project, "SECURITY.md"),
    page!("releasing", "Releasing", Project, "RELEASING.md"),
];

/// The page with this slug.
pub fn page(slug: &str) -> Option<&'static Page> {
    PAGES.iter().find(|page| page.slug == slug)
}

/// The page made from this repository file (`docs/routing.md`), for links.
pub fn page_for_path(path: &str) -> Option<&'static Page> {
    PAGES.iter().find(|page| page.path == path)
}
