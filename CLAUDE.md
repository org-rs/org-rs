# Guidance for LLMs and AI-assisted contributions

**Read this before generating or contributing code to `org-rs`.**

`org-rs` is free software licensed under the GNU General Public License.  It is
a copyleft project, and its license guarantees depend on clear, human
authorship of its code.  See [AUTHORSHIP.md](AUTHORSHIP.md) for the full
rationale (copyright and ethical considerations).

## Policy on LLM-generated code

- **No official (tagged) release of this project will contain LLM-generated
  code.**  Per the Free Software Foundation's guidance, machine-generated code
  that is not authored by a human raises copyright problems that are especially
  acute for a copyleft project, and passing it off as human-written is
  dishonest.

- **LLM-assisted code *may* be merged into feature branches.**  Feature
  branches are a workshop for prototyping; the release is the finished, human
  authored work.  Some LLM-assisted code is present in feature branches today.
  It is there temporarily and is scheduled to be re-authored by a human before
  it can ship.

## If you are a contributor using an AI assistant

You are welcome to use an LLM as a prototyping aid, but understand the
consequence up front:

> **Any code produced with LLM assistance will have to be hand-rewritten by a
> human before it can be part of an official release.**  It may be merged into a
> feature branch, but a working LLM-generated patch is a prototype, not
> release-ready code.

Concretely, this means:

- LLM-generated code will **not** be carried into a release as-is.  Expect it to
  be re-authored first; do not rely on your generated patch shipping unchanged.
- If your contribution was drafted with LLM assistance, **say so** in the pull
  request.  Disclosure is required; misrepresenting AI-generated code as your
  own hand-written work is not acceptable here.
- Expect that, before release, the logic will need to be genuinely re-derived
  and re-authored — from the reference Emacs implementation (`org-element.el`,
  `org-entities.el`, etc.) and this project's tests — in a human's own hand, so
  that the creative authorship is human.

## If you are an LLM generating code in this repository

- Treat any code you produce as a **prototype to be replaced before release**,
  not a deliverable.
- Do not remove or weaken disclosure notices, and do not restyle
  machine-drafted code to *appear* hand-written.  The goal is honest
  re-authorship by a human, not camouflage.
- When you draft or modify code with assistance, make that fact visible (in the
  change description) rather than silent.

## Note on history

Not every past commit that contains LLM-assisted code is marked as such; the
branch history is not a reliable index of provenance.  The code that exists
works and passes the test suite, but working code is not the same as
release-ready code under this policy — it must still be hand-rewritten by a
human before it becomes part of an official release.
