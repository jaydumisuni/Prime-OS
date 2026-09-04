pub(crate) mod background;
pub(crate) mod prime_launcher;
pub(crate) mod primitives;
#[cfg(test)]
mod proof;
pub(crate) mod quick_controls;
pub(crate) mod rail;
pub(crate) mod text;
pub(crate) mod theme;
pub(crate) mod wallpaper;

#[cfg(test)]
pub(crate) use background::paint_settled_background;
pub(crate) use background::{
    paint_background_base, paint_background_motion, paint_status_cluster, paint_top_status_strip,
    StatusClusterLayout, TopStatus, STATUS_CLUSTER_HEIGHT, STATUS_CLUSTER_RIGHT_MARGIN,
    STATUS_CLUSTER_TOP_MARGIN, STATUS_CLUSTER_WIDTH,
};
pub(crate) use prime_launcher::{
    paint_prime_launcher_surface, PrimeLauncherLayout, PrimeLauncherView, PRIME_LAUNCHER_HEIGHT,
    PRIME_LAUNCHER_LEFT_MARGIN, PRIME_LAUNCHER_TOP_MARGIN, PRIME_LAUNCHER_WIDTH,
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
        let dst = Argb::from_u32(0xff071326);
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
            Argb::from_u32(0xff050916),
            Argb::from_u32(0xff0a2030),
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
        assert_eq!(theme.base_0, Argb::from_u32(0xff050916));
        assert_eq!(theme.base_1, Argb::from_u32(0xff071326));
        assert_eq!(theme.base_2, Argb::from_u32(0xff0a2030));
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
            Icon::Files,
            Icon::Terminal,
            Icon::Browser,
            Icon::Settings,
            Icon::Media,
            Icon::Recovery,
            Icon::Status,
            Icon::Network,
            Icon::Audio,
            Icon::Storage,
            Icon::Health,
            Icon::Restart,
            Icon::Power,
            Icon::Search,
            Icon::Chevron,
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
        assert_eq!(config.actions(), &[RailAction::Prime]);
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
            RailConfiguration::from_json("not-json").unwrap_or_default(),
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
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r#"{"schema":"prime.rail.v1","pins":[{"kind":"apps"},{"kind":"search"},{"kind":"audio"}]}"#,
        )
        .unwrap();
        let migrated = RailConfiguration::load_from_path(&path);
        assert_eq!(migrated.actions(), &[RailAction::Prime, RailAction::Audio]);
        let persisted = std::fs::read_to_string(&path).unwrap();
        assert!(!persisted.contains("\"apps\""));
        assert!(!persisted.contains("\"search\""));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rail_height_is_content_driven_and_hit_testing_returns_configured_action() {
        let actions = vec![RailAction::Prime];
        let rail = RailLayout::for_output(1920, 1080, actions.len());
        assert_eq!(rail.bounds.width, 132);
        assert_eq!(rail.bounds.x, 56);
        assert_eq!(rail.bounds.y, 140);
        assert!(
            (110..=140).contains(&rail.bounds.height),
            "Home-only rail should remain a compact floating glass capsule"
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
    fn approved_reference_geometry_is_frozen_for_1080p() {
        assert_eq!(RAIL_WIDTH, 132);
        assert_eq!(RAIL_LEFT_MARGIN, 56);
        assert_eq!(RAIL_TOP_MARGIN, 140);
        assert_eq!(background::TOP_STRIP_RULE_Y, 59);
        assert_eq!(STATUS_CLUSTER_WIDTH, 480);
        assert_eq!(STATUS_CLUSTER_HEIGHT, 44);
    }

    #[test]
    fn rail_actions_have_user_visible_labels_except_prime_mark() {
        assert_eq!(RailAction::Prime.label(), None);
        assert_eq!(RailAction::Network.label(), Some("NETWORK"));
        assert_eq!(RailAction::Audio.label(), Some("AUDIO"));
        assert_eq!(RailAction::Storage.label(), Some("STORAGE"));
        assert_eq!(RailAction::Health.label(), Some("HEALTH"));
    }

    #[test]
    fn reference_rail_and_top_cluster_use_presentation_glyphs() {
        const { assert!(rail::RAIL_PRIMARY_ICON_SIZE >= 34) };
        let icons = [Icon::Shield, Icon::Wifi, Icon::Battery];
        for icon in icons {
            let mut bytes = vec![0u8; 40 * 40 * 4];
            let mut canvas = Canvas::new(&mut bytes, 40, 40).unwrap();
            draw_icon(
                &mut canvas,
                Rect::new(4, 4, 32, 32),
                icon,
                Argb::from_u32(0xffdbeafe),
            );
            let painted = (0..40)
                .flat_map(|y| (0..40).map(move |x| (x, y)))
                .filter(|&(x, y)| canvas.pixel(x, y).is_some_and(|p| p.a > 20))
                .count();
            assert!(painted > 20, "presentation glyph must render");
        }
    }

    #[test]
    fn approved_prime_mark_renders_identity_geometry() {
        let theme = Theme::prime_dark();
        let mut bytes = vec![0u8; 80 * 80 * 4];
        let mut canvas = Canvas::new(&mut bytes, 80, 80).unwrap();
        rail::draw_prime_mark(&mut canvas, Rect::new(5, 5, 70, 70), &theme, false);
        let painted = (0..80)
            .flat_map(|y| (0..80).map(move |x| (x, y)))
            .filter(|&(x, y)| canvas.pixel(x, y).is_some_and(|p| p.a > 80 && p.b > 100))
            .count();
        assert!(painted > 100);
    }

    #[test]
    fn desktop_chrome_paints_bottom_identity_and_watermark() {
        let theme = Theme::prime_dark();
        let mut text = TextSystem::load_system().expect("Prime production font");
        let mut bytes = vec![0u8; 1920 * 1080 * 4];
        let mut canvas = Canvas::new(&mut bytes, 1920, 1080).unwrap();
        paint_settled_background(&mut canvas, &theme);
        let left_before = (970..1055)
            .flat_map(|y| (55..360).map(move |x| (x, y)))
            .map(|(x, y)| canvas.pixel(x, y).unwrap())
            .collect::<Vec<_>>();
        let right_before = (970..1055)
            .flat_map(|y| (1680..1885).map(move |x| (x, y)))
            .map(|(x, y)| canvas.pixel(x, y).unwrap())
            .collect::<Vec<_>>();
        paint_top_status_strip(&mut canvas, &mut text, &theme, TopStatus::Online);
        let left_changed = (970..1055)
            .flat_map(|y| (55..360).map(move |x| (x, y)))
            .zip(left_before)
            .filter(|((x, y), before)| canvas.pixel(*x, *y).is_some_and(|after| after != *before))
            .count();
        let right_changed = (970..1055)
            .flat_map(|y| (1680..1885).map(move |x| (x, y)))
            .zip(right_before)
            .filter(|((x, y), before)| canvas.pixel(*x, *y).is_some_and(|after| after != *before))
            .count();
        assert!(left_changed > 60, "bottom-left Prime identity must render");
        assert!(
            right_changed > 20,
            "bottom-right Prime watermark must render"
        );
    }

    #[test]
    fn wallpaper_composition_scales_without_a_low_resolution_source_grid() {
        let theme = Theme::prime_dark();
        let mut small_bytes = vec![0u8; 320 * 180 * 4];
        let mut large_bytes = vec![0u8; 640 * 360 * 4];
        let mut small = Canvas::new(&mut small_bytes, 320, 180).unwrap();
        let mut large = Canvas::new(&mut large_bytes, 640, 360).unwrap();
        paint_settled_background(&mut small, &theme);
        paint_settled_background(&mut large, &theme);

        let normalized = [(32, 45), (96, 126), (160, 90), (240, 82), (276, 64)];
        for (x, y) in normalized {
            let a = small.pixel(x, y).unwrap();
            let b = large.pixel(x * 2, y * 2).unwrap();
            let delta = |left: u8, right: u8| (i16::from(left) - i16::from(right)).abs();
            assert!(delta(a.r, b.r) <= 14, "red composition drift at {x},{y}");
            assert!(delta(a.g, b.g) <= 14, "green composition drift at {x},{y}");
            assert!(delta(a.b, b.b) <= 14, "blue composition drift at {x},{y}");
        }
    }

    #[test]
    fn idle_wallpaper_motion_is_visible_but_bounded() {
        let theme = Theme::prime_dark();
        let mut first_bytes = vec![0u8; 480 * 270 * 4];
        let mut second_bytes = vec![0u8; 480 * 270 * 4];
        let mut first = Canvas::new(&mut first_bytes, 480, 270).unwrap();
        let mut second = Canvas::new(&mut second_bytes, 480, 270).unwrap();
        paint_background_base(&mut first, &theme);
        paint_background_motion(&mut first, &theme, 0.0);
        paint_background_base(&mut second, &theme);
        paint_background_motion(&mut second, &theme, 1.4);

        let changed = (0..270)
            .flat_map(|y| (0..480).map(move |x| (x, y)))
            .filter(|(x, y)| first.pixel(*x, *y) != second.pixel(*x, *y))
            .count();
        let total = 480 * 270;
        assert!(
            changed > 300,
            "idle animation must produce visible frame delta"
        );
        assert!(
            changed < total / 4,
            "idle animation must not churn the whole desktop every frame"
        );
    }

    #[test]
    fn approved_wallpaper_has_no_placeholder_geometric_boxes() {
        assert_eq!(background::DECORATIVE_BOX_COUNT, 0);
    }

    #[test]
    fn prime_launcher_matches_approved_first_light_proportions_and_avoids_ready_badges() {
        const {
            assert!(PRIME_LAUNCHER_WIDTH >= 840 && PRIME_LAUNCHER_WIDTH <= 880);
            assert!(PRIME_LAUNCHER_HEIGHT >= 760 && PRIME_LAUNCHER_HEIGHT <= 810);
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
    fn quick_controls_use_reference_three_top_cards_and_two_summary_cards() {
        let layout = QuickControlsLayout::new(600, 902);
        let first = layout.card_rect(0);
        let second = layout.card_rect(1);
        let third = layout.card_rect(2);
        assert!(second.x > first.x);
        assert_eq!(second.y, first.y);
        assert!(third.x > second.x);
        assert_eq!(third.y, first.y);
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
    fn approved_wallpaper_reference_anchors_are_preserved() {
        let mut bytes = vec![0u8; 320 * 180 * 4];
        let mut canvas = Canvas::new(&mut bytes, 320, 180).unwrap();
        paint_settled_background(&mut canvas, &Theme::prime_dark());
        let near = |pixel: Argb, expected: (u8, u8, u8), tolerance: i16| {
            (i16::from(pixel.r) - i16::from(expected.0)).abs() <= tolerance
                && (i16::from(pixel.g) - i16::from(expected.1)).abs() <= tolerance
                && (i16::from(pixel.b) - i16::from(expected.2)).abs() <= tolerance
        };
        assert!(near(canvas.pixel(160, 90).unwrap(), (32, 47, 79), 14));
        assert!(near(canvas.pixel(40, 110).unwrap(), (51, 34, 92), 14));
        assert!(near(canvas.pixel(240, 80).unwrap(), (7, 74, 94), 16));
        assert!(near(canvas.pixel(128, 126).unwrap(), (45, 40, 93), 16));
        assert!(near(canvas.pixel(276, 64).unwrap(), (6, 48, 66), 16));
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
            &mut TextSystem::load_system().expect("Prime production font"),
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
    fn rail_surface_has_clean_single_edge_without_hud_highlight_ticks() {
        let actions = RailConfiguration::default().actions().to_vec();
        let rail = RailLayout::for_output(1920, 1080, actions.len());
        let mut bytes = vec![0u8; rail.bounds.width as usize * rail.bounds.height as usize * 4];
        let mut canvas = Canvas::new(&mut bytes, rail.bounds.width, rail.bounds.height).unwrap();
        paint_rail_surface(
            &mut canvas,
            &mut TextSystem::load_system().expect("Prime production font"),
            &Theme::prime_dark(),
            &actions,
            None,
        );

        let center_x = (rail.bounds.width / 2) as i32;
        assert_eq!(
            canvas.pixel(center_x, 7),
            canvas.pixel(center_x, 8),
            "rail must not carry a separate HUD-style highlight tick inside its top edge"
        );
        assert_eq!(
            canvas.pixel(center_x, 2),
            canvas.pixel(center_x, 3),
            "rail must use one continuous perimeter edge rather than a second inner frame"
        );
    }

    #[test]
    fn prime_launcher_uses_one_clean_perimeter_edge() {
        let mut bytes =
            vec![0u8; PRIME_LAUNCHER_WIDTH as usize * PRIME_LAUNCHER_HEIGHT as usize * 4];
        let mut canvas =
            Canvas::new(&mut bytes, PRIME_LAUNCHER_WIDTH, PRIME_LAUNCHER_HEIGHT).unwrap();
        paint_prime_launcher_surface(
            &mut canvas,
            &mut TextSystem::load_system().expect("Prime production font"),
            &Theme::prime_dark(),
            PrimeLauncherView {
                applications: &[],
                selected: 0,
                query: "",
                message: None,
                progress: 1.0,
            },
        );

        let center_x = (PRIME_LAUNCHER_WIDTH / 2) as i32;
        assert_eq!(
            canvas.pixel(center_x, 2),
            canvas.pixel(center_x, 3),
            "Home panel must not carry a second inner perimeter frame"
        );
    }

    #[test]
    fn top_right_status_cluster_matches_approved_top_chrome_and_is_clickable() {
        let layout = StatusClusterLayout::for_surface(480, 44);
        assert_eq!(layout.bounds, Rect::new(0, 0, 480, 44));
        assert!(layout.hit(450.0, 22.0));
        assert!(!layout.hit(-1.0, 18.0));
    }

    #[test]
    fn status_cluster_painter_keeps_transparent_corners_and_renders_truth() {
        let theme = Theme::prime_dark();
        let mut text = TextSystem::load_system().expect("Prime production font");
        let mut bytes =
            vec![0u8; STATUS_CLUSTER_WIDTH as usize * STATUS_CLUSTER_HEIGHT as usize * 4];
        let mut canvas =
            Canvas::new(&mut bytes, STATUS_CLUSTER_WIDTH, STATUS_CLUSTER_HEIGHT).unwrap();
        paint_status_cluster(&mut canvas, &mut text, &theme, TopStatus::Online);
        assert_eq!(canvas.pixel(0, 0).unwrap(), Argb::TRANSPARENT);
        let bright = (0..44)
            .flat_map(|y| (0..480).map(move |x| (x, y)))
            .filter(|&(x, y)| canvas.pixel(x, y).is_some_and(|p| p.a > 160 && p.g > 150))
            .count();
        assert!(bright > 20);
    }

    #[test]
    fn top_status_truth_labels_are_explicit() {
        assert_eq!(TopStatus::Online.label(), "NOMINAL");
        assert_eq!(TopStatus::Limited.label(), "LIMITED");
    }

    #[test]
    fn prime_launcher_layout_maps_four_column_application_cards() {
        let layout = PrimeLauncherLayout::new(858, 786);
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
        assert!(third.x > second.x);
        assert_eq!(layout.application_at(8.0, 8.0, 3), None);
        assert!(
            layout.apps.height >= 540,
            "Home should dedicate its lower body to applications instead of duplicating power controls"
        );
    }

    #[test]
    fn shell_surfaces_preserve_real_compositor_glass() {
        const {
            assert!(rail::RAIL_GLASS_ALPHA <= 64);
            assert!(prime_launcher::LAUNCHER_GLASS_ALPHA <= 64);
            assert!(quick_controls::QUICK_GLASS_ALPHA <= 64);
        }
    }

    #[test]
    fn quick_controls_power_actions_are_bounded_to_cards() {
        let layout = QuickControlsLayout::new(600, 902);
        assert!(layout.collapse.contains(
            layout.collapse.center_x() as i32,
            layout.collapse.center_y() as i32
        ));
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
    fn approved_top_strip_and_rail_follow_reference_typography() {
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
        paint_rail_surface(
            &mut rail_canvas,
            &mut text,
            &theme,
            &actions,
            Some(RailAction::Prime),
        );
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

    #[test]
    fn system_wallpaper_catalog_exposes_exactly_eight_unique_assets() {
        let catalog = wallpaper::system_wallpapers();
        assert_eq!(catalog.len(), 8);
        let mut ids = catalog.iter().map(|entry| entry.id).collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 8);
        assert!(catalog.iter().all(|entry| !entry.encoded.is_empty()));
    }

    #[test]
    fn every_system_wallpaper_decodes_as_rgb_source_art() {
        for index in 0..8 {
            let decoded =
                wallpaper::decode_system_wallpaper(index).expect("approved wallpaper decodes");
            assert_eq!((decoded.width, decoded.height), (1672, 941));
            assert_eq!(
                decoded.rgba.len(),
                decoded.width as usize * decoded.height as usize * 4
            );
        }
    }

    #[test]
    fn wallpaper_selection_defaults_to_system_03_and_round_trips() {
        assert_eq!(
            wallpaper::WallpaperSelection::from_json("{}"),
            wallpaper::WallpaperSelection::System(2)
        );
        let selected = wallpaper::WallpaperSelection::System(6);
        let encoded = selected.to_json().expect("selection JSON");
        assert_eq!(wallpaper::WallpaperSelection::from_json(&encoded), selected);
        assert_eq!(
            wallpaper::WallpaperSelection::from_json(
                r#"{"schema":"prime.wallpaper.v1","selection":"system-99"}"#
            ),
            wallpaper::WallpaperSelection::System(2)
        );
        assert_eq!(
            wallpaper::WallpaperSelection::default(),
            wallpaper::WallpaperSelection::System(2)
        );
        assert_eq!(
            wallpaper::WallpaperSelection::from_json(
                r#"{"schema":"prime.wallpaper.v1","selection":"animated"}"#
            ),
            wallpaper::WallpaperSelection::Animated
        );
    }

    #[test]
    fn wallpaper_selection_persists_in_user_writable_json() {
        let root = std::env::temp_dir().join(format!(
            "prime-wallpaper-config-test-{}",
            std::process::id()
        ));
        let path = root.join("prime/wallpaper.json");
        let selected = wallpaper::WallpaperSelection::System(3);
        selected.save_to_path(&path).expect("save selection");
        assert_eq!(
            wallpaper::WallpaperSelection::load_from_path(&path),
            selected
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn selected_system_wallpaper_cover_scales_into_output_canvas() {
        let decoded = wallpaper::decode_system_wallpaper(0).expect("approved wallpaper decodes");
        let mut bytes = vec![0u8; 320 * 180 * 4];
        let mut canvas = Canvas::new(&mut bytes, 320, 180).unwrap();
        wallpaper::paint_system_wallpaper(&mut canvas, &decoded);
        assert_eq!(canvas.pixel(0, 0).unwrap().a, 255);
        assert_eq!(canvas.pixel(319, 179).unwrap().a, 255);
        assert_ne!(
            canvas.pixel(32, 90).unwrap(),
            canvas.pixel(288, 90).unwrap()
        );
    }
}
