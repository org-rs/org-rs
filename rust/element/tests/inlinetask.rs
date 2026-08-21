use org_element::testutils::*;

#[test]
fn fifteen_stars_is_not_headline() {
    let input = "*************** Inlinetask\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();
    let root_children = &arena[root].children;
    let first = root_children.first().expect("Expected first child");
    assert!(
        !matches!(arena[*first].data, Syntax::Headline(_)),
        "15+ stars should NOT be a Headline, got: {:?}",
        arena[*first].data
    );
}

/// An inline task closed by a `*************** END` marker spans through
/// that marker as a SINGLE node, and the body between the markers is
/// nested inside it. Emacs (with org-inlinetask) parses one inlinetask
/// containing the src block; the END line is not its own node. Rust used
/// to emit two inline tasks (one for END) and leave the body as a sibling.
#[test]
fn inline_task_with_end_marker_nests_body() {
    let input = "* Top\n\
                 *************** TODO task\n\
                 #+begin_src elisp\n\
                 (foo)\n\
                 #+end_src\n\
                 *************** END\n\
                 after\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let it_count = count_type(&arena, root, SyntaxT::InlineTask);
    assert_eq!(
        it_count, 1,
        "expected exactly 1 InlineTask (END is not a separate task), found {}",
        it_count
    );

    fn src_in_inlinetask(arena: &NodeArena, id: NodeId, inside: bool) -> bool {
        if inside && matches!(arena[id].data, Syntax::SrcBlock(_)) {
            return true;
        }
        let now = inside || matches!(arena[id].data, Syntax::InlineTask(_));
        arena[id]
            .children
            .iter()
            .any(|&c| src_in_inlinetask(arena, c, now))
    }
    assert!(
        src_in_inlinetask(&arena, root, false),
        "expected the src block nested inside the inline task"
    );
}
