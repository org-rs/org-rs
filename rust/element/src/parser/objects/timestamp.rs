use super::super::Parser;
use crate::{
    data::{NodeId, Syntax, SyntaxNode},
    environment,
};

impl<'a, 'b, Environment: environment::Environment> Parser<'a, 'b, Environment> {
    #[expect(
        clippy::missing_inline_in_public_items,
        reason = "This function is both huge, so adding `inline` to it might cause problems downstream, and on the hot path, meaning that `inline(never)` would disable inlining"
    )]
    pub fn try_parse_timestamp(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
        let (timestamp_data, consumed) = self.parse_timestamp(text)?;

        // Absorb trailing spaces/tabs into the timestamp's post-blank,
        // matching Emacs org-element behaviour.
        let post_blank = text[consumed..]
            .bytes()
            .take_while(|&b| b == b' ' || b == b'\t')
            .count();
        let total = consumed + post_blank;

        let node = self.arena.alloc(
            SyntaxNode::new(
                Syntax::Timestamp(self.bump.alloc(timestamp_data)),
                (start, start + total),
                self.bump,
            )
            .content((start + 1, start + consumed - 1))
            .post_blank(post_blank)
            .build(),
        );

        Some((node, total))
    }
}
