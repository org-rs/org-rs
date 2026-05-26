fn main() {
    use org_element::prelude::*;
    eprintln!("SyntaxNode  = {} bytes", std::mem::size_of::<SyntaxNode>());
    eprintln!("Syntax      = {} bytes", std::mem::size_of::<Syntax>());
}
