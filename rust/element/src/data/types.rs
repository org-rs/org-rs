use super::Syntax;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SyntaxT {
    OrgData,
    BabelCall,
    CenterBlock,
    Clock,
    Comment,
    CommentBlock,
    DiarySexp,
    Drawer,
    DynamicBlock,
    ExampleBlock,
    ExportBlock,
    FixedWidth,
    FootnoteDefinition,
    Headline,
    HorizontalRule,
    InlineTask,
    Item,
    Keyword,
    LatexEnvironment,
    NodeProperty,
    Paragraph,
    PlainList,
    Planning,
    PropertyDrawer,
    QuoteBlock,
    Section,
    SpecialBlock,
    SrcBlock,
    Table,
    TableRow,
    VerseBlock,
    Bold,
    Code,
    Citation,
    CitationReference,
    Entity,
    FootnoteReference,
    InlineBabelCall,
    InlineSrcBlock,
    Italic,
    LineBreak,
    LatexFragment,
    Link,
    Macro,
    RadioTarget,
    StatisticsCookie,
    StrikeThrough,
    Script,
    TableCell,
    Target,
    Timestamp,
    Underline,
    Verbatim,
    PlainText,
}

impl std::fmt::Display for SyntaxT {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            SyntaxT::OrgData => "OrgData",
            SyntaxT::BabelCall => "BabelCall",
            SyntaxT::CenterBlock => "CenterBlock",
            SyntaxT::Clock => "Clock",
            SyntaxT::Comment => "Comment",
            SyntaxT::CommentBlock => "CommentBlock",
            SyntaxT::DiarySexp => "DiarySexp",
            SyntaxT::Drawer => "Drawer",
            SyntaxT::DynamicBlock => "DynamicBlock",
            SyntaxT::ExampleBlock => "ExampleBlock",
            SyntaxT::ExportBlock => "ExportBlock",
            SyntaxT::FixedWidth => "FixedWidth",
            SyntaxT::FootnoteDefinition => "FootnoteDefinition",
            SyntaxT::Headline => "Headline",
            SyntaxT::HorizontalRule => "HorizontalRule",
            SyntaxT::InlineTask => "InlineTask",
            SyntaxT::Item => "Item",
            SyntaxT::Keyword => "Keyword",
            SyntaxT::LatexEnvironment => "LatexEnvironment",
            SyntaxT::NodeProperty => "NodeProperty",
            SyntaxT::Paragraph => "Paragraph",
            SyntaxT::PlainList => "PlainList",
            SyntaxT::Planning => "Planning",
            SyntaxT::PropertyDrawer => "PropertyDrawer",
            SyntaxT::QuoteBlock => "QuoteBlock",
            SyntaxT::Section => "Section",
            SyntaxT::SpecialBlock => "SpecialBlock",
            SyntaxT::SrcBlock => "SrcBlock",
            SyntaxT::Table => "Table",
            SyntaxT::TableRow => "TableRow",
            SyntaxT::VerseBlock => "VerseBlock",
            SyntaxT::Bold => "Bold",
            SyntaxT::Code => "Code",
            SyntaxT::Citation => "Citation",
            SyntaxT::CitationReference => "CitationReference",
            SyntaxT::Entity => "Entity",
            SyntaxT::FootnoteReference => "FootnoteReference",
            SyntaxT::InlineBabelCall => "InlineBabelCall",
            SyntaxT::InlineSrcBlock => "InlineSrcBlock",
            SyntaxT::Italic => "Italic",
            SyntaxT::LineBreak => "LineBreak",
            SyntaxT::LatexFragment => "LatexFragment",
            SyntaxT::Link => "Link",
            SyntaxT::Macro => "Macro",
            SyntaxT::RadioTarget => "RadioTarget",
            SyntaxT::StatisticsCookie => "StatisticsCookie",
            SyntaxT::StrikeThrough => "StrikeThrough",
            SyntaxT::Script => "Script",
            SyntaxT::TableCell => "TableCell",
            SyntaxT::Target => "Target",
            SyntaxT::Timestamp => "Timestamp",
            SyntaxT::Underline => "Underline",
            SyntaxT::Verbatim => "Verbatim",
            SyntaxT::PlainText => "PlainText",
        })
    }
}

