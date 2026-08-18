use super::{BumpVec, NodeId, Syntax, SyntaxNode, SyntaxT};

/// Deprecated alias for [`ChunkArena`].
pub type NodeArena<'a, 'b> = ChunkArena<'a, 'b>;

/// Single-lifetime alias for a post-parse arena where `'a == 'b`.
pub type ParseArena<'a> = ChunkArena<'a, 'a>;

/// Single-lifetime alias for a post-parse forest where `'a == 'b`.
pub type ParseForest<'a> = Forest<'a, 'a>;

/// A self-contained arena for one chunk of the parse forest.
#[derive(Debug)]
pub struct ChunkArena<'a, 'b> {
    pub(crate) chunk_id: u32,
    pub(crate) _base: usize,
    pub(crate) nodes: Vec<SyntaxNode<'a, 'b>>,
    pub(crate) _bumps: Vec<Box<bumpalo::Bump>>,
}

impl<'a, 'b> ChunkArena<'a, 'b> {
    #[inline]
    pub fn new() -> Self {
        ChunkArena {
            chunk_id: 0,
            _base: 0,
            nodes: Vec::new(),
            _bumps: Vec::new(),
        }
    }

    #[inline]
    pub fn with_id(chunk_id: u32, _base: usize) -> Self {
        ChunkArena {
            chunk_id,
            _base,
            nodes: Vec::new(),
            _bumps: Vec::new(),
        }
    }

    #[inline]
    pub fn alloc(&mut self, mut node: SyntaxNode<'a, 'b>) -> NodeId {
        node.parent = None;
        let local = self.nodes.len() as u32;
        self.nodes.push(node);
        NodeId::new(self.chunk_id, local)
    }

    #[inline]
    pub fn alloc_with_children(
        &mut self,
        mut node: SyntaxNode<'a, 'b>,
        children: BumpVec<'b, NodeId>,
    ) -> NodeId {
        let local = self.nodes.len() as u32;
        let id = NodeId::new(self.chunk_id, local);
        for &child in &children {
            self.nodes[child.local() as usize].parent = Some(id);
        }
        node.parent = None;
        node.children = children;
        self.nodes.push(node);
        id
    }

    #[inline]
    pub fn set_children(&mut self, parent: NodeId, children: BumpVec<'b, NodeId>) {
        debug_assert_eq!(parent.chunk(), self.chunk_id);
        for &child in &children {
            self.nodes[child.local() as usize].parent = Some(parent);
        }
        self.nodes[parent.local() as usize].children = children;
    }

    #[inline]
    pub fn set_title_objects(&mut self, parent: NodeId, objects: BumpVec<'b, NodeId>) {
        debug_assert_eq!(parent.chunk(), self.chunk_id);
        for &obj in &objects {
            self.nodes[obj.local() as usize].parent = Some(parent);
        }
        if let Syntax::Headline(ref mut data) = self.nodes[parent.local() as usize].data {
            data.title_objects = objects;
        }
    }

    #[inline]
    pub fn get(&self, id: NodeId) -> &SyntaxNode<'a, 'b> {
        &self.nodes[id.local() as usize]
    }

    pub fn nodes(&self, root: NodeId) -> ChunkNodes<'_, 'a, 'b> {
        ChunkNodes {
            arena: self,
            stack: vec![root],
        }
    }
}

impl<'a, 'b> Default for ChunkArena<'a, 'b> {
    #[inline]
    fn default() -> Self {
        ChunkArena::new()
    }
}

impl<'a, 'b> std::ops::Index<NodeId> for ChunkArena<'a, 'b> {
    type Output = SyntaxNode<'a, 'b>;
    #[inline]
    fn index(&self, id: NodeId) -> &Self::Output {
        &self.nodes[id.local() as usize]
    }
}

impl<'a, 'b> std::ops::IndexMut<NodeId> for ChunkArena<'a, 'b> {
    #[inline]
    fn index_mut(&mut self, id: NodeId) -> &mut Self::Output {
        &mut self.nodes[id.local() as usize]
    }
}

/// A forest of chunk arenas, representing a complete parsed document.
#[derive(Debug)]
pub struct Forest<'a, 'b> {
    pub(crate) chunks: Vec<ChunkArena<'a, 'b>>,
    pub(crate) root: NodeId,
}

impl<'a, 'b> Forest<'a, 'b> {
    #[inline]
    pub fn from_chunks(chunks: Vec<ChunkArena<'a, 'b>>, root: NodeId) -> Self {
        Forest { chunks, root }
    }

    #[inline]
    pub fn root(&self) -> NodeId {
        self.root
    }

    #[inline]
    pub fn chunks(&self) -> &[ChunkArena<'a, 'b>] {
        &self.chunks
    }

