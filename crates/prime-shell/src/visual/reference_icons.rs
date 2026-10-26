//! Owner-reference system artwork; clipped from the approved UI mockup.
//! This is separate from third-party XDG app icon artwork.
use super::wallpaper::{decode_png, paint_icon_image, DecodedWallpaper};
use super::{Canvas, Rect};
use std::collections::HashMap;
use std::sync::OnceLock;

fn cache() -> &'static HashMap<&'static str, DecodedWallpaper> {
    static CACHE: OnceLock<HashMap<&'static str, DecodedWallpaper>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let entries: &[(&str, &[u8])] = &[
            (
                "machine",
                include_bytes!("../../assets/reference-icons/machine.png"),
            ),
            (
                "settings",
                include_bytes!("../../assets/reference-icons/settings.png"),
            ),
            (
                "terminal",
                include_bytes!("../../assets/reference-icons/terminal.png"),
            ),
            (
                "store",
                include_bytes!("../../assets/reference-icons/store.png"),
            ),
            (
                "files",
                include_bytes!("../../assets/reference-icons/files.png"),
            ),
            (
                "notes",
                include_bytes!("../../assets/reference-icons/notes.png"),
            ),
            (
                "music",
                include_bytes!("../../assets/reference-icons/music.png"),
            ),
            (
                "gallery",
                include_bytes!("../../assets/reference-icons/gallery.png"),
            ),
            (
                "tools",
                include_bytes!("../../assets/reference-icons/tools.png"),
            ),
            (
                "chat",
                include_bytes!("../../assets/reference-icons/chat.png"),
            ),
            (
                "bin",
                include_bytes!("../../assets/reference-icons/bin.png"),
            ),
            (
                "projects",
                include_bytes!("../../assets/reference-icons/projects.png"),
            ),
            (
                "quick-wifi",
                include_bytes!("../../assets/reference-icons/quick-wifi.png"),
            ),
            (
                "quick-bluetooth",
                include_bytes!("../../assets/reference-icons/quick-bluetooth.png"),
            ),
            (
                "quick-performance",
                include_bytes!("../../assets/reference-icons/quick-performance.png"),
            ),
            (
                "quick-focus",
                include_bytes!("../../assets/reference-icons/quick-focus.png"),
            ),
            (
                "quick-dark",
                include_bytes!("../../assets/reference-icons/quick-dark.png"),
            ),
            (
                "quick-night",
                include_bytes!("../../assets/reference-icons/quick-night.png"),
            ),
        ];
        entries
            .iter()
            .filter_map(|(name, bytes)| decode_png(bytes).ok().map(|image| (*name, image)))
            .collect()
    })
}

pub(crate) fn system_icon_for(name: &str) -> Option<&'static DecodedWallpaper> {
    let kind = match name.to_ascii_lowercase().as_str() {
        "machine" | "computer" => "machine",
        "settings" | "settings manager" => "settings",
        "terminal" | "terminal emulator" => "terminal",
        "store" | "prime store" => "store",
        "files" | "file manager" => "files",
        "notes" => "notes",
        "music" => "music",
        "gallery" => "gallery",
        "tools" => "tools",
        "chat" => "chat",
        "bin" => "bin",
        "projects" => "projects",
        _ => return None,
    };
    cache().get(kind)
}

pub(crate) fn quick_icon_for(name: &str) -> Option<&'static DecodedWallpaper> {
    let kind = match name.to_ascii_lowercase().as_str() {
        "wi-fi" | "wifi" => "quick-wifi",
        "bluetooth" => "quick-bluetooth",
        "performance" => "quick-performance",
        "focus" => "quick-focus",
        "dark mode" => "quick-dark",
        "night light" => "quick-night",
        _ => return None,
    };
    cache().get(kind)
}

pub(crate) fn paint_icon(
    canvas: &mut Canvas<'_>,
    image: &DecodedWallpaper,
    target: Rect,
    opacity: u8,
) {
    paint_icon_image(canvas, image, target, opacity);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_reference_icons_decode_with_expected_dimensions() {
        assert_eq!(cache().len(), 18);
        for (name, image) in cache() {
            assert_eq!(
                (image.width, image.height),
                (74, 74),
                "bad dimensions for {name}"
            );
        }
    }
    #[test]
    fn unknown_app_cannot_be_painted_as_prime_system_icon() {
        assert!(system_icon_for("Firefox").is_none());
        assert!(system_icon_for("Microsoft Edge").is_none());
    }
}
