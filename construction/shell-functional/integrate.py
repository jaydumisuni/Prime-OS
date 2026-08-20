from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exact source anchor once, found {count}")
    return text.replace(old, new, 1)


cargo = Path("crates/prime-shell/Cargo.toml")
text = cargo.read_text()
text = replace_once(
    text,
    '[dependencies]\nsmithay-client-toolkit = { workspace = true, features = ["xkbcommon"] }\nwayland-client.workspace = true\n',
    '[dependencies]\nprime-contracts.workspace = true\nserde_json.workspace = true\nsmithay-client-toolkit = { workspace = true, features = ["xkbcommon"] }\nwayland-client.workspace = true\n',
    "prime-shell functional dependencies",
)
cargo.write_text(text)

main = Path("crates/prime-shell/src/main.rs")
text = main.read_text()
if not text.startswith("use std::{error::Error, io, num::NonZeroU32};"):
    raise SystemExit("prime-shell main prelude changed")
text = "mod core_client;\nmod visual;\n\n" + text
text = replace_once(
    text,
    '        println!("Transient mechanics: Orb + quick controls; privileged actions unavailable");',
    '        println!("Functional surfaces: Orb applications + truthful quick controls");',
    "functional help",
)
text = replace_once(
    text,
    '        keyboard_focus: None,\n        exit: false,',
    '        keyboard_focus: None,\n        core: core_client::CoreClient::from_env(),\n        applications: Vec::new(),\n        selected_application: 0,\n        orb_message: None,\n        quick_lines: Vec::new(),\n        exit: false,',
    "functional state initialization",
)
text = replace_once(
    text,
    '    eprintln!("PRIME_SHELL_PRIVILEGED_ACTIONS=unavailable;typed_core_bridge_unearned");',
    '    eprintln!("PRIME_SHELL_CORE_BRIDGE=typed_application_id;quick_controls=truthful_read_only");',
    "functional startup marker",
)
text = replace_once(
    text,
    '    keyboard_focus: Option<ShellSurfaceKind>,\n    exit: bool,',
    '    keyboard_focus: Option<ShellSurfaceKind>,\n    core: core_client::CoreClient,\n    applications: Vec<prime_contracts::ApplicationEntry>,\n    selected_application: usize,\n    orb_message: Option<String>,\n    quick_lines: Vec<String>,\n    exit: bool,',
    "functional state fields",
)

anchor = '''fn draw_surface(
    pool: &mut SlotPool,
    layer: &LayerSurface,
    width: u32,
    height: u32,
    color: u32,
) -> Result<(), Box<dyn Error>> {
    let width = i32::try_from(width)?;
    let height = i32::try_from(height)?;
    let stride = width
        .checked_mul(4)
        .ok_or_else(|| io::Error::other("Prime Shell surface stride overflow"))?;
    let (buffer, canvas) = pool.create_buffer(width, height, stride, wl_shm::Format::Argb8888)?;
    let pixel = color.to_le_bytes();
    for bytes in canvas.chunks_exact_mut(4) {
        bytes.copy_from_slice(&pixel);
    }

    layer.wl_surface().damage_buffer(0, 0, width, height);
    buffer.attach_to(layer.wl_surface())?;
    layer.commit();
    Ok(())
}
'''
replacement = anchor + '''
fn draw_visual_surface<F>(
    pool: &mut SlotPool,
    layer: &LayerSurface,
    width: u32,
    height: u32,
    painter: F,
) -> Result<(), Box<dyn Error>>
where
    F: FnOnce(&mut [u8], u32, u32),
{
    let buffer_width = i32::try_from(width)?;
    let buffer_height = i32::try_from(height)?;
    let stride = buffer_width
        .checked_mul(4)
        .ok_or_else(|| io::Error::other("Prime Shell visual stride overflow"))?;
    let (buffer, canvas) = pool.create_buffer(
        buffer_width,
        buffer_height,
        stride,
        wl_shm::Format::Argb8888,
    )?;
    painter(canvas, width, height);
    layer
        .wl_surface()
        .damage_buffer(0, 0, buffer_width, buffer_height);
    buffer.attach_to(layer.wl_surface())?;
    layer.commit();
    Ok(())
}
'''
text = replace_once(text, anchor, replacement, "visual drawing helper")

