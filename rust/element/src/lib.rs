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

#![warn(clippy::all)]
#![warn(clippy::missing_inline_in_public_items)]
// This should be eventually turned off, but for now this helps reduce the noise
#![allow(dead_code)]
#[macro_use]
extern crate lazy_static;
extern crate memchr;
extern crate regex;

#[macro_use]
pub mod parser;
mod affiliated;
mod babel;
pub mod blocks;
pub mod cursor;
pub mod data;
mod drawer;
pub mod environment;
mod fixed_width;
pub mod headline;
pub mod keyword;
mod latex;
mod list;
pub mod markup;
mod paragraph;
mod planning;
mod table;

/// Everything a user of this library is likely to need, in one place.
///
/// ```rust,ignore
/// use org_element::prelude::*;
/// ```
///
/// There are no name conflicts among the re-exported items: every public
/// type in the crate has a unique name.
pub mod prelude {
    pub use crate::data::{Interval, NodeArena, Nodes, Syntax, SyntaxNode, SyntaxNodeBuilder, SyntaxT};
    pub use crate::data::{
        ClockData, ClockStatus, EntityData,
        ExportSnippetData, FootnoteReferenceData, InlineBabelCallData,
        InlineSrcBlockData, LineNumberingMode, LinkData, LinkFormat, LinkType,
        MacroData, PlanningData, RadioTargetData, StatisticsCookieData,
        StringOrObject, Brackets, ScriptFlags, ScriptKind,
        TimestampData, TimestampType, RepeaterType, TimeUnit,
        WarningType,
    };
    pub use crate::affiliated::{AffiliatedData, DualVal, ElementSpan, ElementSpanBuilder};
    pub use crate::babel::BabelCallData;
    pub use crate::blocks::{
        DynamicBlockData, ExampleBlockData, ExportBlockData, SpecialBlockData, SrcBlockData,
    };
    pub use crate::headline::{
        HeadlineData, InlineTaskData, NodePropertyData, Tag, TodoKeyword,
    };
    pub use crate::keyword::KeywordData;
    pub use crate::latex::LatexEnvironmentData;
    pub use crate::list::{CheckBox, ItemData, ListItem, ListKind, ListStruct, PlainListData};
    pub use crate::markup::FootnoteDefinitionData;
    pub use crate::table::{
        Col, Row, SpreadsheetCellData, SpreadsheetData, SpreadsheetRowData, TableRowType,
    };
    pub use crate::parser::{ParseGranularity, Parser, ParserMode};
    pub use crate::environment::{DefaultEnvironment, Environment};
}

#[cfg(test)]
mod parser_tests;
