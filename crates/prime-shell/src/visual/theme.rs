use super::primitives::Argb;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Theme {
    pub(crate) base_0: Argb,
    pub(crate) base_1: Argb,
    pub(crate) base_2: Argb,
    pub(crate) panel: Argb,
    pub(crate) cyan: Argb,
    pub(crate) cyan_alt: Argb,
    pub(crate) violet: Argb,
    pub(crate) violet_alt: Argb,
    pub(crate) text: Argb,
    pub(crate) muted: Argb,
}

impl Theme {
    pub(crate) const fn prime_dark() -> Self {
        Self {
            base_0: Argb::from_u32(0xff050916),
            base_1: Argb::from_u32(0xff071326),
            base_2: Argb::from_u32(0xff0a2030),
            panel: Argb::from_u32(0xff0f172a),
            cyan: Argb::from_u32(0xff22d3ee),
            cyan_alt: Argb::from_u32(0xff06b6d4),
            violet: Argb::from_u32(0xff8b5cf6),
            violet_alt: Argb::from_u32(0xffa855f7),
            text: Argb::from_u32(0xfff8fafc),
            muted: Argb::from_u32(0xff94a3b8),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::prime_dark()
    }
}
