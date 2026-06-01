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
    blocks::{DynamicBlockData, ExampleBlockData, ExportBlockData, SpecialBlockData, SrcBlockData},
    headline::{HeadlineData, InlineTaskData, NodePropertyData},
    keyword::KeywordData,
    latex::LatexEnvironmentData,
    list::{ItemData, PlainListData},
    markup::FootnoteDefinitionData,
    table::{SpreadsheetCellData, SpreadsheetData, SpreadsheetRowData, TableRowType},
};
use memchr::memchr;
use std::num::NonZeroUsize;

pub use bumpalo::collections::Vec as BumpVec;
use strum_macros::EnumDiscriminants;

pub type NodeId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    pub start: usize,
    pub end: usize,
}

/// ParseTree node.
/// https://orgmode.org/worg/dev/org-element-api.html#attributes
/// Should be bound to the underlying rope's lifetime
#[derive(Debug)]
pub struct SyntaxNode<'a, 'b> {
    /// Parent node index (base-1) in the arena, or `None` for the root.
    /// Stores `p + 1` so that `NonZeroUsize`'s niche optimisation fires,
    /// making `Option<NonZeroUsize>` a single word.
    pub parent: Option<NonZeroUsize>,
    /// Child node indices in the arena.
    pub children: BumpVec<'b, NodeId>,

    pub data: Syntax<'a, 'b>,

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
    pub affiliated: Option<&'b AffiliatedData<'a, 'b>>,
}

/// Builder for [`SyntaxNode`], obtained via [`SyntaxNode::new`].
///
/// `data` and `location` are required; all other fields default to their zero
/// values (`None`, `0`, empty `Vec`).
///
/// ```rust,ignore
/// SyntaxNode::new(Syntax::CenterBlock, location)
///     .content(content_location)
///     .post_blank(n)
///     .affiliated(aff)
///     .build()
/// ```
pub struct SyntaxNodeBuilder<'a, 'b> {
    data: Syntax<'a, 'b>,
    location: Interval,
    content_location: Option<Interval>,
    post_blank: usize,
    affiliated: Option<&'b AffiliatedData<'a, 'b>>,
    bump: &'b bumpalo::Bump,
}

impl<'a, 'b> SyntaxNodeBuilder<'a, 'b> {
    #[inline]
    pub fn content(mut self, loc: impl Into<Interval>) -> Self {
        self.content_location = Some(loc.into());
        self
    }

    #[inline]
    pub fn post_blank(mut self, n: usize) -> Self {
        self.post_blank = n;
        self
    }

    #[inline]
    pub fn affiliated(mut self, aff: Option<AffiliatedData<'a, 'b>>) -> Self {
        self.affiliated = aff.map(|a| &*self.bump.alloc(a));
        self
    }

    #[inline]
    pub fn build(self) -> SyntaxNode<'a, 'b> {
        SyntaxNode {
            parent: None,
            children: BumpVec::new_in(self.bump),
            data: self.data,
            location: self.location,
            content_location: self.content_location,
            post_blank: self.post_blank,
            affiliated: self.affiliated,
        }
    }
}

impl<'a, 'b> SyntaxNode<'a, 'b> {
    /// Begin constructing a [`SyntaxNode`].  Returns a [`SyntaxNodeBuilder`]
    /// pre-loaded with the two required fields; call `.build()` to finish.
    #[allow(clippy::new_ret_no_self)]
    #[inline]
    pub fn new(
        data: Syntax<'a, 'b>,
        location: impl Into<Interval>,
        bump: &'b bumpalo::Bump,
    ) -> SyntaxNodeBuilder<'a, 'b> {
        SyntaxNodeBuilder {
            data,
            location: location.into(),
            content_location: None,
            post_blank: 0,
            affiliated: None,
            bump,
        }
    }

    /// Creates a `SyntaxNode` corosponding to a raw string used as an
    /// element in elisp.
    #[inline]
    pub fn create_raw_at(
        content: &'a str,
        interval: Interval,
        bump: &'b bumpalo::Bump,
    ) -> SyntaxNode<'a, 'b> {
        SyntaxNode {
            parent: None,
            children: BumpVec::new_in(bump),
            data: Syntax::PlainText(content),
            location: interval,
            content_location: None,
            post_blank: 0,
            affiliated: None,
        }
    }

    /// Create a fallback paragraph node spanning to the end of the current line.
    ///
    /// Used for element parsers that are not yet implemented, so that parsing
    /// can continue past the unrecognised content without panicking.
    #[inline]
    pub fn fallback(
        input: &str,
        start: usize,
        limit: usize,
        bump: &'b bumpalo::Bump,
    ) -> SyntaxNode<'a, 'b> {
        let end = memchr(b'\n', &input.as_bytes()[start..limit])
            .map_or(limit, |i| (start + i + 1).min(limit));
        SyntaxNode {
            parent: None,
            children: BumpVec::new_in(bump),
            data: Syntax::Paragraph,
            location: Interval { start, end },
            content_location: None,
            post_blank: 0,
            affiliated: None,
        }
    }

