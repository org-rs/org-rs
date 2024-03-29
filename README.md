[Org Mode](https://orgmode.org/) parser re-write in Rust


# Motivation

[Org](https://orgmode.org/) is probably the best and most complete plain text
organizational system known to mankind.  It has countless applications:
- authoring,
- publishing,
- technical documentation,
- literate programming,
- task and time tracking,
- journalling,
- blogging,
- agendas,
- wikis,
and many more.

Org has a limited presence outside of GNU Emacs, and the standard
implementation is that of GNU Emacs written in Emacs lisp.  Despite
the fact that GNU Emacs is widely available and widely used as a lisp
interpreter, there are applications wherein using Emacs to properly
handle and parse an Org file would be problematic.  This package is
intended to replicate the exact behaviour of Org as implemented in
Emacs lisp, while decoupling the use of Org from Emacs, such that it
can be used in a content management system, as a blogging backend, _etc._

## Prior art

Many attempts have been made.  The most faithful not written in a lisp is the implementaiton in [`pandoc`](https://github.com/jgm/pandoc).

The reason why there have been relatively few attemps is that Org's
syntax is not trivial.  Most of Org's syntax is
[context-sensitive](https://en.wikipedia.org/wiki/Chomsky_hierarchy#Type-1_grammars)
with only a few context-free elements.  This results in a higher
complexity and problematic testing of implementations, as unit testing
of small chunks of Org code does not guarantee correct parsing of the
Org file as a whole.

To this end, Org parsers
[org-ruby](https://github.com/wallyqs/org-ruby) and
[pandoc](https://pandoc.org/), have chosen to focus on a restricted
subset of Org's syntax.  More ambitious projects try to cover all
features but since Org does not have a formal
specification[^1] they rely on observed Org's behavior in
Emacs or author's intuition.  As a result they rarely get finished.

This project aims to be a faithful one-to-one recreation of the Emacs
lisp code translated into Rust.  As such it eschews many of the
problems with writing an ad-hoc parser, and allows for easy adaptation
in case the official Org specification (that of the reference parser),
changes.

Check out our [FAQ](https://github.com/org-rs/org-rs/wiki/FAQ) for
more information.  Also feel free to open an issue and/or discussion.

# Goals

`org-rs` is guided by the following principles:
- Be a faithful recreation of the original implementation, not a
  competing standard or implementation.
- Be standalone, embeddable and reusable.
- Adapt to the users' needs, rather than impose adherence to specific ecosystems.
- Be fast.


# Design decisions

These are the choices that were made to achieve the goals:

- Use Rust.  It's fast, memory safe, and has a healthy package
  ecosystem.  It also can be linked both statically and dynamically
  against C code.
- Adhere to the original emacs lisp implementation in terms of
  structure and organisation.
- Adhere to idiomatic Rust wherever else possible.

These decisions result in a clear scope, and completion criteria, as
well as easily verifiable replication of the behaviour of the original
lisp implementation.


# Roadmap

[element](rust/element) - parser crate is currently the main and only focus.
It should perform just 2 tasks. Generate concrete syntax tree and serialize it
back to canonical Org representation.

The rest of the roadmap is not fully flashed out. Feature-complete parser opens
a lot of possibilities, here are just a few of my ideas:

- Parse tree manipulation tools (like exporting to other formats).
- Language server - [a way to solve "the matrix" problem](https://langserver.org/).
  Enabling other editors to have their own org-mode would be a logical next step.

- CLI tools. I'd love to get integration with
  [TaskWarrior](https://github.com/GothenburgBitFactory/taskwarrior)
  and maybe even use Org as TaskWarrior's DOM.


# Contribution

Any contributions are welcome:
- Code
- Documentation
- Verification
- Spreading the word
- Using in your project in a cool way

If you want to contribute code, please check out the [contribution
guide](doc/CONTRIBUTING.org).

Got a question?  Open a discussion.

# Similar projects

- [vim-orgmode](https://github.com/jceb/vim-orgmode)
- [orgajs](https://github.com/xiaoxinghu/orgajs) nodejs
- [orgnode](http://members.optusnet.com.au/~charles57/GTD/orgnode.html) python
- [org-ruby](https://github.com/wallyqs/org-ruby) ruby
- and [many others](https://orgmode.org/worg/org-tools/index.html)


# More about Org Mode


- [Org-Mode Is One of the Most Reasonable Markup Languages to Use for Text](https://karl-voit.at/2017/09/23/orgmode-as-markup-only/)
- [Awesome guide](http://doc.norang.ca/org-mode.html) about org-mode
- [teaser](https://github.com/novoid/org-mode-workshop/blob/master/featureshow/org-mode-teaser.org)


[^1]: Some attempts were made to formalize the syntax. This project uses them as supplementary materials.
See [contribution guide](doc/CONTRIBUTING.org) for details.
