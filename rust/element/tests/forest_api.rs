use std::collections::HashMap;
use std::ops::Range;

use org_element::prelude::*;

const INPUT: &str = "* Parent\n:PROPERTIES:\n:ID: abc-123\n:END:\n** Child\nThis is *bold* text.\n";

fn first_of(doc: &ParsedDoc<'_>, ty: SyntaxT) -> NodeId {
    doc.forest
        .nodes_of_type(ty)
        .next()
        .unwrap_or_else(|| panic!("expected at least one {} node", ty))
}

#[test]
fn node_at_offset_returns_enclosing_headline() {
    let doc = parse(INPUT);
    let at = doc.forest.node_at_offset(0).expect("node at offset 0");
    let Syntax::Headline(hd) = &doc.forest[at].data else {
        panic!(
            "expected Headline at offset 0, got {}",
            SyntaxT::from(&doc.forest[at].data)
        );
    };
    assert_eq!(hd.level, 1, "offset 0 lands in the level-1 Parent headline");
}

#[test]
fn node_at_offset_descends_to_narrowest_node() {
    let doc = parse(INPUT);
    let bold_off = INPUT.find("bold").expect("fixture contains 'bold'");
    let at = doc
        .forest
        .node_at_offset(bold_off)
        .expect("node at bold offset");
    assert!(
        doc.forest
            .nearest_ancestor_of_type(at, SyntaxT::Paragraph)
            .is_some(),
        "narrowest node at the bold text must live inside a Paragraph, got {}",
        SyntaxT::from(&doc.forest[at].data)
    );
}

#[test]
fn node_at_offset_out_of_range_is_none() {
    let doc = parse(INPUT);
    assert_eq!(doc.forest.node_at_offset(INPUT.len() + 1), None);
}

#[test]
fn children_of_type_filters_only_direct_children() {
    let doc = parse(INPUT);
    let parent = first_of(&doc, SyntaxT::Headline);

    let sections: Vec<NodeId> = doc
        .forest
        .children_of_type(parent, SyntaxT::Section)
        .collect();
    assert_eq!(
        sections.len(),
        1,
        "the Parent headline has one Section child"
    );

    assert_eq!(
        doc.forest
            .children_of_type(parent, SyntaxT::PropertyDrawer)
            .count(),
        0,
        "the drawer is a grandchild (under the Section), not a direct child"
    );
    assert_eq!(
        doc.forest
            .children_of_type(sections[0], SyntaxT::PropertyDrawer)
            .count(),
        1,
        "the drawer is a direct child of the Section"
    );
}

#[test]
fn ancestors_walk_from_leaf_to_root() {
    let doc = parse(INPUT);
    let prop = first_of(&doc, SyntaxT::NodeProperty);
    let chain: Vec<SyntaxT> = doc
        .forest
        .ancestors(prop)
        .map(|a| SyntaxT::from(&doc.forest[a].data))
        .collect();
    assert!(
        chain.contains(&SyntaxT::PropertyDrawer),
        "chain: {:?}",
        chain
    );
    assert!(chain.contains(&SyntaxT::Headline), "chain: {:?}", chain);
    assert_eq!(
        chain.last(),
        Some(&SyntaxT::OrgData),
        "the last ancestor is the root"
    );
}

#[test]
fn nearest_ancestor_of_type_finds_owning_headline() {
    let doc = parse(INPUT);
    let prop = first_of(&doc, SyntaxT::NodeProperty);
    let hl = doc
        .forest
        .nearest_ancestor_of_type(prop, SyntaxT::Headline)
        .expect("property is owned by a headline");
    let Syntax::Headline(hd) = &doc.forest[hl].data else {
        panic!("nearest Headline ancestor is not a headline");
    };
    assert_eq!(
        hd.level, 1,
        "the :ID: property belongs to the level-1 Parent"
    );
}

#[test]
fn post_order_visits_children_before_parents() {
    let doc = parse(INPUT);
    let order: Vec<NodeId> = doc.forest.post_order().collect();

    assert_eq!(
        order.last().copied(),
        Some(doc.forest.root()),
        "root is yielded last"
    );
    assert_eq!(
        order.len(),
        doc.forest.node_ids().count(),
        "post-order visits every node exactly once"
    );

    let pos: HashMap<NodeId, usize> = order.iter().enumerate().map(|(i, &id)| (id, i)).collect();
    for &id in &order {
        for child in doc.forest.children_of(id) {
            assert!(
                pos[&child] < pos[&id],
                "every child must precede its parent in post-order"
            );
        }
    }
}

#[test]
fn forest_into_iterator_matches_nodes() {
    let doc = parse(INPUT);
    let via_nodes = doc.forest.nodes().count();

    let mut via_for = 0;
    for _node in &doc.forest {
        via_for += 1;
    }
    assert_eq!(
        via_for, via_nodes,
        "`for node in &forest` visits every node"
    );
    assert_eq!((&doc.forest).into_iter().count(), via_nodes);
}

#[test]
fn chunk_arena_nodes_traverses_from_root() {
    let bump = bumpalo::Bump::new();
    let mut parser = Parser::new(INPUT, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let first = arena.nodes(root).next().expect("root yields itself first");
    assert!(
        matches!(first.data, Syntax::OrgData),
        "pre-order starts at the root"
    );
    assert!(
        arena
            .nodes(root)
            .any(|n| matches!(n.data, Syntax::Headline(_))),
        "traversal reaches the headlines"
    );
}

#[test]
fn syntax_type_display_names_variants() {
    assert_eq!(SyntaxT::Headline.to_string(), "Headline");
    assert_eq!(SyntaxT::PropertyDrawer.to_string(), "PropertyDrawer");
    assert_eq!(SyntaxT::PlainText.to_string(), "PlainText");
}

#[test]
fn interval_range_round_trips() {
    let iv = Interval { start: 3, end: 7 };
    let r: Range<usize> = iv.into();
    assert_eq!(r, 3..7);
    let back: Interval = (3..7).into();
    assert_eq!(back, iv);
}
