//    This file is part of org-rs.
//
//    org-rs is free software: you can redistribute it and/or modify
//    it under the terms of the GNU General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    org-rs is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU General Public License for more details.
//
//    You should have received a copy of the GNU General Public License
//    along with org-rs.  If not, see <https://www.gnu.org/licenses/>.

// https://orgmode.org/worg/dev/org-element-api.html
// API page lists LineBreak as element, when both org-syntax page and source code list is as object

use crate::{
    affiliated::AffiliatedData,
    babel::BabelCallData,
    blocks::{
        CommentBlockData, DynamicBlockData, ExampleBlockData, ExportBlockData, SpecialBlockData,
        SrcBlockData,
    },
    data::Syntax::BabelCall,
    drawer::DrawerData,
    headline::{HeadlineData, InlineTaskData, NodePropertyData},
    keyword::KeywordData,
    latex::{LatexEnvironmentData, LatexFragmentData},
    list::{ItemData, PlainListData},
    markup::{CommentData, FixedWidthData, FootnoteDefinitionData},
    table::{TableData, TableRowData},
};

use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

use regex::Regex;
use strum_macros::EnumDiscriminants;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    pub start: usize,
    pub end: usize,
}

/// ParseTree node.
/// https://orgmode.org/worg/dev/org-element-api.html#attributes
/// Should be bound to the underlying rope's lifetime
#[derive(Debug)]
pub struct SyntaxNode<'a> {
    /// Parent node.
    pub parent: RefCell<Option<Weak<SyntaxNode<'a>>>>,
    /// Child nodes of this node.
    pub children: RefCell<Vec<Rc<SyntaxNode<'a>>>>,

    pub data: Syntax<'a>,

    /// holds `begin` and `end`
    pub location: Interval,

    /// holds `contents_begin` and `contents_end`
    pub content_location: Option<Interval>,

    /// Holds the number of blank lines, or white spaces, at its end
    /// As a consequence whitespaces or newlines after an element or object
    /// still belong to it. To put it differently,
    /// `location.end` property of an element matches `location.begin` property
    /// of the following one at the same level, if any.
    pub post_blank: usize,

    /// Affiliated keywords
    pub affiliated: Option<AffiliatedData<'a>>,
}

impl<'a> SyntaxNode<'a> {
    pub fn create_root() -> SyntaxNode<'a> {
        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::OrgData,
            location: Interval { start: 0, end: 0 },
            content_location: None,
            post_blank: 0,
            affiliated: None,
        }
    }

    /// Creates an iterator over the node and its direct and indirect
    /// children, in pre-order.
    pub fn nodes(self: &Rc<SyntaxNode<'a>>) -> Nodes<'a> {
        Nodes::new(self.clone())
    }

    /// Creates a `SyntaxNode` corosponding to a raw string used as an
    /// element in elisp.
    pub fn create_raw_at(content: &'a str, interval: Interval) -> SyntaxNode<'a> {
        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(Vec::new()),
            data: Syntax::PlainText(content),
            location: interval,
            content_location: None,
            post_blank: 0,
            affiliated: None,
        }
    }

    /// Appends a child to the node, setting the child's parent correctly.
    pub fn append_child(self: &Rc<SyntaxNode<'a>>, child: Rc<SyntaxNode<'a>>) {
        *child.parent.borrow_mut() = Some(Rc::downgrade(&self));
        self.children.borrow_mut().push(child);
    }

    /// Create a fallback paragraph node spanning to the end of the current line.
    ///
    /// Used for element parsers that are not yet implemented, so that parsing
    /// can continue past the unrecognised content without panicking.
    pub fn fallback(input: &str, start: usize, limit: usize) -> SyntaxNode<'a> {
        let end = input[start..limit]
            .find('\n')
            .map_or(limit, |i| (start + i + 1).min(limit));
        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::Paragraph,
            location: Interval { start, end },
            content_location: None,
            post_blank: 0,
            affiliated: None,
        }
    }
}

