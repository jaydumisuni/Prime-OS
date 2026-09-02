use super::{draw_icon, Argb, Canvas, Icon, Rect, Theme};

pub(crate) const RAIL_WIDTH: u32 = 72;
pub(crate) const RAIL_HEIGHT: u32 = 620;
pub(crate) const RAIL_LEFT_MARGIN: i32 = 28;
pub(crate) const RAIL_TOP_MARGIN: i32 = 96;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RailAction {
    Orb,
    Apps,
    Search,
    Status,
    Network,
    Audio,
    Storage,
    Health,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RailLayout {
    pub(crate) bounds: Rect,
    pub(crate) orb: Rect,
    pub(crate) apps: Rect,
    pub(crate) search: Rect,
    pub(crate) status: Rect,
    pub(crate) network: Rect,
    pub(crate) audio: Rect,
    pub(crate) storage: Rect,
    pub(crate) health: Rect,
}

impl RailLayout {
    #[cfg(test)]
    pub(crate) fn for_output(output_width: u32, output_height: u32) -> Self {
        let max_height = output_height.saturating_sub(RAIL_TOP_MARGIN as u32 + 52);
        let height = RAIL_HEIGHT.min(max_height.max(520));
        let x = RAIL_LEFT_MARGIN
            .min(output_width.saturating_sub(RAIL_WIDTH) as i32)
            .max(0);
        let y = RAIL_TOP_MARGIN
            .min(output_height.saturating_sub(height) as i32)
            .max(0);
        Self::from_bounds(Rect::new(x, y, RAIL_WIDTH, height))
    }

    pub(crate) fn for_surface(width: u32, height: u32) -> Self {
        Self::from_bounds(Rect::new(0, 0, width, height))
    }

    fn from_bounds(bounds: Rect) -> Self {
        let width = bounds.width;
        let height = bounds.height;
        let item_width = width.saturating_sub(14).max(40);
        let item_height = (height.saturating_sub(28) / 8).max(54);
        let item_x = bounds.x + ((width.saturating_sub(item_width)) / 2) as i32;
        let start_y = bounds.y + 10;
        let item = |index: u32| {
            Rect::new(
                item_x,
                start_y + (index * item_height) as i32,
                item_width,
                item_height.saturating_sub(4),
            )
        };
        Self {
            bounds,
            orb: item(0),
            apps: item(1),
            search: item(2),
            status: item(3),
            network: item(4),
            audio: item(5),
            storage: item(6),
            health: item(7),
        }
    }

    pub(crate) fn hit(self, x: f64, y: f64) -> Option<RailAction> {
        let x = x.floor() as i32;
        let y = y.floor() as i32;
        [
            (self.orb, RailAction::Orb),
            (self.apps, RailAction::Apps),
            (self.search, RailAction::Search),
            (self.status, RailAction::Status),
            (self.network, RailAction::Network),
            (self.audio, RailAction::Audio),
            (self.storage, RailAction::Storage),
            (self.health, RailAction::Health),
        ]
        .into_iter()
        .find_map(|(rect, action)| rect.contains(x, y).then_some(action))
    }
}

pub(crate) fn paint_rail_surface(
    canvas: &mut Canvas<'_>,
    theme: &Theme,
    active: Option<RailAction>,
) {
    canvas.clear();
    let layout = RailLayout::for_surface(canvas.width, canvas.height);
    let body = Rect::new(
        1,
        1,
        canvas.width.saturating_sub(2),
        canvas.height.saturating_sub(2),
    );
    canvas.fill_rounded_rect(body, 24, theme.panel.with_alpha(118));
    canvas.stroke_rounded_rect(body, 24, 1, theme.text.with_alpha(62));
    let highlight = Rect::new(6, 5, canvas.width.saturating_sub(12), 1);
    canvas.fill_rounded_rect(highlight, 1, theme.text.with_alpha(42));

    canvas.radial_glow(
        layout.orb.center_x() as f32,
        layout.orb.center_y() as f32,
        52.0,
        theme.violet.with_alpha(112),
    );
    canvas.radial_glow(
        layout.status.center_x() as f32,
        layout.status.center_y() as f32,
        38.0,
        theme.cyan.with_alpha(34),
    );

    let items = [
        (layout.orb, RailAction::Orb, Icon::Orb),
        (layout.apps, RailAction::Apps, Icon::Applications),
        (layout.search, RailAction::Search, Icon::Search),
        (layout.status, RailAction::Status, Icon::Status),
        (layout.network, RailAction::Network, Icon::Network),
        (layout.audio, RailAction::Audio, Icon::Audio),
        (layout.storage, RailAction::Storage, Icon::Storage),
        (layout.health, RailAction::Health, Icon::Health),
    ];
    for (rect, action, icon) in items {
        if active == Some(action) {
            canvas.fill_rounded_rect(rect, 16, theme.violet.with_alpha(54));
            canvas.stroke_rounded_rect(rect, 16, 1, theme.cyan.with_alpha(148));
        }
        let icon_size = if action == RailAction::Orb { 32 } else { 22 };
        let icon_rect = Rect::new(
            rect.center_x().round() as i32 - icon_size as i32 / 2,
            rect.center_y().round() as i32 - icon_size as i32 / 2,
            icon_size,
            icon_size,
        );
        let color = if active == Some(action) {
            theme.cyan
        } else if action == RailAction::Orb {
            Argb::from_u32(0xffc4b5fd)
        } else {
            theme.text.with_alpha(220)
        };
        draw_icon(canvas, icon_rect, icon, color);
    }
}
