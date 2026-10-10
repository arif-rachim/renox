//! The component table (Decision 6 of #372): what each `rx-*` tag promises. A static Rust table,
//! so `view:check`, the editor data file and the plugins can read it without parsing anything.

use super::attrs::Kind;

/// How a component turns into MiniJinja.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum Render {
    /// A call to a macro of a template module.
    Macro {
        /// The template module the macro is imported from.
        module: Module,
        /// The macro's name.
        name: &'static str,
    },
    /// A plain element: the kit has CSS classes for it, not a macro.
    Element {
        /// The HTML tag written.
        tag: &'static str,
        /// The base class.
        class: &'static str,
    },
    /// Code generation of its own.
    Special(Special),
}

/// A built-in template module whose macros components call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Module {
    /// `renox/ui.html`.
    Ui,
    /// `renox/pagination.html`.
    #[allow(dead_code)]
    Pagination,
}

impl Module {
    /// The template name.
    pub(crate) fn path(self) -> &'static str {
        match self {
            Module::Ui => "renox/ui.html",
            Module::Pagination => "renox/pagination.html",
        }
    }

    /// The name the module is imported as.
    pub(crate) fn alias(self) -> &'static str {
        match self {
            Module::Ui => "__rx_ui",
            Module::Pagination => "__rx_pagination",
        }
    }
}

/// The components with code generation of their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum Special {
    /// `rx-page`.
    Page,
    /// `rx-push`.
    Push,
    /// `rx-form`.
    Form,
    /// `rx-table`.
    Table,
    /// `rx-wizard`.
    Wizard,
    /// `rx-wizard-step`, made by its parent.
    WizardStep,
}

/// One attribute a component takes.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(crate) struct Prop {
    /// The name, in kebab case.
    pub name: &'static str,
    /// What the value may be.
    pub kind: Kind,
    /// Whether it must be given.
    pub required: bool,
    /// The words of an `Enum`.
    pub values: &'static [&'static str],
    /// One line about it.
    pub doc: &'static str,
}

/// One place a component takes content.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(crate) struct Slot {
    /// The name; `""` is the default slot.
    pub name: &'static str,
    /// The prop the content fills, if it is not the macro's `caller()`.
    pub into: Option<&'static str>,
    /// The names the content receives (`{% call(row, prefix) %}`).
    pub args: &'static [&'static str],
    /// Whether the macro works without content (it checks `caller is defined`).
    pub optional: bool,
    /// One line about it.
    pub doc: &'static str,
}

/// What a component promises.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(crate) struct Contract {
    /// The tag, such as `rx-stack`.
    pub tag: &'static str,
    /// One line about it.
    pub doc: &'static str,
    /// How it is generated.
    pub render: Render,
    /// The attributes it takes.
    pub props: &'static [Prop],
    /// Where it takes content.
    pub slots: &'static [Slot],
    /// The events it sends.
    pub events: &'static [&'static str],
    /// The prop that `route="name"` fills.
    pub route_prop: Option<&'static str>,
    /// Whether it passes other attributes through.
    pub attrs: bool,
    /// The component it must sit in.
    pub parent: Option<&'static str>,
}

const DEFAULT_SLOT: Slot = Slot {
    name: "",
    into: None,
    args: &[],
    optional: false,
    doc: "The content.",
};

const fn text(name: &'static str, required: bool, doc: &'static str) -> Prop {
    Prop {
        name,
        kind: Kind::Text,
        required,
        values: &[],
        doc,
    }
}

const fn kind_prop(name: &'static str, doc: &'static str) -> Prop {
    Prop {
        name,
        kind: Kind::Enum,
        required: false,
        values: &["info", "success", "warning", "error"],
        doc,
    }
}

