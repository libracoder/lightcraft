//! The desktop side of the personal additions (`lightcraft_ui_egui::custom`): the font and logo
//! file dialogs, and the logo decoder that turns the chosen image into the app icon.

use crate::dialog_filter::FileDialogExt as _;
use lightcraft_ui_egui::custom::CustomServices;

/// Largest logo file read (Settings ▸ Interface ▸ Logo).
const LOGO_MAX_BYTES: u64 = 64 << 20;

pub fn services() -> CustomServices {
    CustomServices {
        pick_font: Some(Box::new(|| {
            crate::file_dialog()
                .set_title(lightcraft_ui_egui::i18n::tr("Interface Font…"))
                .add_filter_nocase(lightcraft_ui_egui::i18n::tr("Font"), &["ttf", "otf", "ttc", "otc"])
                .pick_file()
                .map(|p| vec![p.to_string_lossy().to_string()])
                .unwrap_or_default()
        })),
        pick_logo: Some(Box::new(|| {
            crate::file_dialog()
                .set_title(lightcraft_ui_egui::i18n::tr("Logo…"))
                .add_filter_nocase(lightcraft_ui_egui::i18n::tr("Logo"), &["png", "jpg", "jpeg", "webp", "tif", "tiff"])
                .pick_file()
                .map(|p| vec![p.to_string_lossy().to_string()])
                .unwrap_or_default()
        })),
        load_logo: Some(Box::new(logo_icon)),
    }
}

/// The default logo (Settings ▸ Interface ▸ Logo ▸ Aperture), bundled.
const DEFAULT_LOGO: &[u8] = include_bytes!("../../../assets/custom-logos/default.png");

/// The window icon at launch: the logo chosen in Settings (the default when there are no
/// settings). The macOS Dock takes its icon only here; one that no longer loads is reported, and
/// reset, by the app's first frame.
pub fn startup_icon(prefs: Option<&lightcraft_ui_egui::UiState>) -> egui::IconData {
    let logo = prefs.map(|u| u.settings.custom.logo.as_str()).unwrap_or_default();
    logo_icon(logo).or_else(|_| logo_icon("")).unwrap_or_else(|_| crate::app_icon())
}

/// The logo `path` names as the app icon: one of `custom::LOGOS` or an image file.
fn logo_icon(path: &str) -> Result<egui::IconData, String> {
    use std::io::Read as _;
    match path.trim() {
        "" => return fit_icon(DEFAULT_LOGO, "the default logo"),
        lightcraft_ui_egui::custom::LIGHTCRAFT_LOGO => return Ok(crate::app_icon()),
        _ => {}
    }
    let file = std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?;
    let mut bytes = Vec::new();
    file.take(LOGO_MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|e| format!("{path}: {e}"))?;
    if bytes.len() as u64 > LOGO_MAX_BYTES {
        return Err(format!("{path}: larger than {} MB", LOGO_MAX_BYTES >> 20));
    }
    fit_icon(&bytes, path)
}

/// Decode an image as the app icon, scaled to fit 512 px. On macOS it is placed on Apple's icon
/// grid (an 824/1024 body with a transparent margin), as the built-in icon is, so it sits right
/// among the other Dock icons. `name` is for errors.
fn fit_icon(bytes: &[u8], name: &str) -> Result<egui::IconData, String> {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    limits.max_alloc = Some(1 << 30);
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().map_err(|e| format!("{name}: {e}"))?;
    reader.limits(limits);
    let img = reader.decode().map_err(|e| format!("{name}: not an image this can read ({e})"))?;
    if img.width() == 0 || img.height() == 0 {
        return Err(format!("{name}: the image is empty"));
    }
    const SIDE: u32 = 512;
    let body = if cfg!(target_os = "macos") { SIDE * 824 / 1024 } else { SIDE };
    let fitted = img.resize(body, body, image::imageops::FilterType::Lanczos3).to_rgba8();
    let mut canvas = image::RgbaImage::new(SIDE, SIDE);
    let x = i64::from(SIDE.saturating_sub(fitted.width()) / 2);
    let y = i64::from(SIDE.saturating_sub(fitted.height()) / 2);
    image::imageops::overlay(&mut canvas, &fitted, x, y);
    Ok(egui::IconData { rgba: canvas.into_raw(), width: SIDE, height: SIDE })
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_bundled_logo_decodes_to_a_512_px_icon() {
        for (id, name) in lightcraft_ui_egui::custom::LOGOS {
            let icon = super::logo_icon(id).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(icon.width >= 256 && icon.width == icon.height, "{name}: {}x{}", icon.width, icon.height);
            assert_eq!(icon.rgba.len(), (icon.width * icon.height * 4) as usize, "{name}");
        }
        assert!(super::logo_icon("/no/such/logo.png").is_err());
    }
}
