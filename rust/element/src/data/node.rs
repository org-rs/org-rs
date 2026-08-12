use crate::affiliated::AffiliatedData;
use memchr::memchr;

use super::{BumpVec, Interval, NodeId, Syntax};

#[derive(Debug)]
pub struct SyntaxNode<'a, 'b> {
    /// Parent node in the forest, or `None` for the root.
    pub parent: Option<NodeId>,
    /// Child node indices in the arena.
    pub children: BumpVec<'b, NodeId>,

    pub data: Syntax<'a, 'b>,

    /// holds `begin` and `end`
    pub location: Interval,

    /// holds `contents_begin` and `contents_end`
    pub content_location: Option<Interval>,

    /// Holds the number of blank lines, or white spaces, at its end.
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

    /// Create a fallback paragraph node spanning to the end of the current line.
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
