pub(crate) mod background;
pub(crate) mod orb;
pub(crate) mod primitives;
pub(crate) mod quick_controls;
pub(crate) mod rail;
pub(crate) mod text;
pub(crate) mod theme;

pub(crate) use background::{paint_settled_background, paint_top_status_strip, TopStatus};
pub(crate) use orb::{paint_orb_surface, OrbLayout, ORB_HEIGHT, ORB_WIDTH};
pub(crate) use primitives::{draw_icon, Argb, Canvas, Icon, Rect};
pub(crate) use quick_controls::{
    paint_quick_controls_surface, QuickControlsLayout, QuickControlsView, QUICK_HEIGHT, QUICK_WIDTH,
};
pub(crate) use rail::{
    paint_rail_labels, paint_rail_surface, RailAction, RailLayout, RAIL_HEIGHT, RAIL_LEFT_MARGIN,
    RAIL_TOP_MARGIN, RAIL_WIDTH,
};
pub(crate) use text::{FontWeight, TextStyle, TextSystem};
pub(crate) use theme::Theme;

#[cfg(test)]
mod tests {
    use super::*;
    use prime_contracts::SystemPowerAction;

    #[test]
    fn alpha_blend_preserves_opaque_destination() {
        let dst = Argb::from_u32(0xff050818);
        let src = Argb::from_u32(0x8022d3ee);
        let mixed = src.over(dst);
        assert_eq!(mixed.a, 255);
        assert!(mixed.g > dst.g);
        assert!(mixed.b > dst.b);
    }

    #[test]
    fn rect_geometry_contains_and_centers() {
        let rect = Rect::new(10, 20, 40, 60);
        assert!(rect.contains(10, 20));
        assert!(rect.contains(49, 79));
        assert!(!rect.contains(50, 80));
        assert_eq!(rect.center_x(), 30.0);
        assert_eq!(rect.center_y(), 50.0);
    }

    #[test]
    fn canvas_fill_clear_and_rounded_geometry_are_bounded() {
        let mut bytes = vec![0u8; 32 * 32 * 4];
        let mut canvas = Canvas::new(&mut bytes, 32, 32).unwrap();
        canvas.fill_rect(Rect::new(4, 4, 8, 8), Argb::from_u32(0xff22d3ee));
        assert_eq!(canvas.pixel(4, 4).unwrap(), Argb::from_u32(0xff22d3ee));
        assert_eq!(canvas.pixel(3, 3).unwrap(), Argb::TRANSPARENT);
        canvas.clear();
        assert_eq!(canvas.pixel(4, 4).unwrap(), Argb::TRANSPARENT);
        canvas.fill_rounded_rect(Rect::new(0, 0, 32, 32), 10, Argb::from_u32(0xcc0f172a));
        assert_eq!(canvas.pixel(0, 0).unwrap().a, 0);
        assert!(canvas.pixel(16, 16).unwrap().a > 0);
    }

    #[test]
    fn stroke_gradient_and_glow_have_distinct_material_behavior() {
        let mut bytes = vec![0u8; 64 * 64 * 4];
        let mut canvas = Canvas::new(&mut bytes, 64, 64).unwrap();
        canvas.stroke_rounded_rect(Rect::new(4, 4, 40, 40), 8, 2, Argb::from_u32(0xff8b5cf6));
        assert!(canvas.pixel(24, 4).unwrap().a > 0);
        assert_eq!(canvas.pixel(24, 24).unwrap().a, 0);
        canvas.vertical_gradient(
            Rect::new(48, 0, 8, 32),
            Argb::from_u32(0xff05050d),
            Argb::from_u32(0xff071021),
        );
        assert_ne!(canvas.pixel(50, 1).unwrap(), canvas.pixel(50, 30).unwrap());
        canvas.radial_glow(32.0, 52.0, 10.0, Argb::from_u32(0x8022d3ee));
        assert!(canvas.pixel(32, 52).unwrap().a > canvas.pixel(23, 52).unwrap().a);
    }

