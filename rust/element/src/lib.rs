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
pub mod directive;
pub mod drawer;
pub mod environment;
pub mod from_input;

pub mod headline;
pub mod keyword;
pub mod latex;
pub mod list;
pub mod markup;
pub mod paragraph;
pub mod planning;
pub mod table;
#[cfg(feature = "testutils")]
pub mod testutils;

/// A complete parsed org-mode document with self-owned memory.
///
/// Returned by [`parse`].  The `Forest` field is directly accessible for
/// traversal; the bump allocator is owned internally and lives alongside it.
pub struct ParsedDoc<'a> {
    pub input: &'a str,
    pub forest: data::ParseForest<'a>,
}

impl<'a> ParsedDoc<'a> {
    #[inline]
    pub fn text_of(&self, id: data::NodeId) -> &'a str {
        self.forest.text_of(id, self.input)
    }
}

/// Parse an org-mode document with default settings.
///
/// Uses [`ParseGranularity::Object`] (full parse) and [`DefaultEnvironment`].
/// For custom settings, use [`Parser`] directly with your own bump.
///
/// [`ParseGranularity::Object`]: parser::ParseGranularity::Object
/// [`DefaultEnvironment`]: environment::DefaultEnvironment
/// [`Parser`]: parser::Parser
pub fn parse(input: &str) -> ParsedDoc<'_> {
    parse_with_granularity(input, parser::ParseGranularity::Object)
}

fn parse_with_granularity(input: &str, granularity: parser::ParseGranularity) -> ParsedDoc<'_> {
    let bump: Box<bumpalo::Bump> = Box::new(bumpalo::Bump::new());
    let (arena, root) = {
        let mut p =
            parser::Parser::new(input, granularity, environment::DefaultEnvironment, &*bump);
        p.parse_buffer()
    };
    // SAFETY: `bump` is boxed and will be stored in `arena._bumps`, keeping it
    // alive for as long as the arena.  All node data was allocated in `bump`, so
    // the borrows remain valid.
    let mut arena: data::ChunkArena<'_, '_> = unsafe { std::mem::transmute(arena) };
    arena._bumps.push(bump);
    ParsedDoc {
        input,
        forest: data::Forest::from_chunks(vec![arena], root),
    }
}

impl<'a> from_input::FromInput<'a> for ParsedDoc<'a> {
    type Err = std::convert::Infallible;
    fn from_input(s: &'a str) -> Result<Self, Self::Err> {
        Ok(parse(s))
    }
}

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
    pub use crate::blocks::BlockName;
    pub use crate::blocks::{
        BlockFlags, DynamicBlockData, ExampleBlockData, ExportBlockData, SpecialBlockData,
        SrcBlockData,
    };
    pub use crate::data::{
        Brackets, ClockData, EntityData, FootnoteReferenceData, InlineBabelCallData,
        InlineSrcBlockData, LineNumberingMode, LinkData, LinkFlags, LinkFormat, LinkType,
        MacroData, PlanningData, RadioTargetData, RepeaterType, ScriptFlags, ScriptKind,
        StatisticsCookieData, StringOrObject, TimeUnit, TimestampData, TimestampType, WarningType,
    };
    pub use crate::data::{
        ChunkArena, ChunkNodes, Forest, Interval, NodeArena, NodeEntries, NodeId, NodeIds, Nodes,
        NodesByType, ParseArena, ParseForest, PostOrderIds, Syntax, SyntaxNode, SyntaxNodeBuilder,
        SyntaxT,
    };
    pub use crate::directive::HashDirective;
    pub use crate::environment::{ComplianceEnvironment, DefaultEnvironment, Environment};
    pub use crate::from_input::{FromInput, InputExt};
    pub use crate::headline::{
        HeadlineData, HeadlineFlags, InlineTaskData, NodePropertyData, Tag, TodoKeyword,
    };
    pub use crate::keyword::KeywordData;
    pub use crate::latex::LatexEnvironmentData;
    pub use crate::list::{CheckBox, ItemData, ListItem, ListKind, ListStruct, PlainListData};
    pub use crate::markup::FootnoteDefinitionData;
    pub use crate::parser::{ParseGranularity, Parser, ParserMode};
    pub use crate::table::TableRowType;
    pub use crate::{parse, ParsedDoc};
    pub use bumpalo;
}
