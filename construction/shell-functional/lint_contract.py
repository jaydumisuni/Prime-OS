from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exact source anchor once, found {count}")
    return text.replace(old, new, 1)


main = Path("crates/prime-shell/src/main.rs")
text = main.read_text()

text = replace_once(text, "const ORB_ARGB: u32 = 0xff252b35;\n", "", "obsolete Orb flat color")
text = replace_once(
    text,
    "const QUICK_CONTROLS_ARGB: u32 = 0xff202630;\n",
    "",
    "obsolete quick-controls flat color",
)
text = replace_once(
    text,
    '''struct TransientSurface {
    layer: LayerSurface,
    width: u32,
    height: u32,
    color: u32,
}''',
    '''struct TransientSurface {
    layer: LayerSurface,
    width: u32,
    height: u32,
}''',
    "transient color field",
)
text = replace_once(
    text,
    "        color: u32,\n    ) -> TransientSurface {",
    "    ) -> TransientSurface {",
    "create-overlay color parameter",
)
text = replace_once(
    text,
    "        TransientSurface {\n            layer,\n            width,\n            height,\n            color,\n        }",
    "        TransientSurface {\n            layer,\n            width,\n            height,\n        }",
    "transient color initialization",
)
text = replace_once(
    text,
    "            ORB_WIDTH,\n            ORB_HEIGHT,\n            ORB_ARGB,\n        ));",
    "            ORB_WIDTH,\n            ORB_HEIGHT,\n        ));",
    "Orb create-overlay color argument",
)
text = replace_once(
    text,
    "            QUICK_CONTROLS_WIDTH,\n            QUICK_CONTROLS_HEIGHT,\n            QUICK_CONTROLS_ARGB,\n        ));",
    "            QUICK_CONTROLS_WIDTH,\n            QUICK_CONTROLS_HEIGHT,\n        ));",
    "quick-controls create-overlay color argument",
)

dead_draw = '''fn draw_surface(
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
text = replace_once(text, dead_draw, "", "obsolete flat draw helper")
main.write_text(text)

visual = Path("crates/prime-shell/src/visual.rs")
text = visual.read_text()
reason = 'reason = "bounded software-raster primitive keeps canvas bounds, geometry and style explicit"'
for function_name in ("fill_rect", "stroke_rect", "draw_text"):
    anchor = f"fn {function_name}(\n"
    replacement = f"#[expect(clippy::too_many_arguments, {reason})]\n{anchor}"
    text = replace_once(text, anchor, replacement, f"{function_name} lint contract")
visual.write_text(text)