    #[inline]
    pub fn chunk(&self, id: NodeId) -> &ChunkArena<'a, 'b> {
        &self.chunks[id.chunk() as usize]
    }

    #[inline]
    pub fn chunk_mut(&mut self, id: NodeId) -> &mut ChunkArena<'a, 'b> {
        &mut self.chunks[id.chunk() as usize]
    }

    #[inline]
    pub fn iter(&self, root: NodeId) -> Nodes<'_, 'a, 'b> {
        Nodes {
            forest: self,
            stack: InlineStack::with_first_entry(0),
            current: root,
        }
    }

    #[inline]
    pub fn nodes(&self) -> Nodes<'_, 'a, 'b> {
        self.iter(self.root)
    }

    /// Return the source text slice that corresponds to this node's location.
    #[inline]
    pub fn text_of(&self, id: NodeId, input: &'a str) -> &'a str {
        let loc = &self[id].location;
        &input[loc.start..loc.end]
    }

    /// Iterator over the direct children of `id`.
    #[inline]
    pub fn children_of(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self[id].children.iter().copied()
    }

    /// Return the parent of `id`, or `None` for the root node.
    #[inline]
    pub fn parent_of(&self, id: NodeId) -> Option<NodeId> {
        self[id].parent
    }

    /// Pre-order iterator over all [`NodeId`]s in the forest.
    #[inline]
    pub fn node_ids(&self) -> NodeIds<'_, 'a, 'b> {
        NodeIds {
            forest: self,
            stack: vec![self.root],
        }
    }

    /// Pre-order iterator over all nodes of the given type.
    #[inline]
    pub fn nodes_of_type(&self, ty: SyntaxT) -> NodesByType<'_, 'a, 'b> {
        NodesByType {
            forest: self,
            stack: vec![self.root],
            ty,
        }
    }

    /// Pre-order iterator over `(NodeId, &SyntaxNode)` pairs from the root.
    #[inline]
    pub fn entries(&self) -> NodeEntries<'_, 'a, 'b> {
        NodeEntries {
            forest: self,
            stack: vec![self.root],
        }
    }

    /// Pre-order iterator over `(NodeId, &SyntaxNode)` pairs from `root`.
    #[inline]
    pub fn entries_from(&self, root: NodeId) -> NodeEntries<'_, 'a, 'b> {
        NodeEntries {
            forest: self,
            stack: vec![root],
        }
    }

    pub fn post_order(&self) -> PostOrderIds<'_, 'a, 'b> {
        PostOrderIds {
            forest: self,
            stack: vec![(self.root, false)],
        }
    }

    pub fn post_order_from(&self, root: NodeId) -> PostOrderIds<'_, 'a, 'b> {
        PostOrderIds {
            forest: self,
            stack: vec![(root, false)],
        }
    }

    /// Return the narrowest node whose half-open span `[start, end)` contains
    /// `offset`, or `None` if `offset` falls outside all top-level content.
    ///
    /// The root is a zero-width sentinel that does not span the buffer, so the
    /// descent is driven purely by child containment rather than the current
    /// node's own location.
    pub fn node_at_offset(&self, offset: usize) -> Option<NodeId> {
        let mut current = self.root;
        loop {
            let next = self[current].children.iter().copied().find(|&c| {
                let cl = self[c].location;
                cl.start <= offset && offset < cl.end
            });
            match next {
                Some(c) => current = c,
                None => break,
            }
        }
        (current != self.root).then_some(current)
    }

    pub fn children_of_type<'s>(
        &'s self,
        id: NodeId,
        ty: SyntaxT,
    ) -> impl Iterator<Item = NodeId> + use<'s, 'a, 'b> {
        self.children_of(id)
            .filter(move |&c| SyntaxT::from(&self[c].data) == ty)
    }

    pub fn ancestors<'s>(&'s self, id: NodeId) -> impl Iterator<Item = NodeId> + use<'s, 'a, 'b> {
        std::iter::successors(self[id].parent, move |&p| self[p].parent)
    }

    pub fn nearest_ancestor_of_type(&self, id: NodeId, ty: SyntaxT) -> Option<NodeId> {
        self.ancestors(id)
            .find(|&a| SyntaxT::from(&self[a].data) == ty)
    }
}

impl<'a, 'b> std::ops::Index<NodeId> for Forest<'a, 'b> {
    type Output = SyntaxNode<'a, 'b>;
    #[inline]
    fn index(&self, id: NodeId) -> &Self::Output {
        &self.chunks[id.chunk() as usize].nodes[id.local() as usize]
    }
}

