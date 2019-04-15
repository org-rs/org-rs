// https://orgmode.org/worg/dev/org-syntax.html

// https://orgmode.org/worg/dev/org-element-api.html
// API page lists LineBreak as element, when both org-syntax page and source code list is as object

use crate::headline::{HeadlineData, InlineTaskData};
use crate::list::*;
// use regex::Regex;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;
use std::rc::Weak;
use xi_rope::Interval;

/// Reference to a DOM node.
pub type Handle<'a> = Rc<SyntaxNode<'a>>;

/// Weak reference to a DOM node, used for parent pointers.
pub type WeakHandle<'a> = Weak<SyntaxNode<'a>>;


/// ParseTree node.
/// https://orgmode.org/worg/dev/org-element-api.html#attributes
/// Should be bound to the underlying rope's lifetime
pub struct SyntaxNode<'a> {
    /// Parent node.
    pub parent: Cell<Option<WeakHandle<'a>>>,
    /// Child nodes of this node.
    pub children: RefCell<Vec<Handle<'a>>>,

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

    // TODO affiliated keywords stub
    pub affiliated: Option<()>,
}

impl<'a> SyntaxNode<'a> {
    pub fn create_root() -> SyntaxNode<'a> {
        SyntaxNode {
            parent: Cell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::OrgData,
            location: Interval { start: 0, end: 0 },
            content_location: None,
            post_blank: 0,
            affiliated: None,
        }
    }
}

pub trait SyntaxInfo {
    fn is_greater_element(&self) -> bool;
    fn is_element(&self) -> bool;
    fn is_object(&self) -> bool;
    fn is_recursive_object(&self) -> bool;
    fn is_object_container(&self) -> bool;
    fn is_container(&self) -> bool;
    fn can_contain(&self, that: &Syntax) -> bool;
}

