use super::SyntaxNode;

/// Some elements can contain objects directly in their value fields.
pub enum StringOrObject<'a, 'b> {
    Raw(&'a str),
    Parsed(&'b mut SyntaxNode<'a, 'b>),
}

impl<'a, 'b> core::fmt::Debug for StringOrObject<'a, 'b> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            StringOrObject::Raw(raw) => write!(f, "Raw: {:?}", raw),
            StringOrObject::Parsed(p) => write!(f, "Parsed({:?})", p),
        }
    }
}

impl<'a, 'b> PartialEq for StringOrObject<'a, 'b> {
    #[inline]
    fn eq(&self, other: &StringOrObject<'a, 'b>) -> bool {
        match self {
            StringOrObject::Raw(raw) => match other {
                StringOrObject::Parsed(..) => false,
                StringOrObject::Raw(rhs) => raw.eq(rhs),
            },
            StringOrObject::Parsed(a) => match other {
                StringOrObject::Parsed(b) => {
                    std::ptr::eq(*a as *const SyntaxNode, *b as *const SyntaxNode)
                }
                StringOrObject::Raw(..) => false,
            },
        }
    }
}

#[derive(Debug)]
pub struct ClockData<'a> {
    pub raw: &'a str,
}

impl<'a> ClockData<'a> {
    #[inline]
    pub fn new(raw: &'a str) -> Self {
        Self { raw }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum LineNumberingMode {
    New,
    Continued,
}

#[derive(Debug)]
pub struct PlanningData<'a> {
    /// Timestamp associated to deadline keyword, if any (timestamp object or nil).
    pub deadline: Option<TimestampData<'a>>,

    /// Timestamp associated to scheduled keyword, if any (timestamp object or nil).
    pub scheduled: Option<TimestampData<'a>>,

    /// Timestamp associated to closed keyword, if any (timestamp object or nil).
    pub closed: Option<TimestampData<'a>>,
}

#[derive(Debug)]
pub struct FootnoteReferenceData<'a> {
    /// Footnote's label, if any (string or nil).
    pub label: Option<&'a str>,

    /// Determine whether reference has its definition inline, or not.
    pub type_s: &'a str,
}

#[derive(Debug)]
pub struct InlineBabelCallData<'a> {
    /// Name of code block being called (string).
    pub call: &'a str,

    /// Arguments passed to the code block (string or nil).
    pub arguments: Option<&'a str>,

    /// Raw call, as Org syntax (string).
    pub value: &'a str,
}

#[derive(Debug)]
pub struct InlineSrcBlockData<'a> {
    /// Language of the code in the block (string).
    pub language: &'a str,

    /// Optional header arguments (string or nil).
    pub parameters: Option<&'a str>,

    /// Source code (string).
    pub value: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LinkFormat {
    Plain,
    Angle,
    Bracket,
}

/// Packed bitflags for [`LinkData`], combining [`LinkFormat`] and [`LinkType`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkFlags(u8);

impl LinkFlags {
    const TYPE_SHIFT: u8 = 2;

    #[inline]
    pub fn new(format: LinkFormat, link_type: LinkType) -> Self {
        let f = match format {
            LinkFormat::Plain => 0,
            LinkFormat::Angle => 1,
            LinkFormat::Bracket => 2,
        };
        let t = match link_type {
            LinkType::Coderef => 0,
            LinkType::CustomId => 1,
            LinkType::File => 2,
            LinkType::Fuzzy => 3,
            LinkType::Id => 4,
            LinkType::Radio => 5,
        };
        LinkFlags(f | (t << Self::TYPE_SHIFT))
    }

    #[inline]
    pub fn link_type(self) -> LinkType {
        match self.0 >> Self::TYPE_SHIFT {
            0 => LinkType::Coderef,
            1 => LinkType::CustomId,
            2 => LinkType::File,
            3 => LinkType::Fuzzy,
            4 => LinkType::Id,
            _ => LinkType::Radio,
        }
    }
}

