use crate::affiliated::ElementSpan;
use crate::data::{BumpVec, Interval, NodeId, Syntax, SyntaxNode};
use crate::parser::{ParseGranularity, Parser, ParserMode};
use memchr::memchr;

use super::{
    desc_content_start, CheckBox, ItemData, ListItem, ListKind, ListStruct, PlainListData,
};

impl<'a, 'b, Environment: crate::environment::Environment> Parser<'a, 'b, Environment> {
    /// Fallback: item parser (not yet fully implemented).
    #[inline]
    pub fn item_parser(&mut self) -> NodeId {
        let start = self.cursor.pos();
        self.arena.alloc(SyntaxNode::fallback(
            self.input,
            start,
            self.input.len(),
            self.bump,
        ))
    }

    fn item_parents(items: &[ListItem<'a>]) -> Vec<Option<usize>> {
        let n = items.len();
        let mut parent: Vec<Option<usize>> = vec![None; n];
        let mut stack: Vec<(usize, usize)> = Vec::new();
        for i in 0..n {
            let indent = items[i].indent;
            while let Some(&(top_i, _)) = stack.last() {
                if top_i >= indent {
                    stack.pop();
                } else {
                    break;
                }
            }
            if let Some(&(_, p)) = stack.last() {
                parent[i] = Some(p);
            }
            stack.push((indent, i));
        }
        parent
    }

    /// Plain list parser — groups items by (parent, indent) to create
    /// separate PlainLists for each indentation level.
    #[inline]
    pub fn plain_list_parser(
        &mut self,
        element_span: ElementSpan<'a, 'b>,
        structure: &'b ListStruct<'a, 'b>,
    ) -> NodeId {
        let span = element_span.span;
        let items = &structure.items;
        if items.is_empty() {
            return self.arena.alloc(SyntaxNode::fallback(
                self.input, span.start, span.end, self.bump,
            ));
        }

        let parent = Self::item_parents(items);

        let mut groups: Vec<(usize, usize)> = Vec::new();
        {
            let mut i = 0;
            while i < items.len() {
                let p = parent[i];
                let indent = items[i].indent;
                let mut j = i + 1;
                while j < items.len() {
                    if parent[j] != p {
                        break;
                    }
                    if p.is_none() && items[j].indent != indent {
                        break;
                    }
                    j += 1;
                }
                groups.push((i, j));
                i = j;
            }
        }

        let list_type = Self::get_list_type(items);
        let mut children = BumpVec::new_in(self.bump);

        let should_recurse = matches!(
            self.granularity,
            ParseGranularity::Element | ParseGranularity::Object
        );

        let first_root_indent: Option<usize> = groups
            .iter()
            .find(|&&(s, _)| parent[s].is_none())
            .map(|&(s, _)| items[s].indent);

        for &(start, end) in &groups {
            if parent[start].is_some() {
                continue;
            }
            if first_root_indent.map_or(false, |fi| items[start].indent != fi) {
                break;
            }

            for idx in start..end {
                let item = &items[idx];
                let item_indent = item.indent;

                let end_pos = {
                    let mut found = None;
                    for k in (idx + 1)..items.len() {
                        if items[k].indent <= item_indent {
                            found = Some(items[k].position);
                            break;
                        }
                    }
                    found.unwrap_or(structure.end)
                };

                let item_node = self.item_parser_internal(item, end_pos);
                children.push(item_node);

                if should_recurse {
                    if let Some(loc) = self.arena[item_node].content_location {
                        let saved_ctx = self.item_indent_ctx;
                        self.item_indent_ctx = Some(item_indent);
                        let item_children = self.parse_elements(loc, ParserMode::Planning, None);
                        self.item_indent_ctx = saved_ctx;
                        self.arena.set_children(item_node, item_children);
                    }
                }
            }
        }

        let end = if let Some(last) = children.last() {
            self.arena[*last].location.end
        } else {
            span.end
        };

        let list_data = PlainListData {
            structure,
            type_s: list_type,
        };

        self.arena.alloc_with_children(
            SyntaxNode::new(Syntax::PlainList(list_data), (span.start, end), self.bump)
                .affiliated(element_span.affiliated)
                .build(),
            children,
        )
    }

    fn get_list_type(items: &[ListItem<'a>]) -> ListKind {
        if items.is_empty() {
            return ListKind::Unordered;
        }
        if items[0].tag.is_some() {
            return ListKind::Descriptive;
        }
        for item in items {
            if item
                .bullet
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
            {
                return ListKind::Ordered;
            }
        }
        ListKind::Unordered
    }

    fn item_content_end(&self, content_start: usize, end: usize, item_indent: usize) -> usize {
        let bytes = self.input.as_bytes();
        let mut pos = content_start;

        while pos < end && bytes.get(pos) == Some(&b'\n') {
            pos += 1;
        }

        let mut in_block = false;
        while pos < end {
            let rest = &bytes[pos..];
            match memchr(b'\n', rest) {
                None => break,
                Some(nl_pos) => {
                    let line = &rest[..nl_pos];
                    let is_blank = line.iter().all(|&b| b == b' ' || b == b'\t');
                    let next_line_pos = pos + nl_pos + 1;

                    let ws = line
                        .iter()
                        .position(|&b| b != b' ' && b != b'\t')
                        .unwrap_or(line.len());
                    let trimmed = &line[ws..];
                    if in_block {
                        if trimmed.len() >= 5
                            && trimmed[0] == b'#'
                            && trimmed[1] == b'+'
                            && trimmed[2..5].eq_ignore_ascii_case(b"END")
                        {
                            in_block = false;
                        }
                        pos = next_line_pos;
                        continue;
                    }
                    if trimmed.len() >= 7
                        && trimmed[0] == b'#'
                        && trimmed[1] == b'+'
                        && trimmed[2..7].eq_ignore_ascii_case(b"BEGIN")
                    {
                        let (indent, _) = Self::get_indent(
                            std::str::from_utf8(line).unwrap_or(""),
                            self.tab_width,
                        );
                        if indent > item_indent {
                            in_block = true;
                            pos = next_line_pos;
                            continue;
                        }
                    }

                    if is_blank && next_line_pos <= end {
                        let after_blank = &bytes[next_line_pos..end];
                        let next_nonblank = match memchr(b'\n', after_blank) {
                            Some(pos2) => &after_blank[..pos2],
                            None => after_blank,
                        };
                        if !next_nonblank.is_empty() {
                            let (next_indent, _) = Self::get_indent(
                                std::str::from_utf8(next_nonblank).unwrap_or(""),
                                self.tab_width,
                            );
                            if next_indent <= item_indent {
                                return pos;
                            }
                        } else {
                            return pos;
                        }
                    }
                    pos = next_line_pos;
                }
            }
        }
        end
    }

    fn item_parser_internal(&mut self, item: &ListItem<'a>, end: usize) -> NodeId {
        let bullet = item.bullet;
        let tag = item.tag;

        let bytes = self.input.as_bytes();
        let ws_end = bytes[item.position..]
            .iter()
            .position(|&b| b != b' ' && b != b'\t')
            .unwrap_or(0);
        let bullet_end = item.position + ws_end + item.bullet.len();
        let mut after_bullet = bullet_end;
        while after_bullet < end && (bytes[after_bullet] == b' ' || bytes[after_bullet] == b'\t') {
            after_bullet += 1;
        }

        let content_start = if tag.is_some() {
            desc_content_start(self.input, after_bullet, end).unwrap_or(after_bullet)
        } else if item.checkbox.is_some() {
            after_bullet + 4
        } else {
            after_bullet
        };

        let effective_content_end = self.item_content_end(content_start, end, item.indent);

        let content_location = (content_start < effective_content_end).then_some(Interval {
            start: content_start,
            end: effective_content_end,
        });

        let item_data = ItemData {
            bullet,
            checkbox: item.checkbox.clone(),
            counter: item.counter.unwrap_or(0),
            pre_blank: 0,
            raw_tag: tag,
            tag,
        };

        let mut node = SyntaxNode::new(
            Syntax::Item(self.bump.alloc(item_data)),
            (item.position, end),
            self.bump,
        );
        if let Some(loc) = content_location {
            node = node.content(loc);
        }
        self.arena.alloc(node.build())
    }

    /// Scan input for list items at current indentation level.
    #[inline]
    pub fn list_struct(&self, limit: usize) -> &'b ListStruct<'a, 'b> {
        let bump = self.bump;
        let mut items = BumpVec::new_in(bump);
        let mut pos = self.cursor.pos();
        let input = self.input;

        let mut first_indent: Option<usize> = None;
        let mut in_block: bool = false;

        while pos < limit {
            let (line, next_pos) = match memchr(b'\n', &input.as_bytes()[pos..]) {
                Some(nl_pos) => (&input[pos..pos + nl_pos], pos + nl_pos + 1),
                None => (&input[pos..], limit),
            };

            let (indent, rest) = Self::get_indent(line, self.tab_width);

            let rest_bytes = rest.as_bytes();
            if !in_block
                && first_indent.is_some()
                && indent > first_indent.unwrap()
                && rest_bytes.len() >= 7
                && rest_bytes[0] == b'#'
                && rest_bytes[1] == b'+'
                && (rest_bytes[2..7]).eq_ignore_ascii_case(b"BEGIN")
            {
                in_block = true;
                pos = next_pos;
                continue;
            }

            if in_block {
                if rest_bytes.len() >= 5
                    && rest_bytes[0] == b'#'
                    && rest_bytes[1] == b'+'
                    && (rest_bytes[2..5]).eq_ignore_ascii_case(b"END")
                {
                    in_block = false;
                }
                pos = next_pos;
                continue;
            }

            if rest.is_empty() {
                let blank_rest = &input.as_bytes()[next_pos..];
                let second_nl = memchr(b'\n', blank_rest);
                if let Some(nl2) = second_nl {
                    let second_line = &blank_rest[..nl2];
                    if second_line.iter().all(|&b| b == b' ' || b == b'\t') {
                        break;
                    }
                }
                pos = next_pos;
                continue;
            }

            let starts_with_ordered = {
                let bs = rest.as_bytes();
                let mut j = 0;
                while j < bs.len() && bs[j].is_ascii_digit() {
                    j += 1;
                }
                j > 0
                    && j < bs.len()
                    && (bs[j] == b'.' || bs[j] == b')')
                    && (j + 1 >= bs.len() || bs[j + 1] == b' ' || bs[j + 1] == b'\t')
            };
            let starts_with_bullet = {
                let bs = rest.as_bytes();
                let first = bs.first().copied();
                (first == Some(b'-') || first == Some(b'+') || first == Some(b'*'))
                    && (bs.len() == 1 || bs[1] == b' ' || bs[1] == b'\t')
                    || starts_with_ordered
            };

            if starts_with_bullet {
                if let Some(fi) = first_indent {
                    match self.item_indent_ctx {
                        None => {
                            if indent < fi {
                                break;
                            }
                        }
                        Some(parent_indent) => {
                            if indent <= parent_indent {
                                break;
                            }
                        }
                    }
                }
                first_indent.get_or_insert(indent);
                let (bullet, counter, checkbox, tag) = Self::parse_item_bullet(rest);
                items.push(ListItem {
                    position: pos,
                    indent,
                    bullet,
                    counter,
                    checkbox,
                    tag,
                });
            } else {
                let fi = first_indent.unwrap_or(0);
                if indent > fi {
                    pos = next_pos;
                    continue;
                }
                break;
            }

            pos = next_pos;
        }

        bump.alloc(ListStruct { items, end: pos })
    }

    pub(super) fn get_indent(line: &str, tab_width: u8) -> (usize, &str) {
        let mut indent = 0;
        for (i, c) in line.char_indices() {
            match c {
                ' ' => indent += 1,
                '\t' => indent += tab_width as usize,
                _ => return (indent, &line[i..]),
            }
        }
        (indent, "")
    }

    fn parse_item_bullet(rest: &str) -> (&str, Option<usize>, Option<CheckBox>, Option<&str>) {
        let bytes = rest.as_bytes();
        let len = bytes.len();
        let mut offset = 0;
        let mut bullet: &str = &rest[..0];
        let mut counter = None;
        let mut checkbox = None;
        let mut tag = None;

        if offset < len {
            let c = bytes[offset];
            if c == b'-' || c == b'+' || c == b'*' {
                bullet = &rest[..1];
                offset += 1;
            } else if c.is_ascii_digit() {
                let num_start = offset;
                while offset < len && bytes[offset].is_ascii_digit() {
                    offset += 1;
                }
                if offset < len && (bytes[offset] == b'.' || bytes[offset] == b')') {
                    bullet = &rest[num_start..=offset];
                    offset += 1;
                } else {
                    offset = num_start;
                }
            }
        }

        while offset < len && (bytes[offset] == b' ' || bytes[offset] == b'\t') {
            offset += 1;
        }

        let remainder = rest[offset..].trim_start();
        let remaining = if remainder.starts_with("[@") {
            if let Some(end) = remainder.find(']') {
                let inner = &remainder[2..end];
                let num_str = inner.strip_prefix("start:").unwrap_or(inner);
                counter = num_str.parse().ok();
                remainder[end + 1..].trim_start()
            } else {
                remainder
            }
        } else {
            remainder
        };

        if remaining.starts_with('[') {
            if let Some(end) = remaining.find(']') {
                let content = &remaining[1..end];
                checkbox = match content {
                    "X" => Some(CheckBox::On),
                    " " | "" => Some(CheckBox::Off),
                    "-" => Some(CheckBox::Trans),
                    _ => None,
                };
                let after = remaining[end + 1..].trim_start();
                if let Some(tag_pos) = after.find("::") {
                    tag = Some(after[..tag_pos].trim());
                }
            }
        } else if let Some(tag_pos) = remaining.find("::") {
            tag = Some(remaining[..tag_pos].trim());
        }

        (bullet, counter, checkbox, tag)
    }
}