impl<'a, 'b> std::ops::IndexMut<NodeId> for Forest<'a, 'b> {
    #[inline]
    fn index_mut(&mut self, id: NodeId) -> &mut Self::Output {
        &mut self.chunks[id.chunk() as usize].nodes[id.local() as usize]
    }
}

impl<'s, 'a: 's, 'b: 's> IntoIterator for &'s Forest<'a, 'b> {
    type Item = &'s SyntaxNode<'a, 'b>;
    type IntoIter = Nodes<'s, 'a, 'b>;
    fn into_iter(self) -> Self::IntoIter {
        self.nodes()
    }
}

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

/// A pre-order traversal of [`NodeId`]s inside a [`Forest`], filtered by [`SyntaxT`].
pub struct NodesByType<'s, 'a, 'b> {
    forest: &'s Forest<'a, 'b>,
    stack: Vec<NodeId>,
    ty: SyntaxT,
}

impl<'s, 'a, 'b> Iterator for NodesByType<'s, 'a, 'b> {
    type Item = NodeId;

    #[inline]
    fn next(&mut self) -> Option<NodeId> {
        loop {
            let id = self.stack.pop()?;
            let node = &self.forest[id];
            for &child in node.children.iter().rev() {
                self.stack.push(child);
            }
            if SyntaxT::from(&node.data) == self.ty {
                return Some(id);
            }
        }
    }
}

/// A pre-order traversal of [`NodeId`] values inside a [`Forest`].
pub struct NodeIds<'s, 'a, 'b> {
    forest: &'s Forest<'a, 'b>,
    stack: Vec<NodeId>,
}

impl<'s, 'a, 'b> Iterator for NodeIds<'s, 'a, 'b> {
    type Item = NodeId;

    #[inline]
    fn next(&mut self) -> Option<NodeId> {
        let id = self.stack.pop()?;
        let children = &self.forest[id].children;
        for &child in children.iter().rev() {
            self.stack.push(child);
        }
        Some(id)
    }
}

/// A pre-order traversal of `(NodeId, &SyntaxNode)` pairs inside a [`Forest`].
pub struct NodeEntries<'s, 'a, 'b> {
    forest: &'s Forest<'a, 'b>,
    stack: Vec<NodeId>,
}

impl<'s, 'a, 'b> Iterator for NodeEntries<'s, 'a, 'b> {
    type Item = (NodeId, &'s SyntaxNode<'a, 'b>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let id = self.stack.pop()?;
        let node = &self.forest[id];
        for &child in node.children.iter().rev() {
            self.stack.push(child);
        }
        Some((id, node))
    }
}

/// A pre-order traversal of [`SyntaxNode`] references inside a [`ChunkArena`].
pub struct ChunkNodes<'s, 'a, 'b> {
    arena: &'s ChunkArena<'a, 'b>,
    stack: Vec<NodeId>,
}

impl<'s, 'a, 'b> Iterator for ChunkNodes<'s, 'a, 'b> {
    type Item = &'s SyntaxNode<'a, 'b>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let id = self.stack.pop()?;
        let node = &self.arena[id];
        for &c in node.children.iter().rev() {
            self.stack.push(c);
        }
        Some(node)
    }
}

/// A post-order traversal of [`NodeId`] values inside a [`Forest`].
pub struct PostOrderIds<'s, 'a, 'b> {
    forest: &'s Forest<'a, 'b>,
    stack: Vec<(NodeId, bool)>,
}

impl<'s, 'a, 'b> Iterator for PostOrderIds<'s, 'a, 'b> {
    type Item = NodeId;

    #[inline]
    fn next(&mut self) -> Option<NodeId> {
        loop {
            let (id, visited) = self.stack.last_mut()?;
            if *visited {
                let id = *id;
                self.stack.pop();
                return Some(id);
            }
            *visited = true;
            for &c in self.forest[*id].children.iter().rev() {
                self.stack.push((c, false));
            }
        }
    }
}

/// A pre-order traversal of [`SyntaxNode`] values inside a [`Forest`].
pub struct Nodes<'s, 'a, 'b> {
    forest: &'s Forest<'a, 'b>,
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
            let n = self.forest[self.current].children.len();
            let exhausted = self.stack.last().is_none_or(|idx| idx >= n);
            if !exhausted {
                break;
            }
            self.stack.pop();
            if self.stack.is_empty() {
                return Some(&self.forest[saved]);
            }
            self.current = self.forest[self.current].parent?;
        }

        let child_idx = self.stack.last().unwrap();
        let child = self.forest[self.current].children[child_idx];
        *self.stack.last_mut().unwrap() += 1;
        self.stack.push(0);
        self.current = child;

        Some(&self.forest[saved])
    }
}