impl<'a> From<&'a Syntax<'_, '_>> for SyntaxT {
    #[inline]
    fn from(value: &'a Syntax) -> Self {
        match value {
            Syntax::OrgData => SyntaxT::OrgData,
            Syntax::BabelCall(_) => SyntaxT::BabelCall,
            Syntax::CenterBlock => SyntaxT::CenterBlock,
            Syntax::Clock(_) => SyntaxT::Clock,
            Syntax::Comment(_) => SyntaxT::Comment,
            Syntax::CommentBlock(_) => SyntaxT::CommentBlock,
            Syntax::DiarySexp(_) => SyntaxT::DiarySexp,
            Syntax::Drawer(_) => SyntaxT::Drawer,
            Syntax::DynamicBlock(_) => SyntaxT::DynamicBlock,
            Syntax::ExampleBlock(_) => SyntaxT::ExampleBlock,
            Syntax::ExportBlock(_) => SyntaxT::ExportBlock,
            Syntax::FixedWidth(_) => SyntaxT::FixedWidth,
            Syntax::FootnoteDefinition(_) => SyntaxT::FootnoteDefinition,
            Syntax::Headline(_) => SyntaxT::Headline,
            Syntax::HorizontalRule => SyntaxT::HorizontalRule,
            Syntax::InlineTask(_) => SyntaxT::InlineTask,
            Syntax::Item(_) => SyntaxT::Item,
            Syntax::Keyword(_) => SyntaxT::Keyword,
            Syntax::LatexEnvironment(_) => SyntaxT::LatexEnvironment,
            Syntax::NodeProperty(_) => SyntaxT::NodeProperty,
            Syntax::Paragraph => SyntaxT::Paragraph,
            Syntax::PlainList(_) => SyntaxT::PlainList,
            Syntax::Planning(_) => SyntaxT::Planning,
            Syntax::PropertyDrawer => SyntaxT::PropertyDrawer,
            Syntax::QuoteBlock => SyntaxT::QuoteBlock,
            Syntax::Section => SyntaxT::Section,
            Syntax::SpecialBlock(_) => SyntaxT::SpecialBlock,
            Syntax::SrcBlock(_) => SyntaxT::SrcBlock,
            Syntax::Table => SyntaxT::Table,
            Syntax::TableRow(_) => SyntaxT::TableRow,
            Syntax::VerseBlock => SyntaxT::VerseBlock,
            Syntax::Bold => SyntaxT::Bold,
            Syntax::Code(_) => SyntaxT::Code,
            Syntax::Citation(_) => SyntaxT::Citation,
            Syntax::CitationReference(_) => SyntaxT::CitationReference,
            Syntax::Entity(_) => SyntaxT::Entity,
            Syntax::FootnoteReference(_) => SyntaxT::FootnoteReference,
            Syntax::InlineBabelCall(_) => SyntaxT::InlineBabelCall,
            Syntax::InlineSrcBlock(_) => SyntaxT::InlineSrcBlock,
            Syntax::Italic => SyntaxT::Italic,
            Syntax::LineBreak => SyntaxT::LineBreak,
            Syntax::LatexFragment(_) => SyntaxT::LatexFragment,
            Syntax::Link(_) => SyntaxT::Link,
            Syntax::Macro(_) => SyntaxT::Macro,
            Syntax::RadioTarget(_) => SyntaxT::RadioTarget,
            Syntax::StatisticsCookie(_) => SyntaxT::StatisticsCookie,
            Syntax::StrikeThrough => SyntaxT::StrikeThrough,
            Syntax::Script(_) => SyntaxT::Script,
            Syntax::TableCell => SyntaxT::TableCell,
            Syntax::Target(_) => SyntaxT::Target,
            Syntax::Timestamp(_) => SyntaxT::Timestamp,
            Syntax::Underline => SyntaxT::Underline,
            Syntax::Verbatim(_) => SyntaxT::Verbatim,
            Syntax::PlainText(_) => SyntaxT::PlainText,
        }
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
        )
    }

    pub fn is_object(self) -> bool {
        use SyntaxT::*;
        matches!(
            self,
            Bold | Citation
                | Code
                | Entity
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
        )
    }

    pub fn is_object_container(self) -> bool {
        use SyntaxT::*;
        matches!(
            self,
            Paragraph
                | TableRow
                | VerseBlock
                | Bold
                | Citation
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

    #[inline]
    pub fn can_contain(self, that: SyntaxT) -> bool {
        fn is_from_standard_set(that: SyntaxT) -> bool {
            match that {
                SyntaxT::TableCell => false,
                x if x.is_object() => true,
                _ => false,
            }
        }

        fn is_from_standard_set_no_line_break(that: SyntaxT) -> bool {
            match that {
                SyntaxT::LineBreak => false,
                x => is_from_standard_set(x),
            }
        }

        use SyntaxT::*;
        match self {
            Bold | Italic | FootnoteReference | Paragraph | StrikeThrough | Script | Underline
            | VerseBlock => is_from_standard_set(that),

            Headline | InlineTask | Item => is_from_standard_set_no_line_break(that),

            Keyword => match that {
                FootnoteReference => false,
                x => is_from_standard_set(x),
            },

            Link => matches!(
                that,
                Bold | Code
                    | Entity
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

            Citation => matches!(
                that,
                CitationReference
                    | Bold
                    | Code
                    | Entity
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

            RadioTarget => matches!(
                that,
                Bold | Code | Entity | Italic | LatexFragment | StrikeThrough | Script | Underline
            ),

            TableCell => matches!(
                that,
                Bold | Citation
                    | CitationReference
                    | Code
                    | Entity
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

            TableRow => matches!(that, TableCell),

            _ => false,
        }
    }
}