    #[test]
    fn circle_and_line_primitives_touch_expected_pixels_only() {
        let mut bytes = vec![0u8; 32 * 32 * 4];
        let mut canvas = Canvas::new(&mut bytes, 32, 32).unwrap();
        let color = Argb::from_u32(0xfff8fafc);
        canvas.circle(8, 8, 3, color);
        assert_eq!(canvas.pixel(8, 8).unwrap(), color);
        assert_eq!(canvas.pixel(0, 0).unwrap(), Argb::TRANSPARENT);
        canvas.line((16, 4), (16, 20), 1, color);
        assert_eq!(canvas.pixel(16, 12).unwrap(), color);
        assert_eq!(canvas.pixel(15, 12).unwrap(), Argb::TRANSPARENT);
    }

    #[test]
    fn prime_dark_theme_matches_brand_authority() {
        let theme = Theme::prime_dark();
        assert_eq!(theme.base_0, Argb::from_u32(0xff05050d));
        assert_eq!(theme.base_1, Argb::from_u32(0xff050818));
        assert_eq!(theme.base_2, Argb::from_u32(0xff071021));
        assert_eq!(theme.panel, Argb::from_u32(0xff0f172a));
        assert_eq!(theme.cyan, Argb::from_u32(0xff22d3ee));
        assert_eq!(theme.cyan_alt, Argb::from_u32(0xff06b6d4));
        assert_eq!(theme.violet, Argb::from_u32(0xff8b5cf6));
        assert_eq!(theme.violet_alt, Argb::from_u32(0xffa855f7));
        assert_eq!(theme.text, Argb::from_u32(0xfff8fafc));
        assert_eq!(theme.muted, Argb::from_u32(0xff94a3b8));
    }

    #[test]
    fn font_family_preference_is_noto_then_dejavu() {
        assert_eq!(text::preferred_families(), ["Noto Sans", "DejaVu Sans"]);
    }

    #[test]
    fn glyph_coverage_scales_text_alpha() {
        let color = Argb::from_u32(0xfff8fafc);
        assert_eq!(text::coverage_color(color, 128).a, 128);
        assert_eq!(text::coverage_color(color, 0), Argb::TRANSPARENT);
        assert_eq!(text::coverage_color(color, 255), color);
    }

    #[test]
    fn text_styles_distinguish_regular_and_semibold_hierarchy() {
        assert_eq!(TextStyle::body().weight, FontWeight::Regular);
        assert_eq!(TextStyle::title().weight, FontWeight::Semibold);
        assert!(TextStyle::title().size_px > TextStyle::body().size_px);
    }

    #[test]
    fn system_text_rasterizes_antialiased_prime_copy() {
        let mut text = TextSystem::load_system().expect("KRATOS must provide a Prime Shell font");
        assert!(["Noto Sans", "DejaVu Sans"].contains(&text.family_name()));
        let metrics = text.measure("Prime", TextStyle::body());
        assert!(metrics.width > 0);
        assert!(metrics.height > 0);

        let mut bytes = vec![0u8; 320 * 80 * 4];
        let mut canvas = Canvas::new(&mut bytes, 320, 80).unwrap();
        text.draw(
            &mut canvas,
            (8, 8),
            "Prime",
            TextStyle::body(),
            Argb::from_u32(0xfff8fafc),
        );
        let painted = (0..80)
            .flat_map(|y| (0..320).map(move |x| (x, y)))
            .filter(|&(x, y)| canvas.pixel(x, y).is_some_and(|pixel| pixel.a > 0))
            .count();
        assert!(painted > 32);
    }

    #[test]
    fn every_prime_system_icon_renders_geometry() {
        let icons = [
            Icon::Orb,
            Icon::Applications,
            Icon::Status,
            Icon::Network,
            Icon::Audio,
            Icon::Storage,
            Icon::Health,
            Icon::Restart,
            Icon::Power,
            Icon::Search,
            Icon::Chevron,
            Icon::Blocked,
        ];
        for icon in icons {
            let mut bytes = vec![0u8; 32 * 32 * 4];
            let mut canvas = Canvas::new(&mut bytes, 32, 32).unwrap();
            draw_icon(
                &mut canvas,
                Rect::new(0, 0, 32, 32),
                icon,
                Argb::from_u32(0xff22d3ee),
            );
            let painted = (0..32)
                .flat_map(|y| (0..32).map(move |x| (x, y)))
                .filter(|&(x, y)| canvas.pixel(x, y).is_some_and(|pixel| pixel.a > 0))
                .count();
            assert!(painted > 4, "{icon:?} produced no meaningful geometry");
        }
    }

