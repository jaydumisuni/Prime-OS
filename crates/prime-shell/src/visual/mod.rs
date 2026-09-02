pub(crate) mod background;
pub(crate) mod prime_launcher;
pub(crate) mod primitives;
#[cfg(test)]
mod proof;
pub(crate) mod quick_controls;
pub(crate) mod rail;
pub(crate) mod text;
pub(crate) mod theme;

pub(crate) use background::{
    paint_settled_background, paint_status_cluster, paint_top_status_strip, StatusClusterLayout,
    TopStatus, STATUS_CLUSTER_HEIGHT, STATUS_CLUSTER_RIGHT_MARGIN, STATUS_CLUSTER_TOP_MARGIN,
    STATUS_CLUSTER_WIDTH,
};
pub(crate) use prime_launcher::{
    paint_prime_launcher_surface, PrimeLauncherLayout, PRIME_LAUNCHER_HEIGHT, PRIME_LAUNCHER_WIDTH,
};
pub(crate) use primitives::{draw_icon, Argb, Canvas, Icon, Rect};
pub(crate) use quick_controls::{
    paint_quick_controls_surface, QuickControlsLayout, QuickControlsView, QUICK_HEIGHT, QUICK_WIDTH,
};
pub(crate) use rail::{
    paint_rail_surface, rail_height_for_items, RailAction, RailConfiguration, RailLayout,
    RAIL_LEFT_MARGIN, RAIL_TOP_MARGIN, RAIL_WIDTH,
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
            Icon::Prime,
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
    fn prime_is_the_only_fixed_default_rail_entry() {
        let config = RailConfiguration::default();
        assert_eq!(
            config.actions(),
            &[RailAction::Prime, RailAction::Apps, RailAction::Search,]
        );
    }

    #[test]
    fn rail_configuration_allows_optional_system_and_application_pins_without_duplicates() {
        let config = RailConfiguration::from_json(
            r#"{
              "schema":"prime.rail.v1",
              "pins":[
                {"kind":"apps"},
                {"kind":"search"},
                {"kind":"network"},
                {"kind":"application","application_id":"00000000-0000-0000-0000-000000000004"},
                {"kind":"network"},
                {"kind":"prime"}
              ]
            }"#,
        )
        .unwrap();
        assert_eq!(
            config.actions(),
            &[
                RailAction::Prime,
                RailAction::Apps,
                RailAction::Search,
                RailAction::Network,
                RailAction::Application("00000000-0000-0000-0000-000000000004".to_owned()),
            ]
        );
    }

    #[test]
    fn rail_configuration_round_trips_and_invalid_json_falls_back_to_default() {
        let config = RailConfiguration::from_json(
            r#"{"schema":"prime.rail.v1","pins":[{"kind":"audio"},{"kind":"storage"}]}"#,
        )
        .unwrap();
        let encoded = config.to_json().unwrap();
        assert_eq!(RailConfiguration::from_json(&encoded).unwrap(), config);
        assert_eq!(
            RailConfiguration::from_json_or_default("not-json"),
            RailConfiguration::default()
        );
    }

    #[test]
    fn rail_configuration_persists_in_user_writable_json() {
        let root =
            std::env::temp_dir().join(format!("prime-rail-config-test-{}", std::process::id()));
        let path = root.join("prime/rail.json");
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(
            RailConfiguration::load_from_path(&path),
            RailConfiguration::default()
        );
        let configured = RailConfiguration::from_json(
            r#"{"schema":"prime.rail.v1","pins":[{"kind":"search"},{"kind":"audio"}]}"#,
        )
        .unwrap();
        configured.save_to_path(&path).unwrap();
        assert_eq!(RailConfiguration::load_from_path(&path), configured);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rail_height_is_content_driven_and_hit_testing_returns_configured_action() {
        let actions = vec![RailAction::Prime, RailAction::Apps, RailAction::Search];
        let rail = RailLayout::for_output(1920, 1080, actions.len());
        assert_eq!(rail.bounds.width, 72);
        assert_eq!(rail.bounds.x, 28);
        assert_eq!(rail.bounds.y, 96);
        assert!(
            rail.bounds.height < 360,
            "three-item rail must stay compact"
        );
        assert_eq!(rail.items.len(), actions.len());
        for (index, rect) in rail.items.iter().enumerate() {
            assert_eq!(
                rail.hit(rect.center_x(), rect.center_y(), &actions),
                Some(actions[index].clone())
            );
        }
        assert_eq!(rail.hit(960.0, 540.0, &actions), None);
    }

    #[test]
    fn approved_wallpaper_has_no_placeholder_geometric_boxes() {
        assert_eq!(background::DECORATIVE_BOX_COUNT, 0);
    }

    #[test]
    fn prime_launcher_is_compact_and_avoids_engineering_ready_badges() {
        const {
            assert!(PRIME_LAUNCHER_HEIGHT <= 540);
        }
        assert_eq!(prime_launcher::application_state_label(true), None);
        assert_eq!(
            prime_launcher::application_state_label(false),
            Some("Unavailable")
        );
    }

    #[test]
    fn quick_control_truth_lines_resolve_to_visual_cards() {
        let network = quick_controls::quick_control_card("NET enp1s0: UP CARRIER");
        assert_eq!(network.label, "NETWORK");
        assert_eq!(network.value, "enp1s0: UP CARRIER");
        assert_eq!(network.icon, Icon::Network);
        let audio = quick_controls::quick_control_card("AUDIO PCH: ALC897");
        assert_eq!(audio.label, "AUDIO");
        assert_eq!(audio.value, "PCH: ALC897");
        assert_eq!(audio.icon, Icon::Audio);
        let health = quick_controls::quick_control_card("HEALTH: PROVING");
        assert_eq!(health.label, "HEALTH");
        assert_eq!(health.value, "PROVING");
        assert_eq!(health.icon, Icon::Health);
    }

    #[test]
    fn quick_controls_use_a_two_column_card_grid() {
        let layout = QuickControlsLayout::new(430, 600);
        let first = layout.card_rect(0);
        let second = layout.card_rect(1);
        let third = layout.card_rect(2);
        assert!(second.x > first.x);
        assert_eq!(second.y, first.y);
        assert!(third.y > first.y);
        assert_eq!(third.x, first.x);
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
    fn approved_wallpaper_is_aurora_mist_not_dense_wireframe() {
        const {
            assert!(background::PRIMARY_AURORA_BANDS <= 12);
            assert!(background::SECONDARY_AURORA_BANDS <= 6);
        }
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
        let actions = RailConfiguration::default().actions().to_vec();
        let rail = RailLayout::for_output(1920, 1080, actions.len());
        let mut bytes = vec![0u8; rail.bounds.width as usize * rail.bounds.height as usize * 4];
        let mut canvas = Canvas::new(&mut bytes, rail.bounds.width, rail.bounds.height).unwrap();
        paint_rail_surface(
            &mut canvas,
            &Theme::prime_dark(),
            &actions,
            Some(RailAction::Prime),
        );
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
    fn top_right_status_cluster_is_compact_and_clickable() {
        let layout = StatusClusterLayout::for_surface(176, 36);
        assert_eq!(layout.bounds, Rect::new(0, 0, 176, 36));
        assert!(layout.hit(150.0, 18.0));
        assert!(!layout.hit(-1.0, 18.0));
    }

    #[test]
    fn status_cluster_painter_keeps_transparent_corners_and_renders_truth() {
        let theme = Theme::prime_dark();
        let mut text = TextSystem::load_system().expect("Prime production font");
        let mut bytes = vec![0u8; 176 * 36 * 4];
        let mut canvas = Canvas::new(&mut bytes, 176, 36).unwrap();
        paint_status_cluster(&mut canvas, &mut text, &theme, TopStatus::Online);
        assert_eq!(canvas.pixel(0, 0).unwrap(), Argb::TRANSPARENT);
        let bright = (0..36)
            .flat_map(|y| (0..176).map(move |x| (x, y)))
            .filter(|&(x, y)| canvas.pixel(x, y).is_some_and(|p| p.a > 160 && p.g > 150))
            .count();
        assert!(bright > 20);
    }

    #[test]
    fn top_status_truth_labels_are_explicit() {
        assert_eq!(TopStatus::Online.label(), "ONLINE");
        assert_eq!(TopStatus::Limited.label(), "LIMITED");
    }

    #[test]
    fn prime_launcher_layout_maps_two_column_application_cards() {
        let layout = PrimeLauncherLayout::new(520, 600);
        let first = layout.card_rect(0);
        let second = layout.card_rect(1);
        let third = layout.card_rect(2);
        assert_eq!(
            layout.application_at(first.center_x(), first.center_y(), 3),
            Some(0)
        );
        assert_eq!(
            layout.application_at(second.center_x(), second.center_y(), 3),
            Some(1)
        );
        assert_eq!(
            layout.application_at(third.center_x(), third.center_y(), 3),
            Some(2)
        );
        assert!(second.x > first.x);
        assert!(third.y > first.y);
        assert_eq!(layout.application_at(8.0, 8.0, 3), None);
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
    fn approved_top_strip_uses_production_text_and_rail_is_icon_first() {
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

        let actions = RailConfiguration::default().actions().to_vec();
        let rail = RailLayout::for_output(1920, 1080, actions.len());
        let mut rail_bytes =
            vec![0u8; rail.bounds.width as usize * rail.bounds.height as usize * 4];
        let mut rail_canvas =
            Canvas::new(&mut rail_bytes, rail.bounds.width, rail.bounds.height).unwrap();
        paint_rail_surface(&mut rail_canvas, &theme, &actions, Some(RailAction::Prime));
        let bright_icon_pixels = (0..rail.bounds.height as i32)
            .flat_map(|y| (0..rail.bounds.width as i32).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                rail_canvas
                    .pixel(x, y)
                    .is_some_and(|p| p.a > 180 && p.b > 150)
            })
            .count();
        assert!(bright_icon_pixels > 40);
    }
}