/// The components Renox ships.
pub(crate) static BUILTIN: &[Contract] = &[
    Contract {
        tag: "rx-stack",
        doc: "Children stacked in a column with the kit's spacing.",
        render: Render::Element {
            tag: "div",
            class: "rx-stack",
        },
        props: &[],
        slots: &[DEFAULT_SLOT],
        events: &[],
        route_prop: None,
        attrs: true,
        parent: None,
    },
    Contract {
        tag: "rx-row",
        doc: "Children side by side in a row with the kit's spacing.",
        render: Render::Element {
            tag: "div",
            class: "rx-row",
        },
        props: &[Prop {
            name: "end",
            kind: Kind::Bool,
            required: false,
            values: &[],
            doc: "Pushes the children to the end of the row.",
        }],
        slots: &[DEFAULT_SLOT],
        events: &[],
        route_prop: None,
        attrs: true,
        parent: None,
    },
    Contract {
        tag: "rx-page-header",
        doc: "The page's title, with an optional subtitle, back link, badge and actions.",
        render: Render::Macro {
            module: Module::Ui,
            name: "page_header",
        },
        props: &[
            text("title", true, "The page title."),
            text("subtitle", false, "A line under the title."),
            text("back", false, "The URL of a back link."),
            text("back-label", false, "The back link's text."),
            text("badge", false, "A badge next to the title."),
            kind_prop("badge-kind", "The badge's colour."),
        ],
        slots: &[Slot {
            optional: true,
            doc: "The page's actions.",
            ..DEFAULT_SLOT
        }],
        events: &[],
        route_prop: None,
        attrs: false,
        parent: None,
    },
    Contract {
        tag: "rx-card",
        doc: "A titled section on the page.",
        render: Render::Macro {
            module: Module::Ui,
            name: "card",
        },
        props: &[
            text("title", false, "The card's heading."),
            text("subtitle", false, "A line under the heading."),
        ],
        slots: &[DEFAULT_SLOT],
        events: &[],
        route_prop: None,
        attrs: false,
        parent: None,
    },
    Contract {
        tag: "rx-toolbar",
        doc: "A row of buttons and filters above a list.",
        render: Render::Macro {
            module: Module::Ui,
            name: "toolbar",
        },
        props: &[],
        slots: &[DEFAULT_SLOT],
        events: &[],
        route_prop: None,
        attrs: false,
        parent: None,
    },
    Contract {
        tag: "rx-badge",
        doc: "A small coloured label.",
        render: Render::Macro {
            module: Module::Ui,
            name: "badge",
        },
        props: &[
            text("text", true, "The label; or give it as content."),
            kind_prop("kind", "The colour."),
        ],
        slots: &[Slot {
            into: Some("text"),
            doc: "The label.",
            ..DEFAULT_SLOT
        }],
        events: &[],
        route_prop: None,
        attrs: false,
        parent: None,
    },
    Contract {
        tag: "rx-page",
        doc: "The whole template: extends a layout, sets the SEO title and fills its blocks.",
        render: Render::Special(Special::Page),
        props: &[
            text("layout", true, "The layout's file name; plain text."),
            text("title", false, "The page title for the SEO tags."),
            text(
                "description",
                false,
                "The page description for the SEO tags.",
            ),
        ],
        slots: &[Slot {
            doc: "The page's content block; `<rx-slot name=\"X\">` fills the layout's block X.",
            ..DEFAULT_SLOT
        }],
        events: &[],
        route_prop: None,
        attrs: false,
        parent: None,
    },
    Contract {
        tag: "rx-push",
        doc: "Content added to a stack of the layout.",
        render: Render::Special(Special::Push),
        props: &[
            text("stack", true, "The stack's name."),
            text("once", false, "A key: the content is added once per page."),
        ],
        slots: &[DEFAULT_SLOT],
        events: &[],
        route_prop: None,
        attrs: false,
        parent: None,
    },
    Contract {
        tag: "rx-wizard",
        doc: "A form in steps; each `<rx-wizard-step>` inside is one step.",
        render: Render::Special(Special::Wizard),
        props: &[
            text(
                "id",
                true,
                "The wizard's id; its steps build theirs from it.",
            ),
            text("submit-label", true, "The last step's submit button."),
            text("back-label", false, "The back button's text."),
            text("next-label", false, "The next button's text."),
            Prop {
                name: "cancel",
                kind: Kind::Bool,
                required: false,
                values: &[],
                doc: "Shows a cancel button that closes the sheet.",
            },
            text("cancel-label", false, "The cancel button's text."),
        ],
        slots: &[Slot {
            doc: "Only `<rx-wizard-step>` elements.",
            ..DEFAULT_SLOT
        }],
        events: &[],
        route_prop: None,
        attrs: false,
        parent: None,
    },
    Contract {
        tag: "rx-wizard-step",
        doc: "One step of a wizard; its fields are the content.",
        render: Render::Special(Special::WizardStep),
        props: &[
            text("key", true, "The step's key, unique in the wizard."),
            text("title", false, "The step's name in the progress list."),
        ],
        slots: &[DEFAULT_SLOT],
        events: &[],
        route_prop: None,
        attrs: false,
        parent: Some("rx-wizard"),
    },
];