    #[inline]
    pub fn create_root(bump: &'b bumpalo::Bump) -> SyntaxNode<'a, 'b> {
        SyntaxNode {
            parent: None,
            children: BumpVec::new_in(bump),
            data: Syntax::OrgData,
            location: Interval { start: 0, end: 0 },
            content_location: None,
            post_blank: 0,
            affiliated: None,
        }
    }
}

/// An arena that owns all [`SyntaxNode`] values.  Nodes are addressed
/// by [`NodeId`] (a `usize` index) instead of `Rc` pointers.
#[derive(Debug)]
pub struct NodeArena<'a, 'b> {
    pub(crate) nodes: Vec<SyntaxNode<'a, 'b>>,
}

impl<'a, 'b> NodeArena<'a, 'b> {
    #[inline]
    pub fn new() -> Self {
        NodeArena { nodes: Vec::new() }
    }

    /// Create an arena pre-sized to hold `capacity` nodes.
    ///
    /// # When to use
    ///
    /// Pre-sizing the arena only pays off when two conditions hold simultaneously:
    ///
    /// 1. **The allocation fits within the CPU's L2 cache.**  `SyntaxNode` is
    ///    large (~88 B+), so `capacity` above roughly `L2_bytes / 88` causes
    ///    page-fault and TLB pressure that outweighs the savings from avoided
    ///    Vec doublings.  On a typical 256 KB L2 that ceiling is ~2 900 nodes.
    ///
    /// 2. **The parser is long-lived and reused** across many calls, so the
    ///    upfront cost amortises over many parses.  In the single-shot benchmark
    ///    pattern (`Parser::new` → `parse_buffer` → drop) even a well-sized
    ///    pre-allocation regressed performance by ~10–45% depending on corpus
    ///    size, because the allocation overhead dominated for small inputs and
    ///    the arena exceeded L2 for large ones.
    ///
    /// The `Parser` itself does **not** call this; it lets the arena grow
    /// naturally via `Vec` doubling, which keeps the live working set small and
    /// cache-hot throughout the parse.  Call this only from benchmarked, long-
    /// lived contexts where you have measured a concrete win.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        NodeArena {
            nodes: Vec::with_capacity(capacity),
        }
    }

    /// Allocate a leaf node (no children, no parent).
    #[inline]
    pub fn alloc(&mut self, mut node: SyntaxNode<'a, 'b>) -> NodeId {
        node.parent = None;
        let id = self.nodes.len();
        self.nodes.push(node);
        id
    }

    /// Allocate a node with the given children, setting each child's parent.
    #[inline]
    pub fn alloc_with_children(
        &mut self,
        mut node: SyntaxNode<'a, 'b>,
        children: BumpVec<'b, NodeId>,
    ) -> NodeId {
        let id = self.nodes.len();
        for &child in &children {
            self.nodes[child].parent = NonZeroUsize::new(id + 1);
        }
        node.parent = None;
        node.children = children;
        self.nodes.push(node);
        id
    }

    /// Replace the children of `parent`, updating parent back-pointers.
    #[inline]
    pub fn set_children(&mut self, parent: NodeId, children: BumpVec<'b, NodeId>) {
        for &child in &children {
            self.nodes[child].parent = NonZeroUsize::new(parent + 1);
        }
        self.nodes[parent].children = children;
    }

    /// Store parsed headline-title objects in `HeadlineData::title_objects`.
    ///
    /// Title objects are inline nodes from the headline's secondary string.
    /// They belong to `:title` in Emacs org-element and must not appear in
    /// `SyntaxNode::children`, which contains only body content (sections and
    /// sub-headlines).  This method sets the parent pointer of each object to
    /// `parent` so arena traversals can still walk up to the headline.
    #[inline]
    pub fn set_title_objects(&mut self, parent: NodeId, objects: BumpVec<'b, NodeId>) {
        for &obj in &objects {
            self.nodes[obj].parent = NonZeroUsize::new(parent + 1);
        }
        if let Syntax::Headline(ref mut data) = self.nodes[parent].data {
            data.title_objects = objects;
        }
    }

    #[inline]
    pub fn get(&self, id: NodeId) -> &SyntaxNode<'a, 'b> {
        &self.nodes[id]
    }

    /// Create a pre-order iterator over the subtree rooted at `root`.
    #[inline]
    pub fn nodes(&self, root: NodeId) -> Nodes<'_, 'a, 'b> {
        Nodes {
            arena: self,
            stack: InlineStack::with_first_entry(0),
            current: root,
        }
    }

    /// Iterate over `(id, headline_byte_offset)` pairs in the subtree
    /// rooted at `root`.
    ///
    /// Yields one entry per headline that carries a `:ID:` node-property
    /// in its property drawer.  The id string is borrowed from the
    /// original input text.
    #[inline]
    pub fn id_headlines<'s>(
        &'s self,
        root: NodeId,
    ) -> impl Iterator<Item = (&'a str, usize)> + use<'a, 's, 'b> {
        let mut hl_start = 0usize;
        self.nodes(root).filter_map(move |node| match &node.data {
            Syntax::Headline(_) => {
                hl_start = node.location.start;
                None
            }
            Syntax::NodeProperty(np) if np.key == "ID" => {
                Some((np.value, hl_start))
            }
            _ => None,
        })
    }
}

