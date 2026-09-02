use std::{fs, io, path::Path};

use serde_json::{json, Value};

use super::{draw_icon, Argb, Canvas, Icon, Rect, Theme};

pub(crate) const RAIL_WIDTH: u32 = 72;
pub(crate) const RAIL_LEFT_MARGIN: i32 = 28;
pub(crate) const RAIL_TOP_MARGIN: i32 = 96;
pub(crate) const MAX_RAIL_ITEMS: usize = 8;
const RAIL_VERTICAL_PADDING: u32 = 10;
const RAIL_ITEM_HEIGHT: u32 = 54;
const RAIL_ITEM_GAP: u32 = 6;
const RAIL_SCHEMA: &str = "prime.rail.v1";

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) enum RailAction {
    Prime,
    Apps,
    Search,
    Status,
    Network,
    Audio,
    Storage,
    Health,
    Application(String),
}

impl RailAction {
    fn icon(&self) -> Icon {
        match self {
            Self::Prime => Icon::Prime,
            Self::Apps | Self::Application(_) => Icon::Applications,
            Self::Search => Icon::Search,
            Self::Status => Icon::Status,
            Self::Network => Icon::Network,
            Self::Audio => Icon::Audio,
            Self::Storage => Icon::Storage,
            Self::Health => Icon::Health,
        }
    }