text = replace_once(
    text,
    '''        self.orb = Some(self.create_overlay(
            queue_handle,
            ORB_NAMESPACE,
            Anchor::BOTTOM,
            ORB_WIDTH,
            ORB_HEIGHT,
            ORB_ARGB,
        ));''',
    '''        match self.core.applications() {
            Ok(projection) => {
                self.applications = projection.applications;
                self.selected_application = self
                    .selected_application
                    .min(self.applications.len().saturating_sub(1));
                self.orb_message = if self.applications.is_empty() {
                    Some("NO ADMITTED APPLICATION PROFILES".to_owned())
                } else {
                    None
                };
            }
            Err(error) => {
                self.applications.clear();
                self.selected_application = 0;
                self.orb_message = Some(format!("CORE UNAVAILABLE: {error}"));
            }
        }
        self.orb = Some(self.create_overlay(
            queue_handle,
            ORB_NAMESPACE,
            Anchor::BOTTOM,
            ORB_WIDTH,
            ORB_HEIGHT,
            ORB_ARGB,
        ));''',
    "Orb application projection",
)
text = replace_once(
    text,
    '''        self.quick_controls = Some(self.create_overlay(
            queue_handle,
            QUICK_CONTROLS_NAMESPACE,
            Anchor::TOP | Anchor::RIGHT,
            QUICK_CONTROLS_WIDTH,
            QUICK_CONTROLS_HEIGHT,
            QUICK_CONTROLS_ARGB,
        ));''',
    '''        self.quick_lines = self.core.system_status_lines().unwrap_or_else(|error| {
            vec![format!("CORE STATUS UNAVAILABLE: {error}")]
        });
        self.quick_controls = Some(self.create_overlay(
            queue_handle,
            QUICK_CONTROLS_NAMESPACE,
            Anchor::TOP | Anchor::RIGHT,
            QUICK_CONTROLS_WIDTH,
            QUICK_CONTROLS_HEIGHT,
            QUICK_CONTROLS_ARGB,
        ));''',
    "quick controls system status",
)

marker = '''    fn close_transient(&mut self, kind: ShellSurfaceKind, source: InteractionSource) {'''
methods = '''    fn activate_selected_application(&mut self) {
        let Some(entry) = self.applications.get(self.selected_application).cloned() else {
            self.orb_message = Some("NO APPLICATION SELECTED".to_owned());
            return;
        };
        if !entry.launch_ready {
            self.orb_message = Some(if entry.limitations.is_empty() {
                "APPLICATION IS NOT LAUNCH READY".to_owned()
            } else {
                entry.limitations.join(" | ")
            });
            return;
        }
        self.orb_message = Some(match self.core.launch(entry.application_id) {
            Ok(()) => format!("LAUNCH ACCEPTED: {}", entry.display_name),
            Err(error) => format!("LAUNCH DENIED: {error}"),
        });
    }

    fn move_application_selection(&mut self, delta: isize) {
        if self.applications.is_empty() {
            self.selected_application = 0;
            return;
        }
        let last = self.applications.len() - 1;
        self.selected_application = if delta < 0 {
            self.selected_application.saturating_sub(delta.unsigned_abs())
        } else {
            self.selected_application.saturating_add(delta as usize).min(last)
        };
    }

    fn redraw_orb(&mut self) {
        let Some(orb) = self.orb.as_ref() else {
            return;
        };
        let layer = orb.layer.clone();
        let width = orb.width;
        let height = orb.height;
        if let Err(error) = draw_visual_surface(&mut self.pool, &layer, width, height, |canvas, w, h| {
            visual::paint_orb(
                canvas,
                w,
                h,
                &self.applications,
                self.selected_application,
                self.orb_message.as_deref(),
            );
        }) {
            eprintln!("prime-shell could not redraw Orb: {error}");
            self.exit = true;
        }
    }

'''
text = replace_once(text, marker, methods + marker, "functional Orb methods")

old_orb_draw = '''                if let Err(error) =
                    draw_surface(&mut self.pool, &orb.layer, width, height, orb.color)
                {
                    eprintln!("prime-shell could not draw Orb overlay: {error}");
                    self.exit = true;
                }'''
new_orb_draw = '''                if let Err(error) = draw_visual_surface(
                    &mut self.pool,
                    &orb.layer,
                    width,
                    height,
                    |canvas, w, h| {
                        visual::paint_orb(
                            canvas,
                            w,
                            h,
                            &self.applications,
                            self.selected_application,
                            self.orb_message.as_deref(),
                        );
                    },
                ) {
                    eprintln!("prime-shell could not draw Orb overlay: {error}");
                    self.exit = true;
                }'''
text = replace_once(text, old_orb_draw, new_orb_draw, "Orb visual")
old_quick_draw = '''                if let Err(error) = draw_surface(
                    &mut self.pool,
                    &quick_controls.layer,
                    width,
                    height,
                    quick_controls.color,
                ) {
                    eprintln!("prime-shell could not draw quick-controls overlay: {error}");
                    self.exit = true;
                }'''