/// Complete list of syntax entities
#[derive(EnumDiscriminants)]
#[strum_discriminants(name(SyntaxT))]
pub enum Syntax<'a> {
    /// Root of the parse tree
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
    /// <br>
    /// Keywords follow the syntax:
    /// ```org
    ///   #+KEY: VALUE
    /// ```
    /// KEY can contain any non-whitespace character, but it cannot be equal to “CALL” or any affiliated keyword.<br>
    /// VALUE can contain any character excepted a new line.<br>
    /// If KEY belongs to org-element-document-properties, VALUE can contain objects.
    Keyword(Box<KeywordData<'a>>),

    ///Element
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

    /// Element
    PropertyDrawer(Box<PropertyDrawerData<'a>>),

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

impl<'a> SyntaxInfo for Syntax<'a> {
    #[rustfmt::skip]
    fn is_greater_element(&self) -> bool {
        match self {
            Syntax::CenterBlock             => true,   // Greater element
            Syntax::Drawer(..)              => true,   // Greater element
            Syntax::DynamicBlock(..)        => true,   // Greater element
            Syntax::FootnoteDefinition(..)  => true,   // Greater element
            Syntax::Headline(..)            => true,   // Greater element
            Syntax::InlineTask(..)          => true,   // Greater element
            Syntax::Item(..)                => true,   // Greater element
            Syntax::PlainList(..)           => true,   // Greater element
            Syntax::QuoteBlock              => true,   // Greater element
            Syntax::Section                 => true,   // Greater element
            Syntax::SpecialBlock(..)        => true,   // Greater element
            Syntax::Table(..)               => true,   // Greater element
            _                               => false

        }
    }

    #[rustfmt::skip]
    fn is_element(&self) -> bool {
        match self {
            Syntax::BabelCall(..)           => true,   // Element
            Syntax::CenterBlock             => true,   // Greater element
            Syntax::Clock(..)               => true,   // Element
            Syntax::Comment(..)             => true,   // Element
            Syntax::CommentBlock(..)        => true,   // Element
            Syntax::DiarySexp(..)           => true,   // Element
            Syntax::Drawer(..)              => true,   // Greater element
            Syntax::DynamicBlock(..)        => true,   // Greater element
            Syntax::ExampleBlock(..)        => true,   // Element
            Syntax::ExportBlock(..)         => true,   // Element
            Syntax::FixedWidth(..)          => true,   // Element
            Syntax::FootnoteDefinition(..)  => true,   // Greater element
            Syntax::Headline(..)            => true,   // Greater element
            Syntax::HorizontalRule          => true,   // Element
            Syntax::InlineTask(..)          => true,   // Greater element
            Syntax::Item(..)                => true,   // Greater element
            Syntax::Keyword(..)             => true,   // Element
            Syntax::LatexEnvironment(..)    => true,   // Element
            Syntax::NodeProperty(..)        => true,   // Element
            Syntax::Paragraph               => true,   // Element containing objects.
            Syntax::PlainList(..)           => true,   // Greater element
            Syntax::Planning(..)            => true,   // Element
            Syntax::PropertyDrawer(..)      => true,   // Element
            Syntax::QuoteBlock              => true,   // Greater element
            Syntax::Section                 => true,   // Greater element
            Syntax::SpecialBlock(..)        => true,   // Greater element
            Syntax::SrcBlock(..)            => true,   // Element
            Syntax::Table(..)               => true,   // Greater element
            Syntax::TableRow(..)            => true,   // Element containing objects.
            Syntax::VerseBlock              => true,   // Element containing objects.
            _                               => false

        }
    }

    #[rustfmt::skip]
    fn is_object(&self) -> bool {
        match self {
            Syntax::Bold                    => true,  // Recursive object
            Syntax::Code(..)                => true,  // Object.
            Syntax::Entity(..)              => true,  // Object
            Syntax::ExportSnippet(..)       => true,  // Object
            Syntax::FootnoteReference(..)   => true,  // Recursive object.
            Syntax::InlineBabelCall(..)     => true,  // Object
            Syntax::InlineSrcBlock(..)      => true,  // Object
            Syntax::Italic                  => true,  // Recursive object.
            Syntax::LineBreak               => true,  // Object
            Syntax::LatexFragment(..)       => true,  // Object
            Syntax::Link(..)                => true,  // Recursive object.
            Syntax::Macro(..)               => true,  // Object
            Syntax::RadioTarget(..)         => true,  // Recursive object.
            Syntax::StatisticsCookie(..)    => true,  // Object
            Syntax::StrikeThrough           => true,  // Recursive object.
            Syntax::Subscript(..)           => true,  // Recursive object.
            Syntax::Superscript(..)         => true,  // Recursive object.
            Syntax::TableCell               => true,  // Recursive object
            Syntax::Target(..)              => true,  // Object
            Syntax::Timestamp(..)           => true,  // Object
            Syntax::Underline               => true,  // Recursive object.
            Syntax::Verbatim(..)            => true,  // Object
            Syntax::PlainText(..)           => true,  // Special object
            _                               => false
        }
    }

    #[rustfmt::skip]
    fn is_recursive_object(&self) -> bool {
        match self {
            Syntax::Bold                    => true,  // Recursive object
            Syntax::FootnoteReference(..)   => true,  // Recursive object.
            Syntax::Italic                  => true,  // Recursive object.
            Syntax::Link(..)                => true,  // Recursive object.
            Syntax::RadioTarget(..)         => true,  // Recursive object.
            Syntax::StrikeThrough           => true,  // Recursive object.
            Syntax::Subscript(..)           => true,  // Recursive object.
            Syntax::Superscript(..)         => true,  // Recursive object.
            Syntax::TableCell               => true,  // Recursive object
            Syntax::Underline               => true,  // Recursive object.
            _                               => false
        }
    }

    #[rustfmt::skip]
    fn is_object_container(&self) -> bool {
        match self {
            Syntax::Paragraph               => true,  // Element containing objects.
            Syntax::TableRow(..)            => true,  // Element containing objects.
            Syntax::VerseBlock              => true,  // Element containing objects.
            Syntax::Bold                    => true,  // Recursive object
            Syntax::FootnoteReference(..)   => true,  // Recursive object.
            Syntax::Italic                  => true,  // Recursive object.
            Syntax::Link(..)                => true,  // Recursive object.
            Syntax::RadioTarget(..)         => true,  // Recursive object.
            Syntax::StrikeThrough           => true,  // Recursive object.
            Syntax::Subscript(..)           => true,  // Recursive object.
            Syntax::Superscript(..)         => true,  // Recursive object.
            Syntax::TableCell               => true,  // Recursive object
            Syntax::Underline               => true,  // Recursive object.
            _                               => false
        }
    }

    fn is_container(&self) -> bool {
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
    fn can_contain(&self, that: &Syntax) -> bool
    {
            // (standard-set (remq 'table-cell org-element-all-objects))
            fn is_from_standard_set(that: &Syntax) -> bool {
                match that {
                    Syntax::TableCell => false,
                    x if x.is_object() => true,
                    _ => false,
                }
            }

            /// (standard-set-no-line-break (remq 'line-break standard-set)))
            fn is_from_standard_set_no_line_break(that: &Syntax) -> bool {
                match that {
                    Syntax::LineBreak => false,
                    x => is_from_standard_set(x),
                }
            }

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
                Syntax::Bold
                | Syntax::Italic
                | Syntax::FootnoteReference(..)
                | Syntax::Paragraph
                | Syntax::StrikeThrough
                | Syntax::Subscript(..)
                | Syntax::Superscript(..)
                | Syntax::Underline
                | Syntax::VerseBlock => is_from_standard_set(that),

                // (headline ,@standard-set-no-line-break)
                // (inlinetask ,@standard-set-no-line-break)
                // (item ,@standard-set-no-line-break)
                Syntax::Headline(..) | Syntax::InlineTask(..) | Syntax::Item(..) => {
                    is_from_standard_set_no_line_break(that)
                }

                // (keyword ,@(remq 'footnote-reference standard-set))
                Syntax::Keyword(..) => match that {
                    Syntax::FootnoteReference(..) => false,
                    x => is_from_standard_set(x),
                },

                // Ignore all links in a link description.  Also ignore
                // radio-targets and line breaks.
                // (link bold code entity export-snippet
                //       inline-babel-call inline-src-block italic
                //       latex-fragment macro statistics-cookie
                //       strike-through subscript superscript
                //       underline verbatim)
                Syntax::Link(..) => match that {
                    Syntax::Bold
                    | Syntax::Code(..)
                    | Syntax::Entity(..)
                    | Syntax::ExportSnippet(..)
                    | Syntax::InlineBabelCall(..)
                    | Syntax::InlineSrcBlock(..)
                    | Syntax::Italic
                    | Syntax::LatexFragment(..)
                    | Syntax::Macro(..)
                    | Syntax::StatisticsCookie(..)
                    | Syntax::StrikeThrough
                    | Syntax::Subscript(..)
                    | Syntax::Superscript(..)
                    | Syntax::Underline
                    | Syntax::Verbatim(..) => true,
                    _ => false,
                },

                // Remove any variable object from radio target as it would
                // prevent it from being properly recognized.
                // (radio-target bold code entity italic
                //               latex-fragment strike-through
                //               subscript superscript underline)
                Syntax::RadioTarget(..) => match that {
                    Syntax::Bold
                    | Syntax::Code(..)
                    | Syntax::Entity(..)
                    | Syntax::Italic
                    | Syntax::LatexFragment(..)
                    | Syntax::StrikeThrough
                    | Syntax::Subscript(..)
                    | Syntax::Superscript(..)
                    | Syntax::Underline => true,
                    _ => false,
                },

                // Ignore inline babel call and inline source block as formulas
                // are possible.  Also ignore line breaks and statistics
                // cookies.
                // (table-cell bold code entity export-snippet footnote-reference italic
                //             latex-fragment link macro radio-target strike-through
                //             subscript superscript target timestamp underline verbatim)
                Syntax::TableCell => match that {
                    Syntax::Bold
                    | Syntax::Code(..)
                    | Syntax::Entity(..)
                    | Syntax::ExportSnippet(..)
                    | Syntax::FootnoteReference(..)
                    | Syntax::Italic
                    | Syntax::LatexFragment(..)
                    | Syntax::Link(..)
                    | Syntax::Macro(..)
                    | Syntax::RadioTarget(..)
                    | Syntax::StrikeThrough
                    | Syntax::Subscript(..)
                    | Syntax::Superscript(..)
                    | Syntax::Target(..)
                    | Syntax::Timestamp(..)
                    | Syntax::Underline
                    | Syntax::Verbatim(..) => true,
                    _ => false,
                },

                //(table-row table-cell)
                Syntax::TableRow(..) => match that {
                    Syntax::TableCell => true,
                    _ => false,
                },

                _ => false,
            }
    }
}

pub struct BabelCallData<'a> {
    /// Name of code block being called (string).
    call: &'a str,

    /// Header arguments applied to the named code block (string or nil).
    inside_header: Option<&'a str>,

    /// Arguments passed to the code block (string or nil).
    arguments: Option<&'a str>,

    /// Header arguments applied to the calling instance (string or nil).
    end_header: Option<&'a str>,

    /// Raw call, as Org syntax (string).
    value: &'a str,
}

pub struct ClockData<'a> {
    /// Clock duration for a closed clock, or nil (string or nil).
    duration: &'a str,

    /// Status of current clock (symbol closed or running).
    status: ClockStatus,

    /// Timestamp associated to clock keyword (timestamp object).
    value: TimestampData<'a>,
}