#[derive(Debug)]
pub struct LinkData<'a> {
    pub flags: LinkFlags,
    pub path: &'a str,
    pub raw_link: &'a str,
}

impl<'a> LinkData<'a> {
    #[inline]
    pub fn new_plain(raw: &'a str) -> Self {
        let link_type = if raw.starts_with("https://")
            || raw.starts_with("http://")
            || raw.starts_with("ftp://")
        {
            LinkType::File
        } else if raw.starts_with("id:") {
            LinkType::Id
        } else {
            LinkType::Fuzzy
        };
        LinkData {
            flags: LinkFlags::new(LinkFormat::Plain, link_type),
            path: raw,
            raw_link: raw,
        }
    }

    #[inline]
    pub fn new(raw: &'a str) -> Self {
        let inner = raw
            .strip_prefix("[[")
            .and_then(|s| s.strip_suffix("]]"))
            .unwrap_or(raw);
        let (link_type, path) = if inner.starts_with("http://") || inner.starts_with("https://") {
            (LinkType::File, inner)
        } else if inner.starts_with("file:") {
            (LinkType::File, inner.strip_prefix("file:").unwrap_or(inner))
        } else if inner.starts_with("id:") {
            (LinkType::Id, inner.strip_prefix("id:").unwrap_or(inner))
        } else if inner.starts_with('#') {
            (LinkType::CustomId, inner.strip_prefix('#').unwrap_or(inner))
        } else {
            (LinkType::Fuzzy, raw)
        };
        LinkData {
            flags: LinkFlags::new(LinkFormat::Bracket, link_type),
            path,
            raw_link: raw,
        }
    }

    #[inline]
    pub fn new_angle(raw: &'a str) -> Self {
        let inner = raw
            .strip_prefix("<")
            .and_then(|s| s.strip_suffix(">"))
            .unwrap_or(raw);
        let (link_type, path) = if inner.starts_with("http://") || inner.starts_with("https://") {
            (LinkType::File, inner)
        } else if inner.starts_with("file:") {
            (LinkType::File, inner.strip_prefix("file:").unwrap_or(inner))
        } else if inner.starts_with("id:") {
            (LinkType::Id, inner.strip_prefix("id:").unwrap_or(inner))
        } else {
            (LinkType::Fuzzy, raw)
        };
        LinkData {
            flags: LinkFlags::new(LinkFormat::Angle, link_type),
            path,
            raw_link: raw,
        }
    }

    #[inline]
    pub fn link_type(&self) -> LinkType {
        self.flags.link_type()
    }
}

#[derive(Debug, PartialEq)]
#[repr(u8)]
pub enum LinkType {
    Coderef,
    CustomId,
    File,
    Fuzzy,
    Id,
    Radio,
}

#[derive(Debug)]
pub struct MacroData<'a> {
    pub args: Vec<&'a str>,
    pub key: &'a str,
    pub value: &'a str,
}

#[derive(Debug)]
pub struct RadioTargetData<'a> {
    pub raw_value: &'a str,
}

#[derive(Debug)]
pub struct StatisticsCookieData<'a> {
    pub value: &'a str,
}

#[derive(Debug)]
pub struct CitationData<'a> {
    pub raw: &'a str,
    pub style: Option<&'a str>,
}

impl<'a> CitationData<'a> {
    #[inline]
    pub fn new(raw: &'a str, style: Option<&'a str>) -> Self {
        CitationData { raw, style }
    }
}

/// Whether a subscript or superscript is enclosed in curly brackets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Brackets {
    Bare,
    Bracketed,
}

/// Subscript vs superscript.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptKind {
    Sub,
    Sup,
}

/// Packed bitflag representation of [`ScriptKind`] and [`Brackets`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptFlags(u8);

impl ScriptFlags {
    const SUP: u8 = 0b01;
    const BRACKETED: u8 = 0b10;

    #[inline]
    pub fn new(kind: ScriptKind, brackets: Brackets) -> Self {
        let mut f = 0;
        if matches!(kind, ScriptKind::Sup) {
            f |= Self::SUP;
        }
        if matches!(brackets, Brackets::Bracketed) {
            f |= Self::BRACKETED;
        }
        ScriptFlags(f)
    }

