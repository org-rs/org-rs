/// The environment holds the configuration options that Org uses while
/// parsing.
pub trait Environment {
    /// Registered Org link-type names (without the trailing `:`), used to
    /// recognise plain links such as `file:…` or `help:…`. Mirrors Emacs's
    /// `org-link-types`. Override to add or remove recognised types.
    #[inline]
    fn link_types(&self) -> &[&str] {
        ORG_DEFAULT_LINK_TYPES
    }

    /// First bytes of [`link_types`](Self::link_types). The plain-text scanner
    /// only pauses to test for a plain link at these bytes, so a type whose
    /// first letter is missing here is recognised inside `[[…]]` but not bare
    /// in running text. Keep this consistent with `link_types`.
    #[inline]
    fn link_start_bytes(&self) -> &[u8] {
        ORG_DEFAULT_LINK_START_BYTES
    }

    /// Tab width used for `string-width`-style indent calculations.
    /// Emacs always uses 8 for Org files; override for non-standard setups.
    #[inline]
    fn tab_width(&self) -> u8 {
        8
    }

    /// When `true`, the parser replicates Emacs `org-element` behaviour
    /// exactly — including quirks org-rs would otherwise handle more
    /// sensibly (e.g. treating `_src` in a stray `#+end_src` line as a
    /// subscript). When `false` (the default), org-rs is free to diverge
    /// from Emacs where a saner result is available.
    ///
    /// The corpus oracle comparison (`examples/corpus-check`) parses with
    /// this enabled via [`ComplianceEnvironment`], so the corpus measures
    /// strict Emacs compliance; library users get the saner default.
    #[inline]
    fn emacs_compliance(&self) -> bool {
        false
    }
}

/// Link types recognised by a default `-Q` Emacs session (`org-link-types`).
/// `file` is listed after its `file+…` variants only for readability; the
/// trailing-`:` check disambiguates them regardless of order.
pub const ORG_DEFAULT_LINK_TYPES: &[&str] = &[
    "https",
    "http",
    "ftp",
    "mailto",
    "news",
    "shell",
    "elisp",
    "file+emacs",
    "file+sys",
    "file",
    "help",
    "id",
];

/// First letters of [`ORG_DEFAULT_LINK_TYPES`].
pub const ORG_DEFAULT_LINK_START_BYTES: &[u8] = b"efhimns";

#[derive(Clone)]
pub struct DefaultEnvironment;

impl Environment for DefaultEnvironment {}

/// Like [`DefaultEnvironment`], but with strict Emacs compliance enabled
/// ([`Environment::emacs_compliance`] returns `true`). Used by the corpus
/// oracle comparison so org-rs is measured against Emacs `org-element`
/// behaviour exactly, quirks included.
#[derive(Clone)]
pub struct ComplianceEnvironment;

impl Environment for ComplianceEnvironment {
    #[inline]
    fn emacs_compliance(&self) -> bool {
        true
    }
}