impl<'a, 'b> Default for NodeArena<'a, 'b> {
    #[inline]
    fn default() -> Self {
        NodeArena::new()
    }
}

impl<'a, 'b> std::ops::Index<NodeId> for NodeArena<'a, 'b> {
    type Output = SyntaxNode<'a, 'b>;
    #[inline]
    fn index(&self, id: NodeId) -> &Self::Output {
        &self.nodes[id]
    }
}

/// DFS child-index stack with a fixed inline buffer; only spills to heap
/// when nesting depth exceeds `N`.
struct InlineStack<const N: usize> {
    inline: [usize; N],
    len: usize,
    overflow: Vec<usize>,
}

impl<const N: usize> InlineStack<N> {
    fn with_first_entry(v: usize) -> Self {
        let mut s = Self {
            inline: [0; N],
            len: 0,
            overflow: Vec::new(),
        };
        s.push(v);
        s
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    fn push(&mut self, v: usize) {
        if self.len < N {
            self.inline[self.len] = v;
        } else {
            self.overflow.push(v);
        }
        self.len += 1;
    }

    #[inline]
    fn pop(&mut self) {
        if self.len == 0 {
            return;
        }
        self.len -= 1;
        if self.len >= N {
            self.overflow.pop();
        }
    }

    #[inline]
    fn last(&self) -> Option<usize> {
        if self.len == 0 {
            return None;
        }
        if self.len <= N {
            Some(self.inline[self.len - 1])
        } else {
            self.overflow.last().copied()
        }
    }

    #[inline]
    fn last_mut(&mut self) -> Option<&mut usize> {
        if self.len == 0 {
            return None;
        }
        if self.len <= N {
            Some(&mut self.inline[self.len - 1])
        } else {
            self.overflow.last_mut()
        }
    }
}

/// A pre-order traversal of [`SyntaxNode`] values inside a [`NodeArena`].
pub struct Nodes<'s, 'a, 'b> {
    arena: &'s NodeArena<'a, 'b>,
    stack: InlineStack<16>,
    current: NodeId,
}

impl<'s, 'a, 'b> Iterator for Nodes<'s, 'a, 'b> {
    type Item = &'s SyntaxNode<'a, 'b>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.stack.is_empty() {
            return None;
        }

        let saved = self.current;

        loop {
            let n = self.arena.nodes[self.current].children.len();
            let exhausted = self.stack.last().is_none_or(|idx| idx >= n);
            if !exhausted {
                break;
            }
            self.stack.pop();
            if self.stack.is_empty() {
                return Some(&self.arena.nodes[saved]);
            }
            self.current = self.arena.nodes[self.current].parent?.get() - 1;
        }

        let child_idx = self.stack.last().unwrap();
        let child = self.arena.nodes[self.current].children[child_idx];
        *self.stack.last_mut().unwrap() += 1;
        self.stack.push(0);
        self.current = child;

        Some(&self.arena.nodes[saved])
    }
}

/// An enumerated list of all Org syntactic units.
/// This structure is not meant to be extended.
#[derive(Debug, EnumDiscriminants)]
#[strum_discriminants(name(SyntaxT))]
pub enum Syntax<'a, 'b> {
    /// The root of the parse tree
    OrgData,

    /// Element
    BabelCall(&'b mut BabelCallData<'a>),

    /// Greater element
    CenterBlock,

    /// Element
    Clock(&'b mut ClockData<'a>),

