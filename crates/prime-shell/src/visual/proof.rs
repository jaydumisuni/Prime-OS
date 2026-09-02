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

    let mut status_bytes =
        vec![0u8; STATUS_CLUSTER_WIDTH as usize * STATUS_CLUSTER_HEIGHT as usize * 4];
    {
        let mut status = Canvas::new(
            &mut status_bytes,
            STATUS_CLUSTER_WIDTH,
            STATUS_CLUSTER_HEIGHT,
        )
        .unwrap();
        paint_status_cluster(&mut status, &mut text, &theme, TopStatus::Online);
    }
    composite(
        &mut desktop,
        &mut status_bytes,
        STATUS_CLUSTER_WIDTH,
        STATUS_CLUSTER_HEIGHT,
        (
            OUTPUT_WIDTH as i32 - STATUS_CLUSTER_WIDTH as i32 - STATUS_CLUSTER_RIGHT_MARGIN,
            STATUS_CLUSTER_TOP_MARGIN,
        ),
    );

    let actions = RailConfiguration::default().actions().to_vec();
    let rail_height = rail_height_for_items(actions.len());
    let mut rail_bytes = vec![0u8; RAIL_WIDTH as usize * rail_height as usize * 4];
    {
        let mut rail = Canvas::new(&mut rail_bytes, RAIL_WIDTH, rail_height).unwrap();
        paint_rail_surface(&mut rail, &mut text, &theme, &actions, active);
    }
    composite(
        &mut desktop,
        &mut rail_bytes,
        RAIL_WIDTH,
        rail_height,
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

    let mut prime_scene = base_scene(Some(RailAction::Prime));
    {
        let mut desktop = Canvas::new(&mut prime_scene, OUTPUT_WIDTH, OUTPUT_HEIGHT).unwrap();
        let mut prime_bytes =
            vec![0u8; PRIME_LAUNCHER_WIDTH as usize * PRIME_LAUNCHER_HEIGHT as usize * 4];
        {
            let mut prime_launcher = Canvas::new(
                &mut prime_bytes,
                PRIME_LAUNCHER_WIDTH,
                PRIME_LAUNCHER_HEIGHT,
            )
            .unwrap();
            let mut text = TextSystem::load_system().expect("Prime production font");
            paint_prime_launcher_surface(
                &mut prime_launcher,
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
            &mut prime_bytes,
            PRIME_LAUNCHER_WIDTH,
            PRIME_LAUNCHER_HEIGHT,
            (132, 82),
        );
    }
    std::fs::write(format!("{directory}/02-prime_launcher.bgra"), prime_scene).unwrap();

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