pub enum ClockStatus {
    Running,
    Closed,
}

pub struct CommentData<'a> {
    /// Comments, with pound signs (string).
    value: &'a str,
}

pub struct CommentBlockData<'a> {
    /// Comments, without block's boundaries (string).
    value: &'a str,
}

pub struct DiarySexpData<'a> {
    /// Full Sexp (string).
    value: &'a str,
}

pub struct DrawerData<'a> {
    /// Drawer's name (string).
    drawer_name: &'a str,
}

/// Greater element
pub struct DynamicBlockData<'a> {
    /// Block's parameters (string).
    arguments: &'a str,

    /// Block's name (string).
    block_name: &'a str,

    /// Drawer's name (string).
    drawer_name: &'a str,
}

pub enum LineNumberingMode {
    New,
    Continued,
}

pub struct ExampleBlockData<'a> {
    /// Format string used to write labels in current block,
    /// if different from org_coderef_label_format (string or nil).
    label_fmt: Option<&'a str>,

    ///Language of the code in the block, if specified (string or nil).
    language: Option<&'a str>,

    /// Non_nil if code lines should be numbered.
    /// A `new` value starts numbering from 1 wheareas
    /// `continued` resume numbering from previous numbered block
    /// (symbol new, continued or nil).
    number_lines: Option<LineNumberingMode>,

    /// Block's options located on the block's opening line (string)
    options: &'a str,

    /// Optional header arguments (string or nil)
    parameters: Option<&'a str>,

    /// Non_nil when indentation within the block mustn't be modified
    /// upon export (boolean).
    preserve_indent: bool,

    /// Non_nil if labels should be kept visible upon export (boolean).
    retain_labels: bool,

    /// Optional switches for code block export (string or nil).
    switches: Option<&'a str>,

    /// Non_nil if links to labels contained in the block should
    /// display the label instead of the line number (boolean).
    use_labels: bool,

    /// Contents (string).
    value: &'a str,
}