    /// Element
    Comment(&'a str),

    /// Element
    CommentBlock(&'a str),

    /// Element
    DiarySexp(&'a str),

    /// Greater element
    Drawer(&'a str),

    /// Greater element
    DynamicBlock(&'b mut DynamicBlockData<'a>),

    /// Element
    ExampleBlock(&'b mut ExampleBlockData<'a>),

    /// Element
    ExportBlock(&'b mut ExportBlockData<'a>),

    /// Element
    FixedWidth(&'a str),

    /// Greater element
    FootnoteDefinition(&'b mut FootnoteDefinitionData<'a>),

    /// Greater element
    /// In addition to the following list, any property specified
    /// in a property drawer attached to the headline will be
    /// accessible as an attribute (e.g. CUSTOM_ID).
    Headline(&'b mut HeadlineData<'a, 'b>),

    /// Element
    HorizontalRule,

    /// Greater element
    /// In addition to the following list, any property specified
    /// in a property drawer attached to the headline
    /// will be accessible as an attribute
    /// (with an uppercase name, e.g. CUSTOM_ID).
    InlineTask(&'b mut InlineTaskData<'a, 'b>),

    /// Greater element
    Item(&'b mut ItemData<'a>),

    /// Element
    /// Keywords follow the syntax:
    /// ```org
    ///   #+KEY: VALUE
    /// ```
    /// KEY can contain any non-whitespace character, but it cannot be equal to "CALL" or any affiliated keyword.<br>
    /// VALUE can contain any character excepted a new line.<br>
    /// If KEY belongs to org-element-document-properties, VALUE can contain objects.
    Keyword(&'b mut KeywordData<'a>),

    /// Element
    /// An inline Latex environment element
    LatexEnvironment(&'b mut LatexEnvironmentData<'a>),

    /// Element
    /// Node properties can only exist in property drawers
    NodeProperty(&'b mut NodePropertyData<'a>),

    /// Element containing objects.
    Paragraph,

    /// Greater element
    PlainList(PlainListData<'a, 'b>),

    /// Element
    Planning(&'b mut PlanningData<'a>),

    /// Greater Element
    PropertyDrawer,

    /// Greater element
    QuoteBlock,

    /// Greater element
    Section,

    /// Greater element
    SpecialBlock(&'b mut SpecialBlockData<'a>),

    /// Element
    SrcBlock(&'b mut SrcBlockData<'a>),

    /// Greater element — a plain display table with no formula.
    Table,

    /// Greater element — a table with a `#+TBLFM:` formula requiring recalculation.
    Spreadsheet(&'b mut SpreadsheetData<'a>),

    /// Element containing objects — a row inside a [`Syntax::Table`].
    TableRow(TableRowType),

    /// Element — a row inside a [`Syntax::Spreadsheet`].
    SpreadsheetRow(SpreadsheetRowData),

    /// Object — an individually addressable cell inside a [`Syntax::SpreadsheetRow`].
    SpreadsheetCell(&'b mut SpreadsheetCellData<'a>),

    /// Element containing objects.
    VerseBlock,

    /// Recursive object
    Bold,

    /// Object.
    Code(&'a str),

    /// Object
    Entity(&'b mut EntityData<'a>),

    /// Object
    ExportSnippet(&'b mut ExportSnippetData<'a>),

    /// Recursive object.
    FootnoteReference(&'b mut FootnoteReferenceData<'a>),

    /// Object
    InlineBabelCall(&'b mut InlineBabelCallData<'a>),

    /// Object
    InlineSrcBlock(&'b mut InlineSrcBlockData<'a>),

    /// Recursive object.
    Italic,

    LineBreak,

    /// Object
    LatexFragment(&'a str),

    /// Recursive object.
    Link(&'b mut LinkData<'a>),

    /// Object
    Macro(&'b mut MacroData<'a>),

    /// Recursive object.
    RadioTarget(RadioTargetData<'a>),

    /// Object
    StatisticsCookie(StatisticsCookieData<'a>),

    /// Recursive object.
    StrikeThrough,

    /// Recursive object.
    Script(ScriptFlags),

    /// Recursive object
    TableCell,

    /// Object
    Target(&'a str),

    /// Object
    Timestamp(&'b mut TimestampData<'a>),

    /// Recursive object.
    Underline,

    /// Object
    Verbatim(&'a str),

    /// Special object
    PlainText(&'a str),
}

impl Interval {
    #[inline]
    pub fn as_range(self) -> std::ops::Range<usize> {
        self.start..self.end
    }
}

impl From<(usize, usize)> for Interval {
    #[inline]
    fn from((start, end): (usize, usize)) -> Self {
        Interval { start, end }
    }
}

impl SyntaxT {
    #[inline]
    pub fn is_greater_element(self) -> bool {
        use SyntaxT::*;
        matches!(
            self,
            CenterBlock
                | Drawer
                | DynamicBlock
                | FootnoteDefinition
                | Headline
                | InlineTask
                | Item
                | PlainList
                | PropertyDrawer
                | QuoteBlock
                | Section
                | SpecialBlock
                | Table
                | Spreadsheet
        )
    }

    fn is_element(self) -> bool {
        use SyntaxT::*;
        matches!(
            self,
            BabelCall
                | CenterBlock
                | Clock
                | Comment
                | CommentBlock
                | DiarySexp
                | Drawer
                | DynamicBlock
                | ExampleBlock
                | ExportBlock
                | FixedWidth
                | FootnoteDefinition
                | Headline
                | HorizontalRule
                | InlineTask
                | Item
                | Keyword
                | LatexEnvironment
                | NodeProperty
                | Paragraph
                | PlainList
                | Planning
                | PropertyDrawer
                | QuoteBlock
                | Section
                | SpecialBlock
                | SrcBlock
                | Table
                | Spreadsheet
                | TableRow
                | SpreadsheetRow
                | VerseBlock
        )
    }

    fn is_object(self) -> bool {
        use SyntaxT::*;
        matches!(
            self,
            Bold | Code
                | Entity
                | ExportSnippet
                | FootnoteReference
                | InlineBabelCall
                | InlineSrcBlock
                | Italic
                | LineBreak
                | LatexFragment
                | Link
                | Macro
                | RadioTarget
                | StatisticsCookie
                | StrikeThrough
                | Script
                | TableCell
                | Target
                | Timestamp
                | Underline
                | Verbatim
                | PlainText
                | SpreadsheetCell
        )
    }

    fn is_recursive_object(self) -> bool {
        use SyntaxT::*;
        matches!(
            self,
            Bold | FootnoteReference
                | Italic
                | Link
                | RadioTarget
                | StrikeThrough
                | Script
                | TableCell
                | Underline
        )
    }

    fn is_object_container(self) -> bool {
        use SyntaxT::*;
        matches!(
            self,
            Paragraph
                | TableRow
                | VerseBlock
                | Bold
                | FootnoteReference
                | Italic
                | Link
                | RadioTarget
                | StrikeThrough
                | Script
                | TableCell
                | Underline
        )
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
    #[inline]
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
            Bold | Italic | FootnoteReference | Paragraph | StrikeThrough | Script | Underline
            | VerseBlock => is_from_standard_set(that),

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
            Link => matches!(
                that,
                Bold | Code
                    | Entity
                    | ExportSnippet
                    | InlineBabelCall
                    | InlineSrcBlock
                    | Italic
                    | LatexFragment
                    | Macro
                    | StatisticsCookie
                    | StrikeThrough
                    | Script
                    | Underline
                    | Verbatim
            ),

            // Remove any variable object from radio target as it would
            // prevent it from being properly recognized.
            // (radio-target bold code entity italic
            //               latex-fragment strike-through
            //               subscript superscript underline)
            RadioTarget => matches!(
                that,
                Bold | Code | Entity | Italic | LatexFragment | StrikeThrough | Script | Underline
            ),

            // Ignore inline babel call and inline source block as formulas
            // are possible.  Also ignore line breaks and statistics
            // cookies.
            // (table-cell bold code entity export-snippet footnote-reference italic
            //             latex-fragment link macro radio-target strike-through
            //             subscript superscript target timestamp underline verbatim)
            TableCell => matches!(
                that,
                Bold | Code
                    | Entity
                    | ExportSnippet
                    | FootnoteReference
                    | Italic
                    | LatexFragment
                    | Link
                    | Macro
                    | RadioTarget
                    | StrikeThrough
                    | Script
                    | Target
                    | Timestamp
                    | Underline
                    | Verbatim
            ),

            //(table-row table-cell)
            TableRow => matches!(that, TableCell),

            _ => false,
        }
    }
}

/// Some elements can contain objects directly in their value fields
pub enum StringOrObject<'a, 'b> {
    Raw(&'a str),
    Parsed(&'b mut SyntaxNode<'a, 'b>),
}

impl<'a, 'b> core::fmt::Debug for StringOrObject<'a, 'b> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            StringOrObject::Raw(raw) => write!(f, "Raw: {:?}", raw),
            StringOrObject::Parsed(_p) => unimplemented!(),
        }
    }
}

impl<'a, 'b> PartialEq for StringOrObject<'a, 'b> {
    #[inline]
    fn eq(&self, other: &StringOrObject<'a, 'b>) -> bool {
        match self {
            StringOrObject::Raw(raw) => match other {
                StringOrObject::Parsed(..) => false,
                StringOrObject::Raw(rhs) => raw.eq(rhs),
            },
            StringOrObject::Parsed(_p) => unimplemented!(),
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

impl<'a> ClockData<'a> {
    #[inline]
    pub fn new(raw: &'a str) -> Self {
        Self {
            duration: "",
            status: ClockStatus::Running,
            raw,
        }
    }
}

#[derive(Debug)]
#[repr(u8)]
pub enum ClockStatus {
    Running,
    Closed,
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum LineNumberingMode {
    New,
    Continued,
}

#[derive(Debug)]
pub struct PlanningData<'a> {
    /// Timestamp associated to closed keyword, if any
    /// (timestamp object or nil).
    pub closed: Option<TimestampData<'a>>,

    /// Timestamp associated to deadline keyword, if any
    /// (timestamp object or nil).
    pub deadline: Option<TimestampData<'a>>,

    /// Timestamp associated to scheduled keyword, if any
    /// (timestamp object or nil).
    pub scheduled: Option<TimestampData<'a>>,
}

/// Packed bitflags for [`EntityData`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityFlags(u8);

impl EntityFlags {
    const LATEX_MATH_P: u8 = 0b01;
    const USE_BRACKETS_P: u8 = 0b10;

    #[inline]
    pub fn new(latex_math_p: bool, use_brackets_p: bool) -> Self {
        let mut f = 0;
        if latex_math_p {
            f |= Self::LATEX_MATH_P;
        }
        if use_brackets_p {
            f |= Self::USE_BRACKETS_P;
        }
        EntityFlags(f)
    }

    #[inline]
    pub fn latex_math_p(self) -> bool {
        self.0 & Self::LATEX_MATH_P != 0
    }

    #[inline]
    pub fn use_brackets_p(self) -> bool {
        self.0 & Self::USE_BRACKETS_P != 0
    }
}

#[derive(Debug)]
pub struct EntityData<'a> {
    /// Entity's ASCII representation (string).
    ascii: &'a str,

    /// Entity's HTML representation (string).
    html: &'a str,

    /// Entity's LaTeX representation (string).
    latex: &'a str,

    /// Packed flags.
    flags: EntityFlags,

    /// Entity's Latin-1 encoding representation (string).
    latin1: &'a str,

    /// Entity's name, without backslash nor brackets (string).
    name: &'a str,

    /// Entity's UTF-8 encoding representation (string).
    utf_8: &'a str,
}

impl<'a> EntityData<'a> {
    #[inline]
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
            "leftrightarrow" => (
                "\\leftrightarrow",
                "&harr;",
                "\\leftrightarrow",
                false,
                "↔",
                "↔",
            ),
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
            flags: EntityFlags::new(latex_math_p, false),
            latin1,
            name,
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
    pub label: Option<&'a str>,

    /// Determine whether reference has its
    /// definition inline, or not (symbol inline, standard).
    pub type_s: &'a str,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LinkFormat {
    Plain,
    Angle,
    Bracket,
}

/// Packed bitflags for [`LinkData`], combining [`LinkFormat`] (3 variants, 2 bits)
/// and [`LinkType`] (6 variants, 3 bits) into a single byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkFlags(u8);

impl LinkFlags {
    const FORMAT_MASK: u8 = 0b00011;
    const TYPE_SHIFT: u8 = 2;

    #[inline]
    pub fn new(format: LinkFormat, link_type: LinkType) -> Self {
        let f = match format {
            LinkFormat::Plain => 0,
            LinkFormat::Angle => 1,
            LinkFormat::Bracket => 2,
        };
        let t = match link_type {
            LinkType::Coderef => 0,
            LinkType::CustomId => 1,
            LinkType::File => 2,
            LinkType::Fuzzy => 3,
            LinkType::Id => 4,
            LinkType::Radio => 5,
        };
        LinkFlags(f | (t << Self::TYPE_SHIFT))
    }

    #[inline]
    pub fn format(self) -> LinkFormat {
        match self.0 & Self::FORMAT_MASK {
            0 => LinkFormat::Plain,
            1 => LinkFormat::Angle,
            _ => LinkFormat::Bracket,
        }
    }

    #[inline]
    pub fn link_type(self) -> LinkType {
        match self.0 >> Self::TYPE_SHIFT {
            0 => LinkType::Coderef,
            1 => LinkType::CustomId,
            2 => LinkType::File,
            3 => LinkType::Fuzzy,
            4 => LinkType::Id,
            _ => LinkType::Radio,
        }
    }
}

#[derive(Debug)]
pub struct LinkData<'a> {
    /// Name of application requested to open the link
    /// in Emacs (string or nil).
    /// It only applies to "file" type links.
    application: Option<&'a str>,

    /// Packed format + link-type flags.
    flags: LinkFlags,

    /// Identifier for link's destination.
    /// It is usually the link part with type,
    /// if specified, removed (string).
    path: &'a str,

    ///Uninterpreted link part (string).
    raw_link: &'a str,

    /// Additional information for file location (string or nil).
    /// It only applies to "file" type links.
    search_option: Option<&'a str>,
}

impl<'a> LinkData<'a> {
    #[inline]
    pub fn new_plain(raw: &'a str) -> Self {
        let link_type = if raw.starts_with("https://")
            || raw.starts_with("http://")
            || raw.starts_with("ftp://")
        {
            LinkType::File
        } else {
            LinkType::Fuzzy
        };
        LinkData {
            application: None,
            flags: LinkFlags::new(LinkFormat::Plain, link_type),
            path: raw,
            raw_link: raw,
            search_option: None,
        }
    }

    #[inline]
    pub fn new(raw: &'a str) -> Self {
        // Strip outer [[brackets]] for protocol detection
        let inner = raw
            .strip_prefix("[[")
            .and_then(|s| s.strip_suffix("]]"))
            .unwrap_or(raw);
        let (link_type, path) = if inner.starts_with("http://") || inner.starts_with("https://") {
            (LinkType::File, inner)
        } else if inner.starts_with("file:") {
            (LinkType::File, inner.strip_prefix("file:").unwrap_or(inner))
        } else {
            (LinkType::Fuzzy, raw)
        };

        LinkData {
            application: None,
            flags: LinkFlags::new(LinkFormat::Bracket, link_type),
            path,
            raw_link: raw,
            search_option: None,
        }
    }

    #[inline]
    pub fn link_type(&self) -> LinkType {
        self.flags.link_type()
    }

    #[inline]
    pub fn raw_link(&self) -> &'a str {
        self.raw_link
    }
}

#[derive(Debug)]
#[repr(u8)]
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

/// Whether a subscript or superscript is enclosed in curly brackets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Brackets {
    Bare,
    Bracketed,
}

/// Subscript vs superscript.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptKind {
    Sub,
    Sup,
}

/// Packed bitflag representation of [`ScriptKind`] and [`Brackets`].
///
/// Bit 0 = Sup (0 = Sub, 1 = Sup)
/// Bit 1 = Bracketed (0 = Bare, 1 = Bracketed)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptFlags(u8);

impl ScriptFlags {
    const SUP: u8 = 0b01;
    const BRACKETED: u8 = 0b10;

    #[inline]
    pub fn new(kind: ScriptKind, brackets: Brackets) -> Self {
        let mut f = 0;
        if matches!(kind, ScriptKind::Sup) {
            f |= Self::SUP;
        }
        if matches!(brackets, Brackets::Bracketed) {
            f |= Self::BRACKETED;
        }
        ScriptFlags(f)
    }

    #[inline]
    pub fn kind(self) -> ScriptKind {
        if self.0 & Self::SUP != 0 {
            ScriptKind::Sup
        } else {
            ScriptKind::Sub
        }
    }

    #[inline]
    pub fn brackets(self) -> Brackets {
        if self.0 & Self::BRACKETED != 0 {
            Brackets::Bracketed
        } else {
            Brackets::Bare
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimestampData<'a> {
    /// Day part from timestamp end.
    /// If no ending date is defined, it defaults to start day part (integer).
    pub day_end: usize,

    /// Day part from timestamp start (integer).
    pub day_start: usize,

    /// Hour part from timestamp end.
    /// If no ending date is defined, it defaults to start hour part,
    /// if any (integer or nil).
    pub hour_end: Option<usize>,

    /// Hour part from timestamp start, if specified (integer or nil).
    pub hour_start: Option<usize>,

    /// Minute part from timestamp end.
    /// If no ending date is defined, it defaults to start minute part,
    /// if any (integer or nil).
    pub minute_end: Option<usize>,

    /// Minute part from timestamp start, if specified (integer or nil).
    pub minute_start: Option<usize>,

    /// Month part from timestamp end.
    /// If no ending date is defined, it defaults to start month part
    /// (integer).
    pub month_end: usize,

    /// Month part from timestamp start (integer).
    pub month_start: usize,

    /// Raw timestamp (string).
    pub raw_value: &'a str,

    // TODO maybe the following three fields can be combined into one
    /// Type of repeater, if any (symbol catch_up, restart, cumulate or nil)
    pub repeater_type: Option<RepeaterType>,

    /// Unit of shift, if a repeater is defined
    /// (symbol year, month, week, day, hour or nil).
    pub repeater_unit: Option<TimeUnit>,

    /// Value of shift, if a repeater is defined (integer or nil).
    pub repeater_value: Option<usize>,

    /// Type of timestamp:
    /// (symbol active, active_range, diary, inactive, inactive_range).
    pub type_s: TimestampType,

    /// Type of warning, if any (symbol all, first or nil)
    pub warning_type: Option<WarningType>,

    /// Unit of delay, if one is defined
    /// (symbol year, month, week, day, hour or nil).
    pub warning_unit: Option<TimeUnit>,

    /// Value of delay, if one is defined (integer or nil).
    pub warning_value: Option<usize>,

    /// Year part from timestamp end.
    /// If no ending date is defined, it defaults to start year part (integer)
    pub year_end: usize,

    /// Year part from timestamp start (integer).
    pub year_start: usize,
}

impl<'a> TimestampData<'a> {
    #[inline]
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
        let mut dash_parts = raw.split('-');
        let year_str = dash_parts.next()?;
        let month_str = dash_parts.next()?;
        let day_rest = dash_parts.next()?;
        let end_part = dash_parts.next();

        let year_start: usize = year_str
            .trim_start_matches('<')
            .trim_start_matches('[')
            .parse()
            .ok()?;
        let month_start: usize = month_str.parse().ok()?;
        let day_str = day_rest.split_whitespace().next().unwrap_or("1");
        let day_start: usize = day_str
            .trim_end_matches('>')
            .trim_end_matches(']')
            .parse()
            .ok()?;

        // Try to parse time if present
        let mut hour_start = None;
        let mut minute_start = None;
        // Skip day-name tokens (e.g. "Sun"); find the first HH:MM token.
        for token in day_rest.split_whitespace().skip(1) {
            if token.contains(':') {
                let start_time = token.split('-').next().unwrap_or(token);
                let mut time_iter = start_time.split(':');
                if let (Some(h), Some(m)) = (time_iter.next(), time_iter.next()) {
                    hour_start = h.parse().ok();
                    minute_start = m.trim_end_matches(['>', ']']).parse().ok();
                }
                break;
            }
        }

        // Check for end time in a time range: end_part = "11:30>" when input is "10:15-11:30"
        let (hour_end, minute_end) = if let Some(ep) = end_part {
            let end_trimmed = ep.trim_end_matches(['>', ']']);
            let mut ep_iter = end_trimmed.split(':');
            if let (Some(h), Some(m)) = (ep_iter.next(), ep_iter.next()) {
                (h.parse().ok(), m.parse().ok())
            } else {
                (hour_start, minute_start)
            }
        } else {
            (hour_start, minute_start)
        };

        Some(TimestampData {
            day_end: day_start,
            day_start,
            hour_end,
            hour_start,
            minute_end,
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

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum WarningType {
    All,
    First,
}

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum TimestampType {
    Active,
    ActiveRange,
    Diary,
    Inactive,
    InactiveRange,
}

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum RepeaterType {
    CatchUp,
    Restart,
    Cumulate,
}

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum TimeUnit {
    Year,
    Month,
    Week,
    Day,
    Hour,
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn timestamp_with_time_component() {
        let raw = "<2023-12-31 10:30>";
        let ts = TimestampData::new(raw).expect("Timestamp with time should parse");
        assert_eq!(ts.hour_start, Some(10));
        assert_eq!(ts.minute_start, Some(30));
    }

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
        let bump = bumpalo::Bump::new();
        let mut arena = NodeArena::new();
        let root = arena.alloc(SyntaxNode::create_root(&bump));
        assert_eq!(arena.nodes(root).count(), 1);
    }

    #[test]
    fn nodes_iter_with_several_children_return_all() {
        let bump = bumpalo::Bump::new();
        let mut arena = NodeArena::new();
        let parent = arena.alloc(SyntaxNode::create_root(&bump));
        const NUM_CHILDREN: usize = 4;
        let children: Vec<NodeId> = (0..NUM_CHILDREN)
            .map(|_| arena.alloc(SyntaxNode::create_root(&bump)))
            .collect();
        arena.set_children(parent, BumpVec::from_iter_in(children.clone(), &bump));

        let results: Vec<&SyntaxNode> = arena.nodes(parent).collect();

        assert_eq!(results.len(), NUM_CHILDREN + 1);
        assert!(matches!(results[0].data, Syntax::OrgData));
        for (idx, _child) in children.iter().enumerate() {
            let result_node = results[idx + 1];
            assert_eq!(
                result_node.parent,
                NonZeroUsize::new(parent + 1),
                "Parent mismatch at idx {}",
                idx
            );
            assert!(matches!(result_node.data, Syntax::OrgData));
        }
    }

    #[test]
    fn nodes_iter_with_several_layers_return_all() {
        let bump = bumpalo::Bump::new();
        let mut arena = NodeArena::new();
        const LEVELS: usize = 4;
        let mut ids: Vec<NodeId> = Vec::new();
        for _ in 0..LEVELS {
            ids.push(arena.alloc(SyntaxNode::create_root(&bump)));
        }
        for i in 1..LEVELS {
            arena.set_children(ids[i - 1], BumpVec::from_iter_in([ids[i]], &bump));
        }

        let results: Vec<&SyntaxNode> = arena.nodes(ids[0]).collect();

        assert_eq!(results.len(), LEVELS);
        for node in results.iter() {
            assert!(matches!(node.data, Syntax::OrgData));
        }
    }
}