/// An enumerated list of all Org syntactic units.
/// This structure is not meant to be extended.
#[derive(Debug, EnumDiscriminants)]
#[strum_discriminants(name(SyntaxT))]
pub enum Syntax<'a> {
    /// The root of the parse tree
    OrgData,

    /// Element
    BabelCall(Box<BabelCallData<'a>>),

    /// Greater element
    CenterBlock,

    /// Element
    Clock(Box<ClockData<'a>>),

    /// Element
    Comment(Box<CommentData<'a>>),

    /// Element
    CommentBlock(Box<CommentBlockData<'a>>),

    /// Element
    DiarySexp(Box<DiarySexpData<'a>>),

    /// Greater element
    Drawer(Box<DrawerData<'a>>),

    /// Greater element
    DynamicBlock(Box<DynamicBlockData<'a>>),

    /// Element
    ExampleBlock(Box<ExampleBlockData<'a>>),

    /// Element
    ExportBlock(Box<ExportBlockData<'a>>),

    /// Element
    FixedWidth(Box<FixedWidthData<'a>>),

    /// Greater element
    FootnoteDefinition(Box<FootnoteDefinitionData<'a>>),

    /// Greater element
    /// In addition to the following list, any property specified
    /// in a property drawer attached to the headline will be
    /// accessible as an attribute (with an uppercase name, e.g. CUSTOM_ID).
    Headline(Box<HeadlineData<'a>>),

    /// Element
    HorizontalRule,

    /// Greater element
    /// In addition to the following list, any property specified
    /// in a property drawer attached to the headline
    /// will be accessible as an attribute
    /// (with an uppercase name, e.g. CUSTOM_ID).
    InlineTask(Box<InlineTaskData<'a>>),

    /// Greater element
    Item(Box<ItemData<'a>>),

    /// Element
    /// Keywords follow the syntax:
    /// ```org
    ///   #+KEY: VALUE
    /// ```
    /// KEY can contain any non-whitespace character, but it cannot be equal to “CALL” or any affiliated keyword.<br>
    /// VALUE can contain any character excepted a new line.<br>
    /// If KEY belongs to org-element-document-properties, VALUE can contain objects.
    Keyword(Box<KeywordData<'a>>),

    /// Element
    /// An inline Latex environment element
    LatexEnvironment(Box<LatexEnvironmentData<'a>>),

    /// Element
    /// Node properties can only exist in property drawers
    NodeProperty(Box<NodePropertyData<'a>>),

    /// Element containing objects.
    Paragraph,

    /// Greater element
    PlainList(Box<PlainListData>),

    /// Element
    Planning(Box<PlanningData<'a>>),

    /// Greater Element
    PropertyDrawer,

    /// Greater element
    QuoteBlock,

    /// Greater element
    Section,

    /// Greater element
    SpecialBlock(Box<SpecialBlockData<'a>>),

    /// Element
    SrcBlock(Box<SrcBlockData<'a>>),

    /// Greater element
    Table(Box<TableData<'a>>),

    /// Element containing objects.
    TableRow(Box<TableRowData>),

    /// Element containing objects.
    VerseBlock,

    /// Recursive object
    Bold,

    /// Object.
    Code(Box<CodeData<'a>>),

    /// Object
    Entity(Box<EntityData<'a>>),

    /// Object
    ExportSnippet(Box<ExportSnippetData<'a>>),

    /// Recursive object.
    FootnoteReference(Box<FootnoteReferenceData<'a>>),

    /// Object
    InlineBabelCall(Box<InlineBabelCallData<'a>>),

    /// Object
    InlineSrcBlock(Box<InlineSrcBlockData<'a>>),

    /// Recursive object.
    Italic,

    LineBreak,

    /// Object
    LatexFragment(Box<LatexFragmentData<'a>>),

    /// Recursive object.
    Link(Box<LinkData<'a>>),

    /// Object
    Macro(Box<MacroData<'a>>),

    /// Recursive object.
    RadioTarget(Box<RadioTargetData<'a>>),

    /// Object
    StatisticsCookie(Box<StatisticsCookieData<'a>>),

    /// Recursive object.
    StrikeThrough,

    /// Recursive object.
    Subscript(Box<SubscriptData>),

    /// Recursive object.
    Superscript(Box<SuperscriptData>),

    /// Recursive object
    TableCell,

    /// Object
    Target(Box<TargetData<'a>>),

    /// Object
    Timestamp(Box<TimestampData<'a>>),

    /// Recursive object.
    Underline,

    /// Object
    Verbatim(Box<VerbatimData<'a>>),

    /// Special object
    PlainText(&'a str),
}

impl SyntaxT {
    pub fn is_greater_element(self) -> bool {
        use SyntaxT::*;
        match self {
            CenterBlock | Drawer | DynamicBlock | FootnoteDefinition | Headline | InlineTask
            | Item | PlainList | PropertyDrawer | QuoteBlock | Section | SpecialBlock | Table => {
                true
            }
            _ => false,
        }
    }

    fn is_element(self) -> bool {
        use SyntaxT::*;
        match self {
            BabelCall | CenterBlock | Clock | Comment | CommentBlock | DiarySexp | Drawer
            | DynamicBlock | ExampleBlock | ExportBlock | FixedWidth | FootnoteDefinition
            | Headline | HorizontalRule | InlineTask | Item | Keyword | LatexEnvironment
            | NodeProperty | Paragraph | PlainList | Planning | PropertyDrawer | QuoteBlock
            | Section | SpecialBlock | SrcBlock | Table | TableRow | VerseBlock => true,
            _ => false,
        }
    }

    fn is_object(self) -> bool {
        use SyntaxT::*;
        match self {
            Bold | Code | Entity | ExportSnippet | FootnoteReference | InlineBabelCall
            | InlineSrcBlock | Italic | LineBreak | LatexFragment | Link | Macro | RadioTarget
            | StatisticsCookie | StrikeThrough | Subscript | Superscript | TableCell | Target
            | Timestamp | Underline | Verbatim | PlainText => true,
            _ => false,
        }
    }

    fn is_recursive_object(self) -> bool {
        use SyntaxT::*;
        match self {
            Bold | FootnoteReference | Italic | Link | RadioTarget | StrikeThrough | Subscript
            | Superscript | TableCell | Underline => true,
            _ => false,
        }
    }

    fn is_object_container(self) -> bool {
        use SyntaxT::*;
        match self {
            Paragraph | TableRow | VerseBlock | Bold | FootnoteReference | Italic | Link
            | RadioTarget | StrikeThrough | Subscript | Superscript | TableCell | Underline => true,
            _ => false,
        }
    }

    fn is_container(self) -> bool {
        self.is_greater_element() || self.is_object_container()
    }

    /// Corresponds to `defconst org-element-object-restrictions` in org-element.el
    /// Original doc:
    /// "Alist of objects restrictions.
    /// key is an element or object type containing objects and value is
    /// a list of types that can be contained within an element or object
    /// of such type.
    /// For example, in a `radio-target' object, one can only find
    /// entities, latex-fragments, subscript, superscript and text
    /// markup.
    /// This alist also applies to secondary string.  For example, an
    /// `headline' type element doesn't directly contain objects, but
    /// still has an entry since one of its properties (`:title') does.")
    pub fn can_contain(self, that: SyntaxT) -> bool {
        /// (standard-set (remq 'table-cell org-element-all-objects))
        fn is_from_standard_set(that: SyntaxT) -> bool {
            match that {
                SyntaxT::TableCell => false,
                x if x.is_object() => true,
                _ => false,
            }
        }

        /// (standard-set-no-line-break (remq 'line-break standard-set)))
        fn is_from_standard_set_no_line_break(that: SyntaxT) -> bool {
            match that {
                SyntaxT::LineBreak => false,
                x => is_from_standard_set(x),
            }
        }

        use SyntaxT::*;
        match self {
            // ((bold ,@standard-set)
            // (italic ,@standard-set)
            // (footnote-reference ,@standard-set)
            // (paragraph ,@standard-set)
            // (strike-through ,@standard-set)
            // (subscript ,@standard-set)
            // (superscript ,@standard-set)
            //(verse-block ,@standard-set)))
            //(underline ,@standard-set)
            Bold | Italic | FootnoteReference | Paragraph | StrikeThrough | Subscript
            | Superscript | Underline | VerseBlock => is_from_standard_set(that),

            // (headline ,@standard-set-no-line-break)
            // (inlinetask ,@standard-set-no-line-break)
            // (item ,@standard-set-no-line-break)
            Headline | InlineTask | Item => is_from_standard_set_no_line_break(that),

            // (keyword ,@(remq 'footnote-reference standard-set))
            Keyword => match that {
                FootnoteReference => false,
                x => is_from_standard_set(x),
            },

            // Ignore all links in a link description.  Also ignore
            // radio-targets and line breaks.
            // (link bold code entity export-snippet
            //       inline-babel-call inline-src-block italic
            //       latex-fragment macro statistics-cookie
            //       strike-through subscript superscript
            //       underline verbatim)
            Link => match that {
                Bold | Code | Entity | ExportSnippet | InlineBabelCall | InlineSrcBlock
                | Italic | LatexFragment | Macro | StatisticsCookie | StrikeThrough | Subscript
                | Superscript | Underline | Verbatim => true,
                _ => false,
            },

            // Remove any variable object from radio target as it would
            // prevent it from being properly recognized.
            // (radio-target bold code entity italic
            //               latex-fragment strike-through
            //               subscript superscript underline)
            RadioTarget => match that {
                Bold | Code | Entity | Italic | LatexFragment | StrikeThrough | Subscript
                | Superscript | Underline => true,
                _ => false,
            },

            // Ignore inline babel call and inline source block as formulas
            // are possible.  Also ignore line breaks and statistics
            // cookies.
            // (table-cell bold code entity export-snippet footnote-reference italic
            //             latex-fragment link macro radio-target strike-through
            //             subscript superscript target timestamp underline verbatim)
            TableCell => match that {
                Bold | Code | Entity | ExportSnippet | FootnoteReference | Italic
                | LatexFragment | Link | Macro | RadioTarget | StrikeThrough | Subscript
                | Superscript | Target | Timestamp | Underline | Verbatim => true,
                _ => false,
            },

            //(table-row table-cell)
            TableRow => match that {
                TableCell => true,
                _ => false,
            },

            _ => false,
        }
    }
}

/// Some elements can contain objects directly in their value fields
pub enum StringOrObject<'a> {
    Raw(Cow<'a, str>),
    Parsed(SyntaxNode<'a>),
}

impl<'a> core::fmt::Debug for StringOrObject<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            StringOrObject::Raw(raw) => write!(f, "Raw: {:?}", raw),
            StringOrObject::Parsed(p) => unimplemented!(),
        }
    }
}

impl<'a> PartialEq for StringOrObject<'a> {
    fn eq(&self, other: &StringOrObject) -> bool {
        match self {
            StringOrObject::Raw(raw) => match other {
                StringOrObject::Parsed(..) => false,
                StringOrObject::Raw(rhs) => raw.eq(rhs),
            },
            StringOrObject::Parsed(p) => unimplemented!(),
        }
    }
}

#[derive(Debug)]
pub struct ClockData<'a> {
    /// Clock duration for a closed clock, or nil (string or nil).
    pub duration: &'a str,

    /// Status of current clock (symbol closed or running).
    pub status: ClockStatus,

    /// Raw clock line value (for simple parsing).
    pub raw: &'a str,
}

#[derive(Debug)]
pub enum ClockStatus {
    Running,
    Closed,
}

#[derive(Debug)]
pub struct DiarySexpData<'a> {
    /// Full Sexp (string).
    pub value: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub enum LineNumberingMode {
    New,
    Continued,
}

#[derive(Debug)]
pub struct PlanningData<'a> {
    /// Timestamp associated to closed keyword, if any
    /// (timestamp object or nil).
    closed: Option<TimestampData<'a>>,

    /// Timestamp associated to deadline keyword, if any
    /// (timestamp object or nil).
    deadline: Option<TimestampData<'a>>,

    /// Timestamp associated to scheduled keyword, if any
    /// (timestamp object or nil).
    scheduled: Option<TimestampData<'a>>,
}

#[derive(Debug)]
pub struct CodeData<'a> {
    /// Contents (string).
    pub value: &'a str,
}

#[derive(Debug)]
pub struct EntityData<'a> {
    /// Entity's ASCII representation (string).
    ascii: &'a str,

    /// Entity's HTML representation (string).
    html: &'a str,

    /// Entity's LaTeX representation (string).
    latex: &'a str,

    /// Non-nil if entity's LaTeX representation should be
    /// in math mode (boolean).
    latex_math_p: bool,

    /// Entity's Latin-1 encoding representation (string).
    latin1: &'a str,

    /// Entity's name, without backslash nor brackets (string).
    name: &'a str,

    /// Non-nil if entity is written with optional
    /// brackets in original buffer (boolean).
    use_brackets_p: bool,

    /// Entity's UTF-8 encoding representation (string).
    utf_8: &'a str,
}

impl<'a> EntityData<'a> {
    pub fn new(name: &'a str) -> Option<Self> {
        // Common entity names and their properties
        // This is a basic mapping - full implementation would need all org entities
        let (ascii, html, latex, latex_math_p, latin1, utf_8) = match name {
            "alpha" => ("\\alpha", "&alpha;", "\\alpha", true, "α", "α"),
            "beta" => ("\\beta", "&beta;", "\\beta", true, "β", "β"),
            "gamma" => ("\\gamma", "&gamma;", "\\gamma", true, "γ", "γ"),
            "delta" => ("\\delta", "&delta;", "\\delta", true, "δ", "δ"),
            "epsilon" => ("\\epsilon", "&epsilon;", "\\epsilon", true, "ε", "ε"),
            "zeta" => ("\\zeta", "&zeta;", "\\zeta", true, "ζ", "ζ"),
            "eta" => ("\\eta", "&eta;", "\\eta", true, "η", "η"),
            "theta" => ("\\theta", "&theta;", "\\theta", true, "θ", "θ"),
            "iota" => ("\\iota", "&iota;", "\\iota", true, "ι", "ι"),
            "kappa" => ("\\kappa", "&kappa;", "\\kappa", true, "κ", "κ"),
            "lambda" => ("\\lambda", "&lambda;", "\\lambda", true, "λ", "λ"),
            "mu" => ("\\mu", "&mu;", "\\mu", true, "μ", "μ"),
            "nu" => ("\\nu", "&nu;", "\\nu", true, "ν", "ν"),
            "xi" => ("\\xi", "&xi;", "\\xi", true, "ξ", "ξ"),
            "pi" => ("\\pi", "&pi;", "\\pi", true, "π", "π"),
            "rho" => ("\\rho", "&rho;", "\\rho", true, "ρ", "ρ"),
            "sigma" => ("\\sigma", "&sigma;", "\\sigma", true, "σ", "σ"),
            "tau" => ("\\tau", "&tau;", "\\tau", true, "τ", "τ"),
            "upsilon" => ("\\upsilon", "&upsilon;", "\\upsilon", true, "υ", "υ"),
            "phi" => ("\\phi", "&phi;", "\\phi", true, "φ", "φ"),
            "chi" => ("\\chi", "&chi;", "\\chi", true, "χ", "χ"),
            "psi" => ("\\psi", "&psi;", "\\psi", true, "ψ", "ψ"),
            "omega" => ("\\omega", "&omega;", "\\omega", true, "ω", "ω"),
            "deg" => ("\\deg", "&deg;", "^\\circ", false, "°", "°"),
            "plusmn" => ("\\plusmn", "&plusmn;", "\\pm", false, "±", "±"),
            "times" => ("\\times", "&times;", "\\times", false, "×", "×"),
            "divide" => ("\\divide", "&divide;", "\\div", false, "÷", "÷"),
            "leq" => ("\\leq", "&le;", "\\leq", false, "≤", "≤"),
            "geq" => ("\\geq", "&ge;", "\\geq", false, "≥", "≥"),
            "neq" => ("\\neq", "&ne;", "\\neq", false, "≠", "≠"),
            "pm" => ("\\pm", "&plusmn;", "\\pm", false, "±", "±"),
            "cdot" => ("\\cdot", "&middot;", "\\cdot", false, "·", "·"),
            "rightarrow" => ("\\rightarrow", "&rarr;", "\\rightarrow", false, "→", "→"),
            "leftarrow" => ("\\leftarrow", "&larr;", "\\leftarrow", false, "←", "←"),
            "Rightarrow" => ("\\Rightarrow", "&rArr;", "\\Rightarrow", false, "⇒", "⇒"),
            "Leftarrow" => ("\\Leftarrow", "&lArr;", "\\Leftarrow", false, "⇐", "⇐"),
            "leftrightarrow" => ("\\leftrightarrow", "&harr;", "\\leftrightarrow", false, "↔", "↔"),
            "copyright" => ("\\copyright", "&copy;", "\\copyright", false, "©", "©"),
            "trade" => ("\\trade", "&trade;", "\\texttrademark", false, "™", "™"),
            "registered" => ("\\registered", "&reg;", "\\textregistered", false, "®", "®"),
            "nbsp" => ("\\nbsp", "&nbsp;", "~", false, " ", " "),
            "8211" => ("\\8211", "&#8211;", "--", false, "–", "–"),
            "8212" => ("\\8212", "&#8212;", "---", false, "—", "—"),
            "8220" => ("\\8220", "&#8220;", "``", false, "\u{201C}", "\u{201C}"),
            "8221" => ("\\8221", "&#8221;", "''", false, "\u{201D}", "\u{201D}"),
            _ => return None,
        };

        Some(EntityData {
            ascii,
            html,
            latex,
            latex_math_p,
            latin1,
            name,
            use_brackets_p: false,
            utf_8,
        })
    }
}

#[derive(Debug)]
pub struct ExportSnippetData<'a> {
    /// Relative back_end's name (string).
    back_end: &'a str,

    /// Export code (string).
    value: &'a str,
}

/// Recursive object.
#[derive(Debug)]
pub struct FootnoteReferenceData<'a> {
    /// Footnote's label, if any (string or nil).
    label: Option<&'a str>,

    /// Determine whether reference has its
    /// definition inline, or not (symbol inline, standard).
    type_s: &'a str,
}

#[derive(Debug)]
pub struct InlineBabelCallData<'a> {
    ///Name of code block being called (string).
    call: &'a str,

    ///Header arguments applied to the named code block (string or nil).
    inside_header: Option<&'a str>,

    ///Arguments passed to the code block (string or nil).
    arguments: Option<&'a str>,

    ///Header arguments applied to the calling instance (string or nil).
    end_header: Option<&'a str>,

    ///Raw call, as Org syntax (string).
    value: &'a str,
}

#[derive(Debug)]
pub struct InlineSrcBlockData<'a> {
    ///Language of the code in the block (string).
    language: &'a str,

    ///Optional header arguments (string or nil).
    parameters: Option<&'a str>,

    ///Source code (string).
    value: &'a str,
}

#[derive(Debug)]
pub enum LinkFormat {
    Plain,
    Angle,
    Bracket,
}

#[derive(Debug)]
pub struct LinkData<'a> {
    /// Name of application requested to open the link
    /// in Emacs (string or nil).
    /// It only applies to "file" type links.
    application: Option<&'a str>,

    /// Format for link syntax (symbol plain, angle, bracket).
    format: LinkFormat,

    /// Identifier for link's destination.
    /// It is usually the link part with type,
    /// if specified, removed (string).
    path: &'a str,

    ///Uninterpreted link part (string).
    raw_link: &'a str,

    /// Additional information for file location (string or nil).
    /// It only applies to "file" type links.
    search_option: Option<&'a str>,

    /// Link type
    link_type: LinkType,
}

impl<'a> LinkData<'a> {
    pub fn new(raw: &'a str) -> Self {
        // Simple link parser - extract type from scheme if present
        // e.g., "https://example.com" -> link_type = File, path = "https://example.com"
        let (link_type, path) = if raw.starts_with("http://") || raw.starts_with("https://") {
            (LinkType::File, raw)
        } else if raw.starts_with("file:") {
            (LinkType::File, raw.strip_prefix("file:").unwrap_or(raw))
        } else {
            (LinkType::Fuzzy, raw)
        };

        LinkData {
            application: None,
            format: LinkFormat::Bracket,
            path,
            raw_link: raw,
            search_option: None,
            link_type,
        }
    }
}

#[derive(Debug)]
pub enum LinkType {
    /// Line in some source code,
    Coderef,

    ///Specific headline's custom-id,
    CustomId,

    /// External file,
    File,

    /// Target, referring to a target object, a named element or a headline in the current parse tree,
    Fuzzy,

    /// Specific headline's id,
    Id,

    /// Radio-target.
    Radio,
}

#[derive(Debug)]
pub struct MacroData<'a> {
    /// Arguments passed to the macro (list of strings).
    args: Vec<&'a str>,

    /// Macro's name (string).
    key: &'a str,

    /// Replacement text (string).
    value: &'a str,
}

#[derive(Debug)]
pub struct RadioTargetData<'a> {
    /// Uninterpreted contents (string).
    raw_value: &'a str,
}

#[derive(Debug)]
pub struct StatisticsCookieData<'a> {
    /// Full cookie (string).
    value: &'a str,
}

#[derive(Debug)]
pub struct SubscriptData {
    /// Non_nil if contents are enclosed in curly brackets (t, nil).
    use_brackets_p: bool,
}

/// Recursive object.
#[derive(Debug)]
pub struct SuperscriptData {
    /// Non_nil if contents are enclosed in curly brackets (t, nil).
    use_brackets_p: bool,
}

#[derive(Debug)]
pub struct TargetData<'a> {
    ///Target's ID (string).
    value: &'a str,
}

impl<'a> TargetData<'a> {
    pub fn new(value: &'a str) -> Self {
        TargetData { value }
    }
}

#[derive(Debug)]
pub struct TimestampData<'a> {
    /// Day part from timestamp end.
    /// If no ending date is defined, it defaults to start day part (integer).
    day_end: usize,

    /// Day part from timestamp start (integer).
    day_start: usize,

    /// Hour part from timestamp end.
    /// If no ending date is defined, it defaults to start hour part,
    /// if any (integer or nil).
    hour_end: Option<usize>,

    /// Hour part from timestamp start, if specified (integer or nil).
    hour_start: Option<usize>,

    /// Minute part from timestamp end.
    /// If no ending date is defined, it defaults to start minute part,
    /// if any (integer or nil).
    minute_end: Option<usize>,

    /// Minute part from timestamp start, if specified (integer or nil).
    minute_start: Option<usize>,

    /// Month part from timestamp end.
    /// If no ending date is defined, it defaults to start month part
    /// (integer).
    month_end: usize,

    /// Month part from timestamp start (integer).
    month_start: usize,

    /// Raw timestamp (string).
    raw_value: &'a str,

    // TODO maybe the following three fields can be combined into one
    /// Type of repeater, if any (symbol catch_up, restart, cumulate or nil)
    repeater_type: Option<RepeaterType>,

    /// Unit of shift, if a repeater is defined
    /// (symbol year, month, week, day, hour or nil).
    repeater_unit: Option<TimeUnit>,

    /// Value of shift, if a repeater is defined (integer or nil).
    repeater_value: Option<usize>,

    /// Type of timestamp:
    /// (symbol active, active_range, diary, inactive, inactive_range).
    type_s: TimestampType,

    /// Type of warning, if any (symbol all, first or nil)
    warning_type: Option<WarningType>,

    /// Unit of delay, if one is defined
    /// (symbol year, month, week, day, hour or nil).
    warning_unit: Option<TimeUnit>,

    /// Value of delay, if one is defined (integer or nil).
    warning_value: Option<usize>,

    /// Year part from timestamp end.
    /// If no ending date is defined, it defaults to start year part (integer)
    year_end: usize,

    /// Year part from timestamp start (integer).
    year_start: usize,
}

impl<'a> TimestampData<'a> {
    pub fn new(raw: &'a str) -> Option<Self> {
        let raw = raw.trim();
        if raw.len() < 9 {
            return None;
        }

        // Simple active timestamp: <2023-12-31>
        // Can be:
        // - <2023-12-31>
        // - <2023-12-31 10:30>
        // - <2023-12-31 10:30-11:30>
        // - [2023-12-31] (inactive)

        // Find year, month, day
        let parts: Vec<&str> = raw.split('-').collect();
        if parts.len() < 3 {
            return None;
        }

        let year_str = parts[0].trim_start_matches('<').trim_start_matches('[');
        let year_start: usize = year_str.parse().ok()?;
        let month_start: usize = parts[1].parse().ok()?;
        let day_str = parts[2].split_whitespace().next().unwrap_or("1");
        let day_start: usize = day_str.trim_end_matches('>').trim_end_matches(']').parse().ok()?;

        // Try to parse time if present
        let mut hour_start = None;
        let mut minute_start = None;
        let rest = parts[2].split_whitespace().collect::<Vec<_>>();
        if rest.len() > 1 {
            if let Some(time_part) = rest.get(1) {
                let time_parts: Vec<&str> = time_part.split(':').collect();
                if time_parts.len() >= 2 {
                    hour_start = time_parts[0].parse().ok();
                    minute_start = time_parts[1].trim_end_matches(|c| c == '>' && c == ']').parse().ok();
                }
            }
        }

        Some(TimestampData {
            day_end: day_start,
            day_start,
            hour_end: hour_start,
            hour_start,
            minute_end: minute_start,
            minute_start,
            month_end: month_start,
            month_start,
            raw_value: raw,
            repeater_type: None,
            repeater_unit: None,
            repeater_value: None,
            type_s: if raw.starts_with('<') {
                TimestampType::Active
            } else {
                TimestampType::Inactive
            },
            warning_type: None,
            warning_unit: None,
            warning_value: None,
            year_end: year_start,
            year_start,
        })
    }
}

#[derive(Debug)]
pub enum WarningType {
    All,
    First,
}

#[derive(Debug)]
pub enum TimestampType {
    Active,
    ActiveRange,
    Diary,
    Inactive,
    InactiveRange,
}

#[derive(Debug)]
pub enum RepeaterType {
    CatchUp,
    Restart,
    Cumulate,
}

#[derive(Debug)]
pub enum TimeUnit {
    Year,
    Month,
    Week,
    Day,
    Hour,
}

#[derive(Debug)]
pub struct VerbatimData<'a> {
    ///Contents (string).
    pub value: &'a str,
}

/// A pre-order traversal of a [`SyntaxNode`].
pub struct Nodes<'a> {
    index_stack: Vec<usize>,
    current_node: Rc<SyntaxNode<'a>>,
}

impl<'a> Nodes<'a> {
    fn new(handle: Rc<SyntaxNode<'a>>) -> Nodes<'a> {
        Nodes {
            index_stack: vec![0],
            current_node: handle,
        }
    }
}

impl<'a> Iterator for Nodes<'a> {
    type Item = Rc<SyntaxNode<'a>>;

    fn next(&mut self) -> Option<Rc<SyntaxNode<'a>>> {
        let node = self.current_node.clone();

        if self.index_stack.is_empty() {
            return None;
        }

        while self.current_node.children.borrow().len() <= *self.index_stack.last().unwrap() {
            self.index_stack.pop();
            if self.index_stack.is_empty() {
                return Some(node);
            }
            let next = self
                .current_node
                .parent
                .borrow()
                .as_ref()
                .expect("An iterated parse tree was mutated while an iterator was alive.")
                .upgrade()
                .expect(
                    "An iterated parse tree had its parents deallocated while an iterator is alive",
                )
                .clone();
            self.current_node = next;
        }

        let top_idx_idx = self.index_stack.len() - 1;
        let next = self.current_node.children.borrow()[self.index_stack[top_idx_idx]].clone();
        self.index_stack[top_idx_idx] += 1;
        self.index_stack.push(0);
        self.current_node = next;

        Some(node)
    }
}

mod test {

    use std::rc::Rc;

    use super::*;

    #[test]
    fn can_contain() {
        let bold = SyntaxT::Bold;
        let br = SyntaxT::LineBreak;
        let verse = SyntaxT::VerseBlock;

        fn closure_test(that: SyntaxT, restriction: impl Fn(SyntaxT) -> bool) -> bool {
            restriction(that)
        }

        // TODO find out a way to satisfy grumpy borrow checker and have can_contain method return
        // a lambda and do not get a brain damage from lifetimes
        assert!(!bold.can_contain(SyntaxT::VerseBlock));
        assert!(bold.can_contain(SyntaxT::LineBreak));
        assert!(closure_test(br, |that| bold.can_contain(that)));
        assert!(!closure_test(verse, |that| bold.can_contain(that)));
    }

    #[test]
    fn nodes_iter_with_a_single_element_returns_that_element() {
        let node = Rc::new(SyntaxNode::create_root());
        let out_nodes = node.clone().nodes().collect::<Vec<_>>();
        assert_eq!(out_nodes.len(), 1);
        assert!(Rc::ptr_eq(&out_nodes[0], &node));
    }

    #[test]
    fn nodes_iter_with_several_children_return_all() {
        let parent = Rc::new(SyntaxNode::create_root());
        const NUM_CHILDREN: usize = 4;
        let children = std::iter::repeat(())
            .take(NUM_CHILDREN)
            .map(|_| Rc::new(SyntaxNode::create_root()))
            .collect::<Vec<_>>();
        for child in &children {
            parent.append_child(child.clone());
        }

        let results = parent.nodes().collect::<Vec<_>>();
        dbg!(&results);

        assert_eq!(results.len(), NUM_CHILDREN + 1);
        assert!(Rc::ptr_eq(&parent, &results[0]));

        for (idx, child) in children.iter().enumerate() {
            assert!(
                Rc::ptr_eq(&child, &results[idx + 1]),
                "Pointer did not match (idx = {})",
                idx + 1
            );
        }
    }

    #[test]
    fn nodes_iter_with_several_layers_return_all() {
        const LEVELS: usize = 4;
        let nodes = std::iter::repeat(())
            .take(LEVELS)
            .map(|_| Rc::new(SyntaxNode::create_root()))
            .collect::<Vec<_>>();
        for (idx, node) in nodes.iter().enumerate() {
            if idx == 0 {
                continue;
            }
            nodes[idx - 1].append_child(node.clone());
        }

        let results = nodes[0].nodes().collect::<Vec<_>>();

        assert_eq!(results.len(), nodes.len());

        for (result, input) in results.iter().zip(nodes.iter()) {
            assert!(Rc::ptr_eq(result, input));
        }
    }
}
