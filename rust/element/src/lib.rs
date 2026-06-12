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

#[macro_use]
extern crate lazy_static;
extern crate memchr;
extern crate regex;

#[macro_use]
pub mod parser;
pub mod affiliated;
pub mod babel;
pub mod blocks;
pub mod cursor;
pub mod data;
pub mod drawer;
pub mod environment;
pub mod fixed_width;
pub mod headline;
pub mod keyword;
pub mod latex;
pub mod list;
pub mod markup;
pub mod paragraph;
pub mod planning;
pub mod table;

/// Everything a user of this library is likely to need, in one place.
///
/// ```rust,ignore
/// use org_element::prelude::*;
/// ```
///
/// There are no name conflicts amongst the re-exported items: every public
/// type in the crate has a unique name.
pub mod prelude {
    pub use crate::affiliated::{AffiliatedData, DualVal, ElementSpan, ElementSpanBuilder};
    pub use crate::babel::BabelCallData;
    pub use crate::blocks::{
        BlockFlags, DynamicBlockData, ExampleBlockData, ExportBlockData, SpecialBlockData,
        SrcBlockData,
    };
    pub use crate::data::{
        Brackets, ClockData, ClockStatus, EntityData, EntityFlags, ExportSnippetData,
        FootnoteReferenceData, InlineBabelCallData, InlineSrcBlockData, LineNumberingMode,
        LinkData, LinkFlags, LinkFormat, LinkType, MacroData, PlanningData, RadioTargetData,
        RepeaterType, ScriptFlags, ScriptKind, StatisticsCookieData, StringOrObject, TimeUnit,
        TimestampData, TimestampType, WarningType,
    };
    pub use crate::data::{
        Interval, NodeArena, Nodes, Syntax, SyntaxNode, SyntaxNodeBuilder, SyntaxT,
    };
    pub use crate::environment::{DefaultEnvironment, Environment};
    pub use crate::headline::{
        HeadlineData, HeadlineFlags, InlineTaskData, NodePropertyData, Tag, TodoKeyword,
    };
    pub use crate::keyword::KeywordData;
    pub use crate::latex::LatexEnvironmentData;
    pub use crate::list::{CheckBox, ItemData, ListItem, ListKind, ListStruct, PlainListData};
    pub use crate::markup::FootnoteDefinitionData;
    pub use crate::parser::{ParseGranularity, Parser, ParserMode};
    pub use crate::table::{
        Col, Row, SpreadsheetCellData, SpreadsheetData, SpreadsheetRowData, TableRowType,
    };
    pub use bumpalo;
}

#[cfg(test)]
mod parser_tests;