    fn pin_json(&self) -> Option<Value> {
        match self {
            Self::Prime => None,
            Self::Apps => Some(json!({"kind":"apps"})),
            Self::Search => Some(json!({"kind":"search"})),
            Self::Status => Some(json!({"kind":"status"})),
            Self::Network => Some(json!({"kind":"network"})),
            Self::Audio => Some(json!({"kind":"audio"})),
            Self::Storage => Some(json!({"kind":"storage"})),
            Self::Health => Some(json!({"kind":"health"})),
            Self::Application(application_id) => Some(json!({
                "kind":"application",
                "application_id":application_id,
            })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RailConfiguration {
    actions: Vec<RailAction>,
}

impl Default for RailConfiguration {
    fn default() -> Self {
        Self {
            actions: vec![RailAction::Prime, RailAction::Apps, RailAction::Search],
        }
    }
}

impl RailConfiguration {
    pub(crate) fn actions(&self) -> &[RailAction] {
        &self.actions
    }

    pub(crate) fn from_json(source: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(source).map_err(|error| error.to_string())?;
        if value.get("schema").and_then(Value::as_str) != Some(RAIL_SCHEMA) {
            return Err(format!("rail schema must be {RAIL_SCHEMA}"));
        }
        let pins = value
            .get("pins")
            .and_then(Value::as_array)
            .ok_or_else(|| "rail pins must be an array".to_owned())?;
        let mut actions = vec![RailAction::Prime];
        for pin in pins {
            if actions.len() >= MAX_RAIL_ITEMS {
                break;
            }
            let kind = pin
                .get("kind")
                .and_then(Value::as_str)
                .ok_or_else(|| "rail pin kind must be a string".to_owned())?;
            let action = match kind {
                "prime" => continue,
                "apps" => RailAction::Apps,
                "search" => RailAction::Search,
                "status" => RailAction::Status,
                "network" => RailAction::Network,
                "audio" => RailAction::Audio,
                "storage" => RailAction::Storage,
                "health" => RailAction::Health,
                "application" => {
                    let application_id = pin
                        .get("application_id")
                        .and_then(Value::as_str)
                        .filter(|id| !id.trim().is_empty())
                        .ok_or_else(|| "application rail pin requires application_id".to_owned())?;
                    RailAction::Application(application_id.to_owned())
                }
                other => return Err(format!("unsupported rail pin kind: {other}")),
            };
            if !actions.contains(&action) {
                actions.push(action);
            }
        }
        Ok(Self { actions })
    }

    pub(crate) fn from_json_or_default(source: &str) -> Self {
        Self::from_json(source).unwrap_or_default()
    }

    pub(crate) fn load_from_path(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(source) => Self::from_json_or_default(&source),
            Err(_) => Self::default(),
        }
    }

    pub(crate) fn save_to_path(&self, path: &Path) -> Result<(), io::Error> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let encoded = self
            .to_json()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        fs::write(path, encoded)
    }

    pub(crate) fn to_json(&self) -> Result<String, serde_json::Error> {
        let pins = self
            .actions
            .iter()
            .filter_map(RailAction::pin_json)
            .collect::<Vec<_>>();
        serde_json::to_string_pretty(&json!({
            "schema": RAIL_SCHEMA,
            "pins": pins,
        }))
    }
}

pub(crate) fn rail_height_for_items(item_count: usize) -> u32 {
    let count = item_count.clamp(1, MAX_RAIL_ITEMS) as u32;
    RAIL_VERTICAL_PADDING * 2 + count * RAIL_ITEM_HEIGHT + count.saturating_sub(1) * RAIL_ITEM_GAP
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RailLayout {
    pub(crate) bounds: Rect,
    pub(crate) items: Vec<Rect>,
}

impl RailLayout {
    #[cfg(test)]
    pub(crate) fn for_output(output_width: u32, output_height: u32, item_count: usize) -> Self {
        let desired_height = rail_height_for_items(item_count);
        let max_height = output_height.saturating_sub(RAIL_TOP_MARGIN.max(0) as u32 + 28);
        let height = desired_height.min(max_height.max(RAIL_ITEM_HEIGHT + 20));
        let x = RAIL_LEFT_MARGIN
            .min(output_width.saturating_sub(RAIL_WIDTH) as i32)
            .max(0);
        let y = RAIL_TOP_MARGIN
            .min(output_height.saturating_sub(height) as i32)
            .max(0);
        Self::from_bounds(Rect::new(x, y, RAIL_WIDTH, height), item_count)
    }

    pub(crate) fn for_surface(width: u32, height: u32, item_count: usize) -> Self {
        Self::from_bounds(Rect::new(0, 0, width, height), item_count)
    }

    fn from_bounds(bounds: Rect, item_count: usize) -> Self {
        let count = item_count.clamp(1, MAX_RAIL_ITEMS);
        let item_width = bounds.width.saturating_sub(14).max(40);
        let item_x = bounds.x + ((bounds.width.saturating_sub(item_width)) / 2) as i32;
        let available = bounds.height.saturating_sub(RAIL_VERTICAL_PADDING * 2);
        let gap_total = RAIL_ITEM_GAP * count.saturating_sub(1) as u32;
        let item_height = RAIL_ITEM_HEIGHT.min(available.saturating_sub(gap_total) / count as u32);
        let items = (0..count)
            .map(|index| {
                Rect::new(
                    item_x,
                    bounds.y
                        + RAIL_VERTICAL_PADDING as i32
                        + index as i32 * (item_height + RAIL_ITEM_GAP) as i32,
                    item_width,
                    item_height,
                )
            })
            .collect();
        Self { bounds, items }
    }

    pub(crate) fn hit(&self, x: f64, y: f64, actions: &[RailAction]) -> Option<RailAction> {
        let x = x.floor() as i32;
        let y = y.floor() as i32;
        self.items
            .iter()
            .zip(actions.iter())
            .find_map(|(rect, action)| rect.contains(x, y).then(|| action.clone()))
    }
}

pub(crate) fn paint_rail_surface(
    canvas: &mut Canvas<'_>,
    theme: &Theme,
    actions: &[RailAction],
    active: Option<RailAction>,
) {
    canvas.clear();
    let layout = RailLayout::for_surface(canvas.width, canvas.height, actions.len());
    let body = Rect::new(
        1,
        1,
        canvas.width.saturating_sub(2),
        canvas.height.saturating_sub(2),
    );
    canvas.fill_rounded_rect(body, 24, theme.panel.with_alpha(108));
    canvas.stroke_rounded_rect(body, 24, 1, theme.text.with_alpha(54));
    canvas.fill_rounded_rect(
        Rect::new(7, 5, canvas.width.saturating_sub(14), 1),
        1,
        theme.text.with_alpha(36),
    );

    if let Some(prime_rect) = layout.items.first() {
        canvas.radial_glow(
            prime_rect.center_x() as f32,
            prime_rect.center_y() as f32,
            44.0,
            theme.violet.with_alpha(96),
        );
    }

    for (rect, action) in layout.items.iter().zip(actions.iter()) {
        let is_active = active.as_ref() == Some(action);
        if is_active {
            canvas.fill_rounded_rect(*rect, 16, theme.violet.with_alpha(48));
            canvas.stroke_rounded_rect(*rect, 16, 1, theme.cyan.with_alpha(136));
        }
        let icon_size = if matches!(action, RailAction::Prime) {
            30
        } else {
            21
        };
        let icon_rect = Rect::new(
            rect.center_x().round() as i32 - icon_size as i32 / 2,
            rect.center_y().round() as i32 - icon_size as i32 / 2,
            icon_size,
            icon_size,
        );
        let color = if is_active {
            theme.cyan
        } else if matches!(action, RailAction::Prime) {
            Argb::from_u32(0xffc4b5fd)
        } else {
            theme.text.with_alpha(214)
        };
        draw_icon(canvas, icon_rect, action.icon(), color);
    }
}