    #[inline]
    pub fn kind(self) -> ScriptKind {
        if self.0 & Self::SUP != 0 {
            ScriptKind::Sup
        } else {
            ScriptKind::Sub
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimestampData<'a> {
    pub day_end: usize,
    pub day_start: usize,
    pub hour_end: Option<usize>,
    pub hour_start: Option<usize>,
    pub minute_end: Option<usize>,
    pub minute_start: Option<usize>,
    pub month_end: usize,
    pub month_start: usize,
    pub raw_value: &'a str,
    pub repeater_type: Option<RepeaterType>,
    pub repeater_unit: Option<TimeUnit>,
    pub repeater_value: Option<usize>,
    pub type_s: TimestampType,
    pub warning_type: Option<WarningType>,
    pub warning_unit: Option<TimeUnit>,
    pub warning_value: Option<usize>,
    pub year_end: usize,
    pub year_start: usize,
}

impl<'a> TimestampData<'a> {
    #[inline]
    pub fn new(raw: &'a str) -> Option<Self> {
        let raw = raw.trim();
        if raw.len() < 9 {
            return None;
        }

        let mut dash_parts = raw.split('-');
        let year_str = dash_parts.next()?;
        let month_str = dash_parts.next()?;
        let day_rest = dash_parts.next()?;
        let end_part = dash_parts.next();

        let year_trimmed = year_str.trim_start_matches('<').trim_start_matches('[');
        if year_trimmed.len() != 4 || month_str.len() != 2 {
            return None;
        }
        let year_start: usize = year_trimmed.parse().ok()?;
        let month_start: usize = month_str.parse().ok()?;
        if !(1..=12).contains(&month_start) {
            return None;
        }
        let day_str = day_rest.split_whitespace().next().unwrap_or("1");
        let day_trimmed = day_str.trim_end_matches('>').trim_end_matches(']');
        if day_trimmed.len() != 2 {
            return None;
        }
        let day_start: usize = day_trimmed.parse().ok()?;
        if !(1..=31).contains(&day_start) {
            return None;
        }

        let mut hour_start = None;
        let mut minute_start = None;
        for token in day_rest.split_whitespace().skip(1) {
            if token.contains(':') {
                let start_time = token.split('-').next().unwrap_or(token);
                let mut time_iter = start_time.split(':');
                if let (Some(h), Some(m)) = (time_iter.next(), time_iter.next()) {
                    hour_start = h.parse().ok();
                    minute_start = m.trim_end_matches(['>', ']']).parse().ok();
                }
                break;
            }
        }

        let (hour_end, minute_end) = if let Some(ep) = end_part {
            let end_trimmed = ep.trim_end_matches(['>', ']']);
            let mut ep_iter = end_trimmed.split(':');
            if let (Some(h), Some(m)) = (ep_iter.next(), ep_iter.next()) {
                (h.parse().ok(), m.parse().ok())
            } else {
                (hour_start, minute_start)
            }
        } else {
            (hour_start, minute_start)
        };

        Some(TimestampData {
            day_end: day_start,
            day_start,
            hour_end,
            hour_start,
            minute_end,
            minute_start,
            month_end: month_start,
            month_start,
            raw_value: raw,
            repeater_type: None,
            repeater_unit: None,
            repeater_value: None,
            type_s: if raw.starts_with('<') {
                TimestampType::Active
            } else {
                TimestampType::Inactive
            },
            warning_type: None,
            warning_unit: None,
            warning_value: None,
            year_end: year_start,
            year_start,
        })
    }
}

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum WarningType {
    All,
    First,
}

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum TimestampType {
    Active,
    ActiveRange,
    Diary,
    Inactive,
    InactiveRange,
}

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum RepeaterType {
    CatchUp,
    Restart,
    Cumulate,
}

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum TimeUnit {
    Year,
    Month,
    Week,
    Day,
    Hour,
}
