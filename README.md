[Org Mode](https://orgmode.org/) parser re-write in Rust


# Motivation

[Org](https://orgmode.org/) is probably the best and most complete plain text
organizational system known to mankind. It has countless applications like
authoring, publishing, task and time tracking, journal, blog, agenda, wiki
etc...

Unfortunately Org was originally developed for Emacs and therefore
available only inside Emacs. It is a huge limiting factor for Org's development
and popularization. Because of that it is not as popular outside of Emacs
community as it should be.

Many attempts were made to fix this. It all starts with a parser.
But because Org's syntax is not trivial and in fact most of it is
[context-sensitive](https://en.wikipedia.org/wiki/Chomsky_hierarchy#Type-1_grammars)
with only a few context-free elements, it is quite easy to get it wrong. 
Some Org parsers have chosen to focus on a restricted subset of Org's syntax like
[org-ruby](https://github.com/wallyqs/org-ruby) or [pandoc](https://pandoc.org/).
More ambitious projects try to cover all features but since Org does not have a
formal specification<sup>1</sup> they rely on observed Org's behavior in Emacs
or author's intuition.  As a result they rarely get finished.

But the absence of a good a spec and the complexity of the grammar are not show
stoppers. Why reinventing the wheel when we can just copy it!  This project
takes the only surefire way to get it right - use the original elisp parser
implementation as a blueprint!


Check out [FAQ](https://github.com/org-rs/org-rs/wiki/FAQ) for more information 
and feel free to open an issue if you have more questions!

# Goals

- Be fast. Speed matters. Period.
- Be feature-complete, compliant with original implementation. Nobody wants Nth competing standard.
- Be standalone, embeddable and reusable. User must not be locked into 
one particular editor or ecosystem. Integrations with language servers,
 editors and plugins should be encouraged.


# Design decisions

These are the choices that were made to achieve the goals:

- Rust. Because it is fast, memory safe and provides C FFI. And most importantly it is cool.

- Original elisp algorithm. While using the original elisp source as a guideline
  might result in less idiomatic Rust code it has its advantages:

  - Scope of work is well-defined and finish line is visible. This should encourage
    contributions even from people who want to get started with Rust.

  - Getting "feature-complete" is just a matter of getting to the finish line.


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

Any contributions are welcome. If you want to help check out
[contribution guide](doc/CONTRIBUTING.org).

Got a question? Stop by for a chat at [gitter](https://gitter.im/org-rs/community)

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



## Footnote

1. Some attempts were made to formalize the syntax. This project uses them as supplementary materials.
See [contribution guide](doc/CONTRIBUTING.org) for details.

