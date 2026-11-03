//! The application's icon where the program itself has to supply it.
//!
//! Inside a macOS app bundle the icon is the bundle's (`make bundle-gui`), and
//! this leaves it alone. Run from `cargo run` there is no bundle, so the Dock
//! would show a generic icon; it is set here instead. Windows and Linux take a
//! window icon as raw pixels.

use tao::window::Icon;

/// 512 pixels square, the rounded variant of the artwork in `icons/`.
const PNG: &[u8] = include_bytes!("../assets/app-icon.png");

/// The icon as pixels for `WindowBuilder::with_window_icon`.
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub fn window_icon() -> Option<Icon> {
    let (rgba, w, h) = decode(PNG)?;
    Icon::from_rgba(rgba, w, h).ok()
}

/// RGBA pixels, width and height of a PNG of any of the usual layouts.
fn decode(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let raw = &buf[..info.buffer_size()];
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => raw.to_vec(),
        png::ColorType::Rgb => raw.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => raw.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Grayscale => raw.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    Some((rgba, info.width, info.height))
}

/// Sets the Dock icon, unless the program is running from an app bundle.
#[cfg(target_os = "macos")]
pub fn set_dock_icon() {
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::{NSBundle, NSData};

    if NSBundle::mainBundle().bundleIdentifier().is_some() {
        return;
    }
    let Some(mtm) = MainThreadMarker::new() else { return };
    let data = NSData::with_bytes(PNG);
    if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
        // SAFETY: on the main thread, with an image that stays alive for the call.
        unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image)) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_is_a_square_of_opaque_corners_and_rounded_ones() {
        let (rgba, w, h) = decode(PNG).expect("the embedded icon decodes to 8-bit RGBA");
        assert_eq!((w, h), (512, 512));
        assert_eq!(rgba.len(), 512 * 512 * 4);
        // The artwork is a rounded square: the very corner is transparent and the middle is not.
        assert_eq!(rgba[3], 0, "the top-left pixel is transparent");
        let middle = (256 * 512 + 256) * 4;
        assert_eq!(rgba[middle + 3], 255, "the middle is opaque");
        assert!(window_icon().is_some());
    }
}