new_quick_draw = '''                if let Err(error) = draw_visual_surface(
                    &mut self.pool,
                    &quick_controls.layer,
                    width,
                    height,
                    |canvas, w, h| visual::paint_quick_controls(canvas, w, h, &self.quick_lines),
                ) {
                    eprintln!("prime-shell could not draw quick-controls overlay: {error}");
                    self.exit = true;
                }'''
text = replace_once(text, old_quick_draw, new_quick_draw, "quick controls visual")

old_background = '''            if let Err(error) = draw_surface(&mut self.pool, layer, width, height, BACKGROUND_ARGB)
            {
                eprintln!("prime-shell could not draw background: {error}");'''
new_background = '''            if let Err(error) = draw_visual_surface(&mut self.pool, layer, width, height, |canvas, w, h| {
                visual::paint_background(canvas, w, h, BACKGROUND_ARGB);
            }) {
                eprintln!("prime-shell could not draw background: {error}");'''
text = replace_once(text, old_background, new_background, "background visual")
old_rail = '''            if let Err(error) = draw_surface(&mut self.pool, layer, width, height, RAIL_ARGB) {
                eprintln!("prime-shell could not draw rail: {error}");'''
new_rail = '''            if let Err(error) = draw_visual_surface(&mut self.pool, layer, width, height, |canvas, w, h| {
                visual::paint_rail(canvas, w, h, RAIL_ARGB);
            }) {
                eprintln!("prime-shell could not draw rail: {error}");'''
text = replace_once(text, old_rail, new_rail, "rail visual")

old_key_tail = '''        let Some(character) = event.keysym.key_char() else {
            return;
        };
        if self.keyboard_focus == Some(ShellSurfaceKind::Rail) {
            match character.to_ascii_lowercase() {
                'o' => self.toggle_orb(queue_handle, InteractionSource::Keyboard),
                'q' => self.toggle_quick_controls(queue_handle, InteractionSource::Keyboard),
                _ => {}
            }
        } else if self.keyboard_focus == Some(ShellSurfaceKind::Orb) && character == '\\r' {
            eprintln!("PRIME_SHELL_ORB_ACTIVATE=unavailable;prime_exec_bridge_unearned");
        }'''
new_key_tail = '''        if self.keyboard_focus == Some(ShellSurfaceKind::Orb) {
            if event.keysym == Keysym::Up {
                self.move_application_selection(-1);
                self.redraw_orb();
                return;
            }
            if event.keysym == Keysym::Down {
                self.move_application_selection(1);
                self.redraw_orb();
                return;
            }
            if event.keysym == Keysym::Return {
                self.activate_selected_application();
                self.redraw_orb();
                return;
            }
        }

        let Some(character) = event.keysym.key_char() else {
            return;
        };
        if self.keyboard_focus == Some(ShellSurfaceKind::Rail) {
            match character.to_ascii_lowercase() {
                'o' => self.toggle_orb(queue_handle, InteractionSource::Keyboard),
                'q' => self.toggle_quick_controls(queue_handle, InteractionSource::Keyboard),
                _ => {}
            }
        }'''
text = replace_once(text, old_key_tail, new_key_tail, "Orb keyboard navigation")

old_pointer = '''            if &event.surface == self.rail.wl_surface() {
                if event.position.0 <= RAIL_TRIGGER_WIDTH {
                    self.toggle_orb(queue_handle, InteractionSource::Pointer);
                } else if self.rail_width > 0
                    && event.position.0 >= f64::from(self.rail_width) - RAIL_TRIGGER_WIDTH
                {
                    self.toggle_quick_controls(queue_handle, InteractionSource::Pointer);
                }
            }'''
new_pointer = '''            if &event.surface == self.rail.wl_surface() {
                if event.position.0 <= RAIL_TRIGGER_WIDTH {
                    self.toggle_orb(queue_handle, InteractionSource::Pointer);
                } else if self.rail_width > 0
                    && event.position.0 >= f64::from(self.rail_width) - RAIL_TRIGGER_WIDTH
                {
                    self.toggle_quick_controls(queue_handle, InteractionSource::Pointer);
                }
            } else if self
                .orb
                .as_ref()
                .is_some_and(|orb| &event.surface == orb.layer.wl_surface())
            {
                if let Some(index) = visual::orb_row_at(event.position.1, self.applications.len()) {
                    self.selected_application = index;
                    self.activate_selected_application();
                    self.redraw_orb();
                }
            }'''
text = replace_once(text, old_pointer, new_pointer, "Orb pointer activation")

main.write_text(text)
