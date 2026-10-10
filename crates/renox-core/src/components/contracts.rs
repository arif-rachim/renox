//! The component table (Decision 6 of #372): what each `rx-*` tag promises. A static Rust table,
//! so `view:check`, the editor data file and the plugins can read it without parsing anything.

use super::attrs::Kind;

/// How a component turns into MiniJinja.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum Render {
    /// A call to a macro of a template module.
    Macro {
        /// The template the macro is imported from.
        module: &'static str,
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
    doc: "The content.",
};

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
];
