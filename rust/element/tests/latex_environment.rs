use org_element::testutils::*;

#[test]
fn latex_environment() {
    for (input, desc) in &[
        (
            "\\begin{equation}\nE = mc^2\n\\end{equation}\n",
            "basic equation",
        ),
        (
            "\\begin{equation}\n\\frac{a}{b}\n\\end{equation}\n",
            "equation with frac",
        ),
        (
            "\\begin{aligned}\na &= b \\\\\nc &= d\n\\end{aligned}\n",
            "aligned",
        ),
    ] {
        let count = get_type_count(input, SyntaxT::LatexEnvironment, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 latex environment ({}), found {}",
            desc, count
        );
    }
}
