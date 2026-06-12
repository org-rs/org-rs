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
}

/// Link types recognised by a default `-Q` Emacs session (`org-link-types`).
/// `file` is listed after its `file+…` variants only for readability; the
/// trailing-`:` check disambiguates them regardless of order.
pub const ORG_DEFAULT_LINK_TYPES: &[&str] = &[
    "https", "http", "ftp", "mailto", "news", "shell", "elisp", "file+emacs", "file+sys", "file",
    "help", "id",
];

/// First letters of [`ORG_DEFAULT_LINK_TYPES`].
pub const ORG_DEFAULT_LINK_START_BYTES: &[u8] = b"efhimns";

pub struct DefaultEnvironment;

impl Environment for DefaultEnvironment {}