    #[test]
    fn kratos_1080p_rail_is_vertical_and_floating() {
        let rail = RailLayout::for_output(1920, 1080);
        assert!(rail.bounds.width <= 96);
        assert!(rail.bounds.height > 400);
        assert!(rail.bounds.x >= 12);
        assert!(rail.bounds.y >= 40);
        assert!(rail.bounds.height > rail.bounds.width * 4);
    }

    #[test]
    fn approved_rail_entries_resolve_to_real_shell_actions() {
        let rail = RailLayout::for_output(1920, 1080);
        let expected = [
            (rail.orb, RailAction::Orb),
            (rail.apps, RailAction::Apps),
            (rail.search, RailAction::Search),
            (rail.status, RailAction::Status),
            (rail.network, RailAction::Network),
            (rail.audio, RailAction::Audio),
            (rail.storage, RailAction::Storage),
            (rail.health, RailAction::Health),
        ];
        for (rect, action) in expected {
            assert_eq!(rail.hit(rect.center_x(), rect.center_y()), Some(action));
        }
    }

    #[test]
    fn rail_hit_targets_map_orb_and_status() {
        let rail = RailLayout::for_output(1920, 1080);
        assert_eq!(
            rail.hit(rail.orb.center_x(), rail.orb.center_y()),
            Some(RailAction::Orb)
        );
        assert_eq!(
            rail.hit(rail.status.center_x(), rail.status.center_y()),
            Some(RailAction::Status)
        );
        assert_eq!(rail.hit(960.0, 540.0), None);
    }

    #[test]
    fn settled_background_is_prime_dark_without_permanent_white_center_mark() {
        let mut bytes = vec![0u8; 320 * 180 * 4];
        let mut canvas = Canvas::new(&mut bytes, 320, 180).unwrap();
        paint_settled_background(&mut canvas, &Theme::prime_dark());
        let center = canvas.pixel(160, 90).unwrap();
        let top = canvas.pixel(160, 4).unwrap();
        let bottom = canvas.pixel(160, 175).unwrap();
        assert_eq!(center.a, 255);
        assert_ne!(center, Argb::from_u32(0xfff8fafc));
        assert_ne!(top, bottom);
    }

    #[test]
    fn wallpaper_carries_violet_and_cyan_energy_through_the_desktop_body() {
        let mut bytes = vec![0u8; 480 * 270 * 4];
        let mut canvas = Canvas::new(&mut bytes, 480, 270).unwrap();
        paint_settled_background(&mut canvas, &Theme::prime_dark());
        let mut cyan_pixels = 0usize;
        let mut violet_pixels = 0usize;
        for y in 24..250 {
            for x in 24..456 {
                let pixel = canvas.pixel(x, y).unwrap();
                if pixel.g > 65 && pixel.b > 90 && pixel.b > pixel.r + 20 {
                    cyan_pixels += 1;
                }
                if pixel.r > 55 && pixel.b > 85 && pixel.b > pixel.g + 12 {
                    violet_pixels += 1;
                }
            }
        }
        assert!(
            cyan_pixels > 800,
            "wallpaper cyan energy is too edge-only or too weak"
        );
        assert!(
            violet_pixels > 800,
            "wallpaper violet energy is too edge-only or too weak"
        );
    }