pub struct ExportBlockData<'a> {
    ///Related back_end's name (string).
    type_s: &'a str,

    ///Contents (string)
    value: &'a str,
}

pub struct FixedWidthData<'a> {
    ///Contents, without colons prefix (string).
    value: &'a str,
}

/// Greater element
pub struct FootnoteDefinitionData<'a> {
    /// Label used for references (string).
    label: &'a str,

    /// Number of newline characters between the
    /// beginning of the footnoote and the beginning
    /// of the contents (0, 1 or 2).
    pre_blank: u8,
}

pub struct KeywordData<'a> {
    /// Keyword's name (string).
    key: &'a str,
    /// Keyword's value (string).
    value: &'a str,
}

pub struct LatexEnvironmentData<'a> {
    /// Buffer position at first affiliated keyword or
    /// at the beginning of the first line of environment (integer).
    begin: usize,

    /// Buffer position at the first non_blank line
    /// after last line of the environment, or buffer's end (integer).
    end: usize,

    /// Number of blank lines between last environment's
    /// line and next non_blank line or buffer's end (integer).
    post_blank: usize,

    ///LaTeX code (string).
    value: &'a str,
}

pub struct NodePropertyData<'a> {
    key: &'a str,
    value: &'a str,
}

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

pub struct PropertyDrawerData<'a> {
    // TODO consider creating a type for Poperty
    // TODO consider using Rope's range insteaf of String
    /// Properties defined in the drawer (alist).
    properties: Vec<&'a str>,
}

pub struct SpecialBlockData<'a> {
    /// Block's name (string).
    type_s: &'a str,
    /// Raw contents in block (string).
    raw_value: &'a str,
}

