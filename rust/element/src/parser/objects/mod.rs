mod emphasis;
mod inline;
mod links;
mod markup;
mod text;
mod timestamp;

use super::Parser;
use crate::{
    data::{BumpVec, Interval, NodeId, Syntax, SyntaxNode, SyntaxT},
    environment,
};

impl<'a, 'b, Environment: environment::Environment> Parser<'a, 'b, Environment> {
    /// Parse objects between `beg` and `end` and return recursive structure.
    #[inline(never)]
    pub fn parse_objects(
        &mut self,
        interval: impl Into<Interval>,
        restriction: impl Fn(SyntaxT) -> bool,
    ) -> BumpVec<'b, NodeId> {
        let interval = interval.into();
        // Mark this region's start so an emphasis marker sitting at the
        // boundary passes the pre-character check. Saved/restored to keep the
        // enclosing region's boundary intact across nested object parsing.
        let saved_region_start = self.object_region_start;
        self.object_region_start = interval.start;
        let mut children: BumpVec<'b, NodeId> = BumpVec::new_in(self.bump);
        let mut pos = interval.start;

        while pos < interval.end {
            let remaining = &self.input[pos..interval.end];

            let consumed = {
                let bytes = remaining.as_bytes();
                if bytes.is_empty() {
                    None
                } else {
                    match bytes[0] {
                        b'*' => self.try_parse_bold(remaining, pos).map(|(n, c)| {
                            if restriction(SyntaxT::Bold) {
                                children.push(n);
                            }
                            c
                        }),
                        b'/' => self.try_parse_italic(remaining, pos).map(|(n, c)| {
                            if restriction(SyntaxT::Italic) {
                                children.push(n);
                            }
                            c
                        }),
                        b'~' => self.try_parse_code(remaining, pos).map(|(n, c)| {
                            if restriction(SyntaxT::Code) {
                                children.push(n);
                            }
                            c
                        }),
                        b'=' => self.try_parse_verbatim(remaining, pos).map(|(n, c)| {
                            if restriction(SyntaxT::Verbatim) {
                                children.push(n);
                            }
                            c
                        }),
                        b'_' => {
                            if let Some((n, c)) =
                                self.try_parse_script(remaining, pos, crate::data::ScriptKind::Sub)
                            {
                                if restriction(SyntaxT::Script) {
                                    children.push(n);
                                }
                                Some(c)
                            } else {
                                self.try_parse_underline(remaining, pos).map(|(n, c)| {
                                    if restriction(SyntaxT::Underline) {
                                        children.push(n);
                                    }
                                    c
                                })
                            }
                        }
                        b'^' => self
                            .try_parse_script(remaining, pos, crate::data::ScriptKind::Sup)
                            .map(|(n, c)| {
                                if restriction(SyntaxT::Script) {
                                    children.push(n);
                                }
                                c
                            }),
                        b'+' => self.try_parse_strikethrough(remaining, pos).map(|(n, c)| {
                            if restriction(SyntaxT::StrikeThrough) {
                                children.push(n);
                            }
                            c
                        }),
                        b'[' => {
                            let mut result = None;
                            // A statistics cookie is only recognised where it is
                            // permitted (e.g. not inside a table cell); otherwise
                            // `[/]` stays plain text, matching Emacs.
                            if restriction(SyntaxT::StatisticsCookie) {
                                if let Some((n, c)) =
                                    self.try_parse_statistics_cookie(remaining, pos)
                                {
                                    children.push(n);
                                    result = Some(c);
                                }
                            }
                            if result.is_none() {
                                if let Some((n, c)) =
                                    self.try_parse_footnote_reference(remaining, pos)
                                {
                                    if restriction(SyntaxT::FootnoteReference) {
                                        children.push(n);
                                    }
                                    result = Some(c);
                                } else if let Some((n, c)) =
                                    self.try_parse_timestamp(remaining, pos)
                                {
                                    if restriction(SyntaxT::Timestamp) {
                                        children.push(n);
                                    }
                                    result = Some(c);
                                } else if let Some((n, c)) = self.try_parse_citation(remaining, pos)
                                {
                                    // Only consume bytes when Citation is permitted;
                                    // otherwise `[cite:...]` stays as plain text,
                                    // matching Emacs behaviour in e.g. table cells.
                                    if restriction(SyntaxT::Citation) {
                                        children.push(n);
                                        result = Some(c);
                                    }
                                } else if let Some((n, c)) = self.try_parse_link(remaining, pos) {
                                    if restriction(SyntaxT::Link) {
                                        children.push(n);
                                    }
                                    result = Some(c);
                                }
                            }
                            result
                        }
                        b'\\' => {
                            let mut result = None;
                            if let Some((n, c)) = self.try_parse_latex_math(remaining, pos) {
                                if restriction(SyntaxT::LatexFragment) {
                                    children.push(n);
                                }
                                result = Some(c);
                            } else if let Some((n, c)) = self.try_parse_line_break(remaining, pos) {
                                if restriction(SyntaxT::LineBreak) {
                                    children.push(n);
                                }
                                result = Some(c);
                            } else if let Some((n, c)) =
                                self.try_parse_latex_fragment(remaining, pos)
                            {
                                if restriction(SyntaxT::LatexFragment) {
                                    children.push(n);
                                }
                                result = Some(c);
                            } else if let Some((n, c)) = self.try_parse_entity(remaining, pos) {
                                if restriction(SyntaxT::Entity) {
                                    children.push(n);
                                }
                                result = Some(c);
                            }
                            result
                        }
                        b'$' => {
                            let mut result = None;
                            if let Some((n, c)) = self.try_parse_latex_math(remaining, pos) {
                                if restriction(SyntaxT::LatexFragment) {
                                    children.push(n);
                                }
                                result = Some(c);
                            }
                            result
                        }
                        b'<' => {
                            let mut result = None;
                            if let Some((n, c)) = self.try_parse_timestamp(remaining, pos) {
                                if restriction(SyntaxT::Timestamp) {
                                    children.push(n);
                                }
                                result = Some(c);
                            } else if let Some((n, c)) = self.try_parse_radio_target(remaining, pos)
                            {
                                if restriction(SyntaxT::RadioTarget) {
                                    children.push(n);
                                }
                                result = Some(c);
                            } else if let Some((n, c)) = self.try_parse_target(remaining, pos) {
                                if restriction(SyntaxT::Target) {
                                    children.push(n);
                                }
                                result = Some(c);
                            } else if let Some((n, c)) = self.try_parse_angle_link(remaining, pos) {
                                if restriction(SyntaxT::Link) {
                                    children.push(n);
                                }
                                result = Some(c);
                            }
                            result
                        }
                        b'{' => self.try_parse_macro(remaining, pos).map(|(n, c)| {
                            if restriction(SyntaxT::Macro) {
                                children.push(n);
                            }
                            c
                        }),
                        b'c' => self
                            .try_parse_inline_babel_call(remaining, pos)
                            .map(|(n, c)| {
                                if restriction(SyntaxT::InlineBabelCall) {
                                    children.push(n);
                                }
                                c
                            }),
                        // Inline source block: `src_<lang>{body}` or
                        // `src_<lang>[:headers]{body}`.
                        b's' => self
                            .try_parse_inline_src_block(remaining, pos)
                            .map(|(n, c)| {
                                if restriction(SyntaxT::InlineSrcBlock) {
                                    children.push(n);
                                }
                                c
                            }),
                        // Plain-link start bytes come from the environment so
                        // the recognised types stay configurable.
                        _ if self.environment.link_start_bytes().contains(&bytes[0]) => {
                            self.try_parse_plain_link(remaining, pos).map(|(n, c)| {
                                if restriction(SyntaxT::Link) {
                                    children.push(n);
                                } else {
                                    // Link not allowed in
                                    // (e.g. another link).  Absorb
                                    // the URL text into the preceding
                                    // PlainText so the node
                                    // boundaries match Emacs.
                                    let fused = children.last().copied().and_then(|prev| {
                                        if let Syntax::PlainText(_) = &self.arena[prev].data {
                                            if self.arena[prev].location.end == pos {
                                                let new_start = self.arena[prev].location.start;
                                                return Some((prev, new_start, pos + c));
                                            }
                                        }
                                        None
                                    });
                                    if let Some((prev, new_start, new_end)) = fused {
                                        self.arena[prev].data =
                                            Syntax::PlainText(&self.input[new_start..new_end]);
                                        self.arena[prev].location = crate::data::Interval {
                                            start: new_start,
                                            end: new_end,
                                        };
                                    } else {
                                        let pt = self.arena.alloc(
                                            SyntaxNode::new(
                                                Syntax::PlainText(&self.input[pos..pos + c]),
                                                (pos, pos + c),
                                                self.bump,
                                            )
                                            .build(),
                                        );
                                        children.push(pt);
                                    }
                                }
                                c
                            })
                        }
                        _ => None,
                    }
                }
            };

            if let Some(c) = consumed {
                pos += c;
            } else if let Some((node, c)) = self.try_parse_radio_link(remaining, pos) {
                children.push(node);
                pos += c;
            } else if let Some((node, c)) = self.try_parse_plain_text(remaining, pos) {
                // Coalesce with the previous child if it is also a PlainText
                // ending exactly here.  This fuses the two pieces that result
                // when scan_plain_text_end stops at a markup-potential character
                // ('+', '/', …) whose subsequent markup parse then fails.
                let fused = children.last().copied().and_then(|prev| {
                    if let Syntax::PlainText(_) = &self.arena[prev].data {
                        if self.arena[prev].location.end == pos {
                            let new_start = self.arena[prev].location.start;
                            let new_end = pos + c;
                            return Some((prev, new_start, new_end));
                        }
                    }
                    None
                });
                if let Some((prev, new_start, new_end)) = fused {
                    self.arena[prev].data = Syntax::PlainText(&self.input[new_start..new_end]);
                    self.arena[prev].location = crate::data::Interval {
                        start: new_start,
                        end: new_end,
                    };
                } else {
                    children.push(node);
                }
                pos += c;
            } else {
                // No construct recognised and scan_plain_text_end treats this
                // byte as an unconditional stop (e.g. `[`, `\`, `<`).
                // Extend the previous PlainText by one byte, or start a new one,
                // so the node boundary matches Emacs (which keeps such bytes in
                // the surrounding PlainText rather than creating a gap).
                let fused = children.last().copied().and_then(|prev| {
                    if let Syntax::PlainText(_) = &self.arena[prev].data {
                        if self.arena[prev].location.end == pos {
                            let new_start = self.arena[prev].location.start;
                            return Some((prev, new_start, pos + 1));
                        }
                    }
                    None
                });
                if let Some((prev, new_start, new_end)) = fused {
                    self.arena[prev].data = Syntax::PlainText(&self.input[new_start..new_end]);
                    self.arena[prev].location = crate::data::Interval {
                        start: new_start,
                        end: new_end,
                    };
                } else {
                    let byte_node = self.arena.alloc(
                        SyntaxNode::new(
                            Syntax::PlainText(&self.input[pos..pos + 1]),
                            (pos, pos + 1),
                            self.bump,
                        )
                        .build(),
                    );
                    children.push(byte_node);
                }
                pos += 1;
            }
        }

        self.object_region_start = saved_region_start;
        children
    }
}
