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

use crate::affiliated::AffiliatedData;
use crate::babel::BabelCallData;
use crate::blocks::CommentBlockData;
use crate::blocks::DynamicBlockData;
use crate::blocks::ExampleBlockData;
use crate::blocks::ExportBlockData;
use crate::blocks::SpecialBlockData;
use crate::blocks::SrcBlockData;
use crate::data::Syntax::BabelCall;
use crate::drawer::DrawerData;
use crate::headline::{HeadlineData, InlineTaskData, NodePropertyData};
use crate::keyword::KeywordData;
use crate::latex::LatexEnvironmentData;
use crate::latex::LatexFragmentData;
use crate::list::*;
use crate::markup::CommentData;
use crate::markup::FixedWidthData;
use crate::markup::FootnoteDefinitionData;
use crate::table::{TableData, TableRowData};
use std::borrow::Cow;
use std::cell::Cell;
use std::cell::RefCell;
use std::fmt;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use std::rc::Weak;

use regex::Regex;

/// Reference to a DOM node.
pub type Handle<'a> = Rc<SyntaxNode<'a>>;

/// Weak reference to a DOM node, used for parent pointers.
pub type WeakHandle<'a> = Weak<SyntaxNode<'a>>;

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
    pub parent: RefCell<Option<WeakHandle<'a>>>,
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
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::OrgData,
            location: Interval { start: 0, end: 0 },
            content_location: None,
            post_blank: 0,
            affiliated: None,
        }
    }


    /// Creates a `SyntaxNode` corosponding to a raw string used as an element
    /// in elisp.
    pub fn create_raw_at(content: &'a str, interval: Interval) -> SyntaxNode<'a> {
        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::PlainText(string),
            location: interval,
            content_location: None,
            post_blank: 0,
            afiliated: None,
        }
    }

    /// Appends a child to the node, setting the child's parent correctly.
    pub fn append_child(self: &Handle<'a>, child: Handle<'a>) {
        *child.parent.borrow_mut() = Some(Rc::downgrade(&self));
        self.children.borrow_mut().push(child);
    }
}

/// Complete list of syntax entities
#[derive(Debug, EnumDiscriminants)]
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
    #[rustfmt::skip]
    pub fn is_greater_element(self) -> bool {
        use SyntaxT::*;
        match self {
            CenterBlock        => true,   // Greater element
            Drawer             => true,   // Greater element
            DynamicBlock       => true,   // Greater element
            FootnoteDefinition => true,   // Greater element
            Headline           => true,   // Greater element
            InlineTask         => true,   // Greater element
            Item               => true,   // Greater element
            PlainList          => true,   // Greater element
            PropertyDrawer     => true,   // Greater element
            QuoteBlock         => true,   // Greater element
            Section            => true,   // Greater element
            SpecialBlock       => true,   // Greater element
            Table              => true,   // Greater element
            _                  => false

        }
    }

    #[rustfmt::skip]
    fn is_element(self) -> bool {
        use SyntaxT::*;
        match self {
            BabelCall          => true,   // Element
            CenterBlock        => true,   // Greater element
            Clock              => true,   // Element
            Comment            => true,   // Element
            CommentBlock       => true,   // Element
            DiarySexp          => true,   // Element
            Drawer             => true,   // Greater element
            DynamicBlock       => true,   // Greater element
            ExampleBlock       => true,   // Element
            ExportBlock        => true,   // Element
            FixedWidth         => true,   // Element
            FootnoteDefinition => true,   // Greater element
            Headline           => true,   // Greater element
            HorizontalRule     => true,   // Element
            InlineTask         => true,   // Greater element
            Item               => true,   // Greater element
            Keyword            => true,   // Element
            LatexEnvironment   => true,   // Element
            NodeProperty       => true,   // Element
            Paragraph          => true,   // Element containing objects.
            PlainList          => true,   // Greater element
            Planning           => true,   // Element
            PropertyDrawer     => true,   // Greater element
            QuoteBlock         => true,   // Greater element
            Section            => true,   // Greater element
            SpecialBlock       => true,   // Greater element
            SrcBlock           => true,   // Element
            Table              => true,   // Greater element
            TableRow           => true,   // Element containing objects.
            VerseBlock         => true,   // Element containing objects.
            _                  => false

        }
    }

    #[rustfmt::skip]
    fn is_object(self) -> bool {
        use SyntaxT::*;
        match self {
            Bold              => true,  // Recursive object
            Code              => true,  // Object.
            Entity            => true,  // Object
            ExportSnippet     => true,  // Object
            FootnoteReference => true,  // Recursive object.
            InlineBabelCall   => true,  // Object
            InlineSrcBlock    => true,  // Object
            Italic            => true,  // Recursive object.
            LineBreak         => true,  // Object
            LatexFragment     => true,  // Object
            Link              => true,  // Recursive object.
            Macro             => true,  // Object
            RadioTarget       => true,  // Recursive object.
            StatisticsCookie  => true,  // Object
            StrikeThrough     => true,  // Recursive object.
            Subscript         => true,  // Recursive object.
            Superscript       => true,  // Recursive object.
            TableCell         => true,  // Recursive object
            Target            => true,  // Object
            Timestamp         => true,  // Object
            Underline         => true,  // Recursive object.
            Verbatim          => true,  // Object
            PlainText         => true,  // Special object
            _                 => false
        }
    }

    #[rustfmt::skip]
    fn is_recursive_object(self) -> bool {
        use SyntaxT::*;
        match self {
            Bold              => true,  // Recursive object
            FootnoteReference => true,  // Recursive object.
            Italic            => true,  // Recursive object.
            Link              => true,  // Recursive object.
            RadioTarget       => true,  // Recursive object.
            StrikeThrough     => true,  // Recursive object.
            Subscript         => true,  // Recursive object.
            Superscript       => true,  // Recursive object.
            TableCell         => true,  // Recursive object
            Underline         => true,  // Recursive object.
            _                 => false
        }
    }

    #[rustfmt::skip]
    fn is_object_container(self) -> bool {
        use SyntaxT::*;
        match self {
            Paragraph         => true,  // Element containing objects.
            TableRow          => true,  // Element containing objects.
            VerseBlock        => true,  // Element containing objects.
            Bold              => true,  // Recursive object
            FootnoteReference => true,  // Recursive object.
            Italic            => true,  // Recursive object.
            Link              => true,  // Recursive object.
            RadioTarget       => true,  // Recursive object.
            StrikeThrough     => true,  // Recursive object.
            Subscript         => true,  // Recursive object.
            Superscript       => true,  // Recursive object.
            TableCell         => true,  // Recursive object
            Underline         => true,  // Recursive object.
            _                 => false
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
        // (standard-set (remq 'table-cell org-element-all-objects))
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

impl<'a> Debug for StringOrObject<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
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
    duration: &'a str,

    /// Status of current clock (symbol closed or running).
    status: ClockStatus,

    /// Timestamp associated to clock keyword (timestamp object).
    value: TimestampData<'a>,
}

#[derive(Debug)]
pub enum ClockStatus {
    Running,
    Closed,
}

#[derive(Debug)]
pub struct DiarySexpData<'a> {
    /// Full Sexp (string).
    value: &'a str,
}

#[derive(Debug)]
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

// ===== Objects Data ======

#[derive(Debug)]
pub struct CodeData<'a> {
    /// Contents (string).
    value: &'a str,
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
    value: &'a str,
}

mod test {

    use crate::data::SyntaxT;

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
}