pub struct SrcBlockData<'a> {
    /// Format string used to write labels in current block,
    /// if different from org_coderef_label_format (string or nil).
    label_fmt: Option<&'a str>,

    /// Language of the code in the block, if specified (string or nil).
    language: Option<&'a str>,

    /// Non_nil if code lines should be numbered.
    /// A `new` value starts numbering from 1 wheareas
    /// `continued` resume numbering from previous
    /// numbered block (symbol new, continued or nil).
    number_lines: Option<LineNumberingMode>,

    /// Optional header arguments (string or nil).
    parameters: Option<&'a str>,

    /// Non_nil when indentation within the block
    /// mustn't be modified upon export (boolean).
    preserve_indent: bool,
    ///Non_nil if labels should be kept visible upon export (boolean).
    retain_labels: bool,

    /// Optional switches for code block export (string or nil).
    switches: Option<&'a str>,

    /// Non_nil if links to labels contained in the block
    /// should display the label instead of the line number (boolean).
    use_labels: bool,

    ///Source code (string).
    value: &'a str,
}

pub struct TableData<'a> {
    /// Formulas associated to the table, if any (string or nil).
    tblfm: Option<&'a str>,
    //Table's origin (symbol table.el, org).
    // type_s

    //Raw table.el table or nil (string or nil).
    // value
}

pub struct TableRowData {
    table_row_type: TableRowType,
}

/// Row's type (symbol standard, rule).
pub enum TableRowType {
    Standard,
    Rule,
}

// ===== Objects Data ======

pub struct CodeData<'a> {
    /// Contents (string).
    value: &'a str,
}

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

pub struct ExportSnippetData<'a> {
    /// Relative back_end's name (string).
    back_end: &'a str,

    /// Export code (string).
    value: &'a str,
}

/// Recursive object.
pub struct FootnoteReferenceData<'a> {
    /// Footnote's label, if any (string or nil).
    label: Option<&'a str>,

    /// Determine whether reference has its
    /// definition inline, or not (symbol inline, standard).
    type_s: &'a str,
}

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

pub struct InlineSrcBlockData<'a> {
    ///Language of the code in the block (string).
    language: &'a str,

    ///Optional header arguments (string or nil).
    parameters: Option<&'a str>,

    ///Source code (string).
    value: &'a str,
}

pub struct LatexFragmentData<'a> {
    ///LaTeX code (string).
    value: &'a str,
}

pub enum LinkFormat {
    Plain,
    Angle,
    Bracket,
}

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

pub struct MacroData<'a> {
    /// Arguments passed to the macro (list of strings).
    args: Vec<&'a str>,

    /// Macro's name (string).
    key: &'a str,

    /// Replacement text (string).
    value: &'a str,
}

pub struct RadioTargetData<'a> {
    /// Uninterpreted contents (string).
    raw_value: &'a str,
}

pub struct StatisticsCookieData<'a> {
    /// Full cookie (string).
    value: &'a str,
}

pub struct SubscriptData {
    /// Non_nil if contents are enclosed in curly brackets (t, nil).
    use_brackets_p: bool,
}

/// Recursive object.
pub struct SuperscriptData {
    /// Non_nil if contents are enclosed in curly brackets (t, nil).
    use_brackets_p: bool,
}

pub struct TargetData<'a> {
    ///Target's ID (string).
    value: &'a str,
}

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

pub enum WarningType {
    All,
    First,
}

pub enum TimestampType {
    Active,
    ActiveRange,
    Diary,
    Inactive,
    InactiveRange,
}

pub enum RepeaterType {
    CatchUp,
    Restart,
    Cumulate,
}

pub enum TimeUnit {
    Year,
    Month,
    Week,
    Day,
    Hour,
}

pub struct VerbatimData<'a> {
    ///Contents (string).
    value: &'a str,
}

mod test {
    use crate::data::{Syntax, SyntaxInfo};

    #[test]
    fn can_contain() {
        let bold  = Syntax::Bold;
        let br    = Syntax::LineBreak;
        let verse = Syntax::VerseBlock;

        fn closure_test(that: &Syntax, restriction: impl Fn(&Syntax)-> bool ) -> bool
        {
            restriction(that)
        }

        assert!(!bold.can_contain(&Syntax::VerseBlock));
        assert!(bold.can_contain(&Syntax::LineBreak));
        assert!(closure_test(&br, |that| bold.can_contain(that)));
        assert!(!closure_test(&verse, |that| bold.can_contain(that)));
    }


}