    #[test]
    fn rail_surface_has_transparent_corners_and_brand_lit_body() {
        let rail = RailLayout::for_output(1920, 1080);
        let mut bytes = vec![0u8; rail.bounds.width as usize * rail.bounds.height as usize * 4];
        let mut canvas = Canvas::new(&mut bytes, rail.bounds.width, rail.bounds.height).unwrap();
        paint_rail_surface(&mut canvas, &Theme::prime_dark(), Some(RailAction::Orb));
        assert_eq!(canvas.pixel(0, 0).unwrap(), Argb::TRANSPARENT);
        assert!(
            canvas
                .pixel(
                    (rail.bounds.width / 2) as i32,
                    (rail.bounds.height / 2) as i32
                )
                .unwrap()
                .a
                > 0
        );
        let lit_pixels = (0..rail.bounds.height as i32)
            .flat_map(|y| (0..rail.bounds.width as i32).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                canvas
                    .pixel(x, y)
                    .is_some_and(|pixel| pixel.b > 150 && pixel.g > 100 && pixel.a > 100)
            })
            .count();
        assert!(lit_pixels > 20);
    }

    #[test]
    fn top_status_truth_labels_are_explicit() {
        assert_eq!(TopStatus::Online.label(), "ONLINE");
        assert_eq!(TopStatus::Limited.label(), "LIMITED");
    }

    #[test]
    fn orb_layout_maps_only_application_cards() {
        let layout = OrbLayout::new(520, 600);
        assert_eq!(
            layout.row_at(layout.apps.x as f64 + 20.0, layout.apps.y as f64 + 20.0, 3),
            Some(0)
        );
        assert_eq!(
            layout.row_at(layout.apps.x as f64 + 20.0, layout.apps.y as f64 + 112.0, 3),
            Some(1)
        );
        assert_eq!(layout.row_at(8.0, 8.0, 3), None);
    }

    #[test]
    fn quick_controls_power_actions_are_bounded_to_cards() {
        let layout = QuickControlsLayout::new(430, 600);
        assert_eq!(
            layout.power_action_at(layout.restart.center_x(), layout.restart.center_y()),
            Some(SystemPowerAction::Reboot)
        );
        assert_eq!(
            layout.power_action_at(layout.power_off.center_x(), layout.power_off.center_y()),
            Some(SystemPowerAction::PowerOff)
        );
        assert_eq!(layout.power_action_at(10.0, 10.0), None);
    }

    #[test]
    fn approved_top_strip_and_rail_labels_use_production_text() {
        let theme = Theme::prime_dark();
        let mut text = TextSystem::load_system().expect("Prime production font");

        let mut desktop_bytes = vec![0u8; 480 * 270 * 4];
        let mut desktop = Canvas::new(&mut desktop_bytes, 480, 270).unwrap();
        paint_settled_background(&mut desktop, &theme);
        let before_top = (0..44)
            .flat_map(|y| (0..480).map(move |x| (x, y)))
            .filter(|&(x, y)| desktop.pixel(x, y).is_some_and(|p| p.r > 180 && p.g > 180))
            .count();
        paint_top_status_strip(&mut desktop, &mut text, &theme, TopStatus::Online);
        let after_top = (0..44)
            .flat_map(|y| (0..480).map(move |x| (x, y)))
            .filter(|&(x, y)| desktop.pixel(x, y).is_some_and(|p| p.r > 180 && p.g > 180))
            .count();
        assert!(after_top > before_top);

        let rail = RailLayout::for_output(1920, 1080);
        let mut rail_bytes =
            vec![0u8; rail.bounds.width as usize * rail.bounds.height as usize * 4];
        let mut rail_canvas =
            Canvas::new(&mut rail_bytes, rail.bounds.width, rail.bounds.height).unwrap();
        paint_rail_surface(&mut rail_canvas, &theme, None);
        let before_rail = (0..rail.bounds.height as i32)
            .flat_map(|y| (0..rail.bounds.width as i32).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                rail_canvas
                    .pixel(x, y)
                    .is_some_and(|p| p.r > 205 && p.g > 205 && p.b > 205)
            })
            .count();
        paint_rail_labels(&mut rail_canvas, &mut text, &theme);
        let after_rail = (0..rail.bounds.height as i32)
            .flat_map(|y| (0..rail.bounds.width as i32).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                rail_canvas
                    .pixel(x, y)
                    .is_some_and(|p| p.r > 205 && p.g > 205 && p.b > 205)
            })
            .count();
        assert!(after_rail > before_rail);
    }
}
