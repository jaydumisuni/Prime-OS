use prime_contracts::ApplicationEntry;

use super::*;

const OUTPUT_WIDTH: u32 = 1920;
const OUTPUT_HEIGHT: u32 = 1080;

fn composite(
    desktop: &mut Canvas<'_>,
    surface_bytes: &mut [u8],
    width: u32,
    height: u32,
    origin: (i32, i32),
) {
    let surface = Canvas::new(surface_bytes, width, height).unwrap();
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            if let Some(pixel) = surface.pixel(x, y) {
                if pixel.a > 0 {
                    desktop.blend_pixel(origin.0 + x, origin.1 + y, pixel);
                }
            }
        }
    }
}

fn base_scene(active: Option<RailAction>) -> Vec<u8> {
    let theme = Theme::prime_dark();
    let mut text = TextSystem::load_system().expect("Prime production font");
    let mut bytes = vec![0u8; OUTPUT_WIDTH as usize * OUTPUT_HEIGHT as usize * 4];
    let mut desktop = Canvas::new(&mut bytes, OUTPUT_WIDTH, OUTPUT_HEIGHT).unwrap();
    paint_settled_background(&mut desktop, &theme);
    paint_top_status_strip(&mut desktop, &mut text, &theme, TopStatus::Online);

    let mut rail_bytes = vec![0u8; RAIL_WIDTH as usize * RAIL_HEIGHT as usize * 4];
    {
        let mut rail = Canvas::new(&mut rail_bytes, RAIL_WIDTH, RAIL_HEIGHT).unwrap();
        paint_rail_surface(&mut rail, &theme, active);
    }
    composite(
        &mut desktop,
        &mut rail_bytes,
        RAIL_WIDTH,
        RAIL_HEIGHT,
        (RAIL_LEFT_MARGIN, RAIL_TOP_MARGIN),
    );
    bytes
}

fn applications_fixture() -> Vec<ApplicationEntry> {
    serde_json::from_str(
        r#"[
          {
            "application_id":"00000000-0000-0000-0000-000000000001",
            "display_name":"Files",
            "profile_revision":1,
            "profile_digest":"proof-files",
            "execution_backend":"NATIVE",
            "compatibility":{"state":"FUNCTIONAL","evidence_refs":[]},
            "launch_ready":true,
            "limitations":[]
          },
          {
            "application_id":"00000000-0000-0000-0000-000000000002",
            "display_name":"Terminal",
            "profile_revision":1,
            "profile_digest":"proof-terminal",
            "execution_backend":"NATIVE",
            "compatibility":{"state":"FUNCTIONAL","evidence_refs":[]},
            "launch_ready":true,
            "limitations":[]
          },
          {
            "application_id":"00000000-0000-0000-0000-000000000003",
            "display_name":"Diagnostics",
            "profile_revision":1,
            "profile_digest":"proof-diagnostics",
            "execution_backend":"NATIVE",
            "compatibility":{"state":"RECOGNIZED","evidence_refs":[]},
            "launch_ready":false,
            "limitations":["proof fixture"]
          },
          {
            "application_id":"00000000-0000-0000-0000-000000000004",
            "display_name":"Settings",
            "profile_revision":1,
            "profile_digest":"proof-settings",
            "execution_backend":"NATIVE",
            "compatibility":{"state":"FUNCTIONAL","evidence_refs":[]},
            "launch_ready":true,
            "limitations":[]
          }
        ]"#,
    )
    .expect("visual proof application fixture")
}

#[test]
fn production_surfaces_can_be_dumped_for_machine_visual_review() {
    let Ok(directory) = std::env::var("PRIME_VISUAL_PROOF_DIR") else {
        return;
    };
    std::fs::create_dir_all(&directory).unwrap();
    let theme = Theme::prime_dark();

    let baseline = base_scene(None);
    std::fs::write(format!("{directory}/01-baseline.bgra"), baseline).unwrap();

    let mut orb_scene = base_scene(Some(RailAction::Orb));
    {
        let mut desktop = Canvas::new(&mut orb_scene, OUTPUT_WIDTH, OUTPUT_HEIGHT).unwrap();
        let mut orb_bytes = vec![0u8; ORB_WIDTH as usize * ORB_HEIGHT as usize * 4];
        {
            let mut orb = Canvas::new(&mut orb_bytes, ORB_WIDTH, ORB_HEIGHT).unwrap();
            let mut text = TextSystem::load_system().expect("Prime production font");
            paint_orb_surface(
                &mut orb,
                &mut text,
                &theme,
                &applications_fixture(),
                0,
                None,
                1.0,
            );
        }
        composite(
            &mut desktop,
            &mut orb_bytes,
            ORB_WIDTH,
            ORB_HEIGHT,
            (132, 82),
        );
    }
    std::fs::write(format!("{directory}/02-orb.bgra"), orb_scene).unwrap();

    let mut quick_scene = base_scene(Some(RailAction::Status));
    {
        let mut desktop = Canvas::new(&mut quick_scene, OUTPUT_WIDTH, OUTPUT_HEIGHT).unwrap();
        let mut quick_bytes = vec![0u8; QUICK_WIDTH as usize * QUICK_HEIGHT as usize * 4];
        {
            let mut quick = Canvas::new(&mut quick_bytes, QUICK_WIDTH, QUICK_HEIGHT).unwrap();
            let mut text = TextSystem::load_system().expect("Prime production font");
            let lines = vec![
                "NET enp1s0: UP CARRIER".to_owned(),
                "AUDIO PCH: ALC897".to_owned(),
                "PWR AC: ONLINE".to_owned(),
                "STORAGE LOCAL: READY".to_owned(),
                "HEALTH: PROVING".to_owned(),
            ];
            paint_quick_controls_surface(
                &mut quick,
                &mut text,
                &theme,
                QuickControlsView {
                    lines: &lines,
                    power_ready: true,
                    pending_power: None,
                    message: None,
                    progress: 1.0,
                },
            );
        }
        composite(
            &mut desktop,
            &mut quick_bytes,
            QUICK_WIDTH,
            QUICK_HEIGHT,
            (OUTPUT_WIDTH as i32 - QUICK_WIDTH as i32 - 28, 70),
        );
    }
    std::fs::write(format!("{directory}/03-quick-controls.bgra"), quick_scene).unwrap();
}
