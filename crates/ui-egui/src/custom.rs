//! Personal additions kept on top of upstream (Settings ▸ Interface): the interface font, the
//! text size, an accent colour over the theme's own and the app's logo.
//!
//! Everything lives here and in `panels/settings/custom.rs`; upstream files only call in through
//! one-line hooks, so the fork rebases onto each release with few conflicts. The hooks:
//! - `lib.rs`: `pub mod custom`, the `custom` fields of `Services` and `LightcraftApp`, and
//!   [`logic`] at the start of `logic_inner`;
//! - `state.rs`: `AppSettings::custom` (flattened, so the keys sit beside upstream's);
//! - `theme.rs`: [`with_ui_font`], [`ts`], [`accented`] and [`scale_text_styles`];
//! - `menus.rs`: the `app.uiFont`, `app.logo` and `app.appearance` commands ([`command`]);
//! - `panels/settings.rs`: the Interface tab section; `panels/dialogs.rs`: the logo in About;
//! - the desktop app (`apps/lightcraft/src/custom.rs`): the file dialogs and the logo decoder.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use egui::{Color32, FontData, FontDefinitions, FontFamily};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::LightcraftApp;
use crate::theme::{FONT_SEMIBOLD, Tokens};

// ------------------------------------------------------------------------------------ Settings

/// The highlight colour (selection, active controls); `Theme` = the theme's own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Accent {
    #[default]
    Theme,
    Blue,
    Violet,
    Amber,
    Teal,
    Pink,
    Green,
}

impl Accent {
    pub const ALL: [Accent; 7] = [Accent::Theme, Accent::Blue, Accent::Violet, Accent::Amber, Accent::Teal, Accent::Pink, Accent::Green];

    pub fn label(self) -> &'static str {
        match self {
            Accent::Theme => "Theme default",
            Accent::Blue => "Blue",
            Accent::Violet => "Violet",
            Accent::Amber => "Amber",
            Accent::Teal => "Teal",
            Accent::Pink => "Pink",
            Accent::Green => "Green",
        }
    }

    /// Its colour (`None` = the theme's own).
    pub fn colour(self) -> Option<Color32> {
        let rgb = |v: u32| Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
        Some(match self {
            Accent::Theme => return None,
            Accent::Blue => rgb(0x0165dd),
            Accent::Violet => rgb(0x7c6cff),
            Accent::Amber => rgb(0xe8871e),
            Accent::Teal => rgb(0x13a89e),
            Accent::Pink => rgb(0xe0457b),
            Accent::Green => rgb(0x3fa34d),
        })
    }
}

/// The settings this module adds, flattened into `AppSettings` (saved in `ui.json`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CustomSettings {
    /// Interface font: a `.ttf` / `.otf` / `.ttc` file ("" = Inter, bundled; `app.uiFont`).
    pub ui_font: String,
    /// The app's logo: an image file used as the window, taskbar and Dock icon and in About
    /// ("" = LightCraft's own; `app.logo`).
    pub logo: String,
    /// Interface text size in percent (100 = as designed; Settings offers [`TEXT_SIZES`]).
    pub text_size: u32,
    pub accent: Accent,
}

impl Default for CustomSettings {
    fn default() -> Self {
        CustomSettings { ui_font: String::new(), logo: String::new(), text_size: 100, accent: Accent::Theme }
    }
}

/// Decode an image file into an app icon (`""` = the app's built-in icon). The host links the
/// image decoders; the UI crate stays codec-free.
pub type LoadIcon = Box<dyn FnMut(&str) -> Result<egui::IconData, String>>;

/// The host services this module uses (`Services::custom`; all desktop only).
#[derive(Default)]
pub struct CustomServices {
    /// Open dialog for a font file (Settings ▸ Interface ▸ Font ▸ Other…).
    pub pick_font: Option<crate::PickFiles>,
    /// Open dialog for a logo image (Settings ▸ Interface ▸ Logo).
    pub pick_logo: Option<crate::PickFiles>,
    /// Decodes the logo.
    pub load_logo: Option<LoadIcon>,
}

/// What this module last applied (`LightcraftApp::custom`).
#[derive(Default)]
pub struct Custom {
    font_applied: Option<String>,
    look_applied: Option<(u32, Accent)>,
    logo_applied: Option<String>,
    /// The user's logo as a texture for About and Settings; `None` = LightCraft's own.
    pub logo: Option<egui::TextureHandle>,
}

// -------------------------------------------------------------------------- Text size, accent

/// Interface text size in percent; every themed text size goes through [`ts`]. Global like the
/// UI language: every context the app builds paints with it.
static TEXT_SCALE: AtomicU32 = AtomicU32::new(100);

/// The accent over the theme's own (an [`Accent`] index into [`Accent::ALL`]).
static ACCENT: AtomicU32 = AtomicU32::new(0);

/// Text sizes offered in Settings (percent).
pub const TEXT_SIZES: [u32; 5] = [80, 90, 100, 110, 120];

/// A text size (points) at the interface text size the user chose.
pub fn ts(size: f32) -> f32 {
    size * TEXT_SCALE.load(Ordering::Relaxed) as f32 / 100.0
}

/// `t` with the user's accent in place of the theme's own.
pub fn accented(t: Tokens) -> Tokens {
    let accent = Accent::ALL.get(ACCENT.load(Ordering::Relaxed) as usize).copied().unwrap_or_default();
    Tokens { accent: accent.colour().unwrap_or(t.accent), ..t }
}

/// Scale the egui text styles `theme::apply_kind` just set to the user's text size.
pub fn scale_text_styles(ctx: &egui::Context) {
    if TEXT_SCALE.load(Ordering::Relaxed) == 100 {
        return;
    }
    ctx.all_styles_mut(|s| {
        for font in s.text_styles.values_mut() {
            font.size = ts(font.size);
        }
    });
}

// ---------------------------------------------------------------------------------------- Font

/// Name of the user's interface font in [`FontDefinitions::font_data`].
const UI_FONT: &str = "user-ui-font";

/// Largest font file accepted for the interface (a CJK collection is ~20-40 MB).
const UI_FONT_MAX_BYTES: u64 = 64 << 20;

/// The interface font (`None` = Inter), added by [`with_ui_font`].
static UI_FONT_DATA: std::sync::Mutex<Option<Arc<FontData>>> = std::sync::Mutex::new(None);

/// Make the font file at `path` the interface font (`""` = back to Inter). The file is read and
/// checked here: egui panics on a font it cannot parse, so a bad file is refused with an error and
/// the current font stays. Takes effect when the fonts are next installed.
pub fn set_ui_font(path: &str) -> Result<(), String> {
    let font = if path.trim().is_empty() { None } else { Some(Arc::new(load_ui_font(path)?)) };
    *UI_FONT_DATA.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = font;
    Ok(())
}

/// Read and check a TrueType / OpenType font (`.ttf`, `.otf`, the first face of a `.ttc`).
pub fn load_ui_font(path: &str) -> Result<FontData, String> {
    use std::io::Read as _;
    let file = std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?;
    let mut bytes = Vec::new();
    file.take(UI_FONT_MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|e| format!("{path}: {e}"))?;
    if bytes.len() as u64 > UI_FONT_MAX_BYTES {
        return Err(format!("{path}: larger than {} MB; not a font this can use", UI_FONT_MAX_BYTES >> 20));
    }
    check_ui_font(&bytes).map_err(|e| format!("{path}: {e}"))?;
    Ok(FontData::from_owned(bytes))
}

/// The checks egui's own parser (skrifa) would otherwise fail with a panic, plus one of ours: the
/// font draws Latin letters, so the interface stays readable.
fn check_ui_font(bytes: &[u8]) -> Result<(), String> {
    use skrifa::MetadataProvider as _;
    let font = skrifa::FontRef::from_index(bytes, 0).map_err(|e| format!("not a TrueType or OpenType font ({e})"))?;
    let charmap = font.charmap();
    if ['a', 'Z', '0'].iter().any(|c| charmap.map(*c).is_none()) {
        return Err("the font has no Latin letters or digits".into());
    }
    if font.outline_glyphs().format().is_none() {
        return Err("the font has no outlines (a bitmap or colour-only font)".into());
    }
    Ok(())
}

/// `fonts` with the interface font (when set) ahead of Inter in the regular and semibold
/// families; Inter and the rest stay as fallbacks for characters it lacks. The font has one
/// weight, so semibold text uses it too.
pub fn with_ui_font(fonts: FontDefinitions) -> FontDefinitions {
    let font = UI_FONT_DATA.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
    with_font(fonts, font.as_ref())
}

fn with_font(mut fonts: FontDefinitions, font: Option<&Arc<FontData>>) -> FontDefinitions {
    let Some(data) = font else { return fonts };
    fonts.font_data.insert(UI_FONT.into(), data.clone());
    for family in [FontFamily::Proportional, FontFamily::Name(FONT_SEMIBOLD.into())] {
        fonts.families.entry(family).or_default().insert(0, UI_FONT.into());
    }
    fonts
}

/// A font installed on the system that can be the interface font (Settings ▸ Interface ▸ Font).
#[derive(Clone, Debug, PartialEq)]
pub struct InstalledFont {
    /// Its full name from the font file ("Avenir Next Medium").
    pub name: String,
    pub path: String,
}

static INSTALLED_FONTS: std::sync::OnceLock<Vec<InstalledFont>> = std::sync::OnceLock::new();
#[cfg(not(target_arch = "wasm32"))]
static INSTALLED_FONTS_SCAN: std::sync::Once = std::sync::Once::new();

/// The fonts installed on this system that the interface can use, by name; `None` while the first
/// call's background scan is still reading them (`ctx` is repainted when it finishes). Scanned
/// once per run: hundreds of files, so never on the UI thread.
pub fn installed_fonts(ctx: &egui::Context) -> Option<&'static [InstalledFont]> {
    if let Some(fonts) = INSTALLED_FONTS.get() {
        return Some(fonts);
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = ctx;
        Some(INSTALLED_FONTS.get_or_init(Vec::new))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        INSTALLED_FONTS_SCAN.call_once(|| {
            let ctx = ctx.clone();
            let spawned = std::thread::Builder::new().name("font-scan".into()).spawn(move || {
                let _ = INSTALLED_FONTS.set(scan_fonts(&system_font_dirs()));
                ctx.request_repaint();
            });
            if spawned.is_err() {
                let _ = INSTALLED_FONTS.set(Vec::new());
            }
        });
        INSTALLED_FONTS.get().map(Vec::as_slice)
    }
}

/// Where the system and the user keep fonts.
#[cfg(not(target_arch = "wasm32"))]
fn system_font_dirs() -> Vec<std::path::PathBuf> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        dirs.extend(["/System/Library/Fonts", "/System/Library/Fonts/Supplemental", "/Library/Fonts"].map(Into::into));
        dirs.extend(home.map(|h| h.join("Library/Fonts")));
    } else if cfg!(target_os = "windows") {
        let windir = std::env::var_os("WINDIR").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\Windows".into());
        dirs.push(windir.join("Fonts"));
        dirs.extend(std::env::var_os("LOCALAPPDATA").map(|d| std::path::PathBuf::from(d).join("Microsoft\\Windows\\Fonts")));
    } else {
        dirs.extend(["/usr/share/fonts", "/usr/local/share/fonts"].map(Into::into));
        dirs.extend(home.into_iter().flat_map(|h| [h.join(".local/share/fonts"), h.join(".fonts")]));
    }
    dirs
}

/// Every usable font file under `dirs` (two levels deep: Linux sorts fonts into folders), named
/// from its own name table, sorted by name, one entry per name.
pub fn scan_fonts(dirs: &[std::path::PathBuf]) -> Vec<InstalledFont> {
    let mut files = Vec::new();
    let mut pending: Vec<(std::path::PathBuf, u8)> = dirs.iter().map(|d| (d.clone(), 0)).collect();
    while let Some((dir, depth)) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if depth < 2 {
                    pending.push((path, depth + 1));
                }
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| ["ttf", "otf", "ttc", "otc"].contains(&e.to_ascii_lowercase().as_str()))
            {
                files.push(path);
            }
        }
    }
    let mut fonts: Vec<InstalledFont> = files
        .into_iter()
        .filter(|p| std::fs::metadata(p).is_ok_and(|m| m.len() <= UI_FONT_MAX_BYTES))
        .filter_map(|p| {
            let bytes = std::fs::read(&p).ok()?;
            check_ui_font(&bytes).ok()?;
            Some(InstalledFont { name: font_name(&bytes)?, path: p.to_string_lossy().to_string() })
        })
        .collect();
    fonts.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.path.cmp(&b.path)));
    fonts.dedup_by(|a, b| a.name == b.name);
    fonts
}

/// The full name in a font's name table ("Avenir Next Medium"); its family name when it has none.
fn font_name(bytes: &[u8]) -> Option<String> {
    use skrifa::MetadataProvider as _;
    let font = skrifa::FontRef::from_index(bytes, 0).ok()?;
    [skrifa::string::StringId::FULL_NAME, skrifa::string::StringId::FAMILY_NAME]
        .into_iter()
        .find_map(|id| font.localized_strings(id).english_or_first().map(|s| s.chars().collect::<String>()))
        // a few old fonts append their version ("Academy Engraved LET Plain:1.0")
        .map(|s| s.split(':').next().unwrap_or_default().trim().to_string())
        .filter(|s| !s.is_empty() && !s.starts_with('.'))
}

// ------------------------------------------------------------------------------- Frame logic

/// Apply the settings that changed since the last frame (all of them on the first). Runs at the
/// start of each frame, before the app installs its fonts and theme for the first time.
pub fn logic(app: &mut LightcraftApp, ctx: &egui::Context) {
    let settings = &app.ui.settings.custom;
    if app.custom.font_applied.as_deref() != Some(settings.ui_font.as_str()) {
        // a font that no longer loads (moved, deleted) falls back to Inter with a notice
        if let Err(e) = set_ui_font(&settings.ui_font) {
            app.notices.push(format!("{}: {e}", crate::i18n::tr("The interface font couldn't be loaded, so Inter is used")));
            app.ui.settings.custom.ui_font.clear();
            let _ = set_ui_font("");
        }
        app.custom.font_applied = Some(app.ui.settings.custom.ui_font.clone());
        if app.styled {
            crate::theme::install_fonts_with_chinese(ctx, app.chinese_font.as_ref());
        }
    }
    let look = (app.ui.settings.custom.text_size, app.ui.settings.custom.accent);
    if app.custom.look_applied != Some(look) {
        app.custom.look_applied = Some(look);
        TEXT_SCALE.store(look.0.clamp(50, 200), Ordering::Relaxed);
        ACCENT.store(Accent::ALL.iter().position(|a| *a == look.1).unwrap_or(0) as u32, Ordering::Relaxed);
        // restyle with the theme in use (none yet on the first frame: the app styles it next)
        if let Some(kind) = crate::theme::current(ctx) {
            crate::theme::apply_kind(ctx, kind);
        }
    }
    apply_logo(app, ctx);
}

/// Apply the logo setting when it changed: the texture About and Settings show, and the window
/// icon (Windows and Linux at once; macOS sets the Dock icon at launch only). A logo that no
/// longer loads falls back to LightCraft's with a notice.
pub fn apply_logo(app: &mut LightcraftApp, ctx: &egui::Context) {
    let path = app.ui.settings.custom.logo.clone();
    let previous = app.custom.logo_applied.replace(path.clone());
    if previous.as_deref() == Some(path.as_str()) {
        return;
    }
    // at launch with the built-in logo the host's own icon is already up
    if previous.is_none() && path.is_empty() {
        return;
    }
    let Some(load) = app.services.custom.load_logo.as_mut() else { return };
    let icon = match load(&path) {
        Ok(icon) => icon,
        Err(e) => {
            app.notices.push(format!("{}: {e}", crate::i18n::tr("The logo couldn't be loaded, so LightCraft's is used")));
            app.ui.settings.custom.logo.clear();
            app.custom.logo_applied = Some(String::new());
            app.custom.logo = None;
            match load("") {
                Ok(icon) => icon,
                Err(_) => return,
            }
        }
    };
    app.custom.logo = (!app.ui.settings.custom.logo.is_empty()).then(|| {
        let image = egui::ColorImage::from_rgba_unmultiplied([icon.width as usize, icon.height as usize], &icon.rgba);
        ctx.load_texture("user-logo", image, egui::TextureOptions::LINEAR)
    });
    ctx.send_viewport_cmd(egui::ViewportCommand::Icon(Some(Arc::new(icon))));
}

// ------------------------------------------------------------------------------------ Commands

/// The commands this module adds, run from `menus::run_ui_command`.
pub fn command(app: &mut LightcraftApp, id: &str, p: &Value) -> Result<Value, String> {
    match id {
        "app.uiFont" => set_ui_font_command(app, p),
        "app.logo" => set_logo(app, p),
        "app.appearance" => set_appearance(app, p),
        _ => Err(format!("unknown command {id}")),
    }
}

/// `path` from the params, or a file chosen with `pick` (`Ok(None)`: the dialog was cancelled).
fn path_or_pick(p: &Value, pick: Option<&mut crate::PickFiles>) -> Result<Option<String>, String> {
    if let Some(path) = p.get("path") {
        return path.as_str().map(|s| Some(s.trim().to_string())).ok_or_else(|| "path must be a string (\"\" = the built-in one)".into());
    }
    let pick = pick.ok_or("no file dialog on this platform; pass {\"path\": …}")?;
    Ok(pick().into_iter().next())
}

/// `app.uiFont {path?}`: make the font file at `path` the interface font (`""` = Inter), or one
/// chosen in a dialog. The file is checked before anything changes; applied on the next frame.
fn set_ui_font_command(app: &mut LightcraftApp, p: &Value) -> Result<Value, String> {
    let Some(path) = path_or_pick(p, app.services.custom.pick_font.as_mut())? else { return Ok(Value::Null) };
    if !path.is_empty() {
        load_ui_font(&path)?;
    }
    app.ui.settings.custom.ui_font = path.clone();
    Ok(json!({"path": path}))
}

/// `app.logo {path?}`: use the image at `path` as the app's logo (`""` = LightCraft's), or one
/// chosen in a dialog. The image is decoded before anything changes; applied on the next frame.
fn set_logo(app: &mut LightcraftApp, p: &Value) -> Result<Value, String> {
    let Some(path) = path_or_pick(p, app.services.custom.pick_logo.as_mut())? else { return Ok(Value::Null) };
    if !path.is_empty() {
        let load = app.services.custom.load_logo.as_mut().ok_or("logos can't be read on this platform")?;
        load(&path)?;
    }
    app.ui.settings.custom.logo = path.clone();
    Ok(json!({"path": path}))
}

/// `app.appearance {accent?, textSize?}`: the accent colour and the text size (Settings ▸
/// Interface); returns both. Applied on the next frame.
fn set_appearance(app: &mut LightcraftApp, p: &Value) -> Result<Value, String> {
    let accent = match p.get("accent") {
        Some(v) => {
            Some(serde_json::from_value::<Accent>(v.clone()).map_err(|_| format!("unknown accent {v} (theme|blue|violet|amber|teal|pink|green)"))?)
        }
        None => None,
    };
    let size = match p.get("textSize") {
        Some(v) => Some(v.as_u64().filter(|n| (50..=200).contains(n)).ok_or("textSize is a percentage from 50 to 200")? as u32),
        None => None,
    };
    let s = &mut app.ui.settings.custom;
    if let Some(accent) = accent {
        s.accent = accent;
    }
    if let Some(size) = size {
        s.text_size = size;
    }
    Ok(json!({"accent": s.accent, "textSize": s.text_size}))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{LightcraftApp, Services};

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("lightcraft-custom-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const INTER: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");

    #[test]
    fn a_font_file_is_checked_before_it_becomes_the_interface_font() {
        let dir = scratch("font");
        let good = dir.join("Inter.ttf");
        std::fs::write(&good, INTER).unwrap();
        let bad = dir.join("broken.ttf");
        std::fs::write(&bad, b"not a font at all").unwrap();
        let mut app = LightcraftApp::new(lightcraft_engine::Session::new(), Services::default());

        let good = good.to_string_lossy().to_string();
        assert_eq!(app.run("app.uiFont", json!({"path": good})).unwrap(), json!({"path": good}));
        assert_eq!(app.ui.settings.custom.ui_font, good);

        // refused, and the font chosen before stays
        let err = app.run("app.uiFont", json!({"path": bad.to_string_lossy()})).unwrap_err();
        assert!(err.contains("not a TrueType or OpenType font"), "{err}");
        assert!(app.run("app.uiFont", json!({"path": dir.join("missing.ttf").to_string_lossy()})).is_err());
        assert_eq!(app.ui.settings.custom.ui_font, good);

        // "" goes back to Inter; no dialog here, so no path is an error rather than a hang
        app.run("app.uiFont", json!({"path": ""})).unwrap();
        assert!(app.ui.settings.custom.ui_font.is_empty());
        assert!(app.run("app.uiFont", json!({})).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_interface_font_comes_first_and_inter_stays_as_fallback() {
        let font = std::sync::Arc::new(egui::FontData::from_static(INTER));
        let fonts = super::with_font(crate::theme::font_definitions(&[]), Some(&font));
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Name(crate::theme::FONT_SEMIBOLD.into())] {
            let names = &fonts.families[&family];
            assert_eq!(names.first().map(String::as_str), Some("user-ui-font"), "{family:?}");
            assert!(names.iter().any(|n| n.starts_with("Inter")), "{family:?}: {names:?}");
        }
        // and egui accepts it
        let ctx = egui::Context::default();
        ctx.set_fonts(fonts);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.label("Aa0");
        });
        out.textures_delta.clear();
    }

    #[test]
    fn installed_fonts_are_named_from_the_font_and_unusable_files_are_left_out() {
        let dir = scratch("scan");
        std::fs::create_dir_all(dir.join("nested")).unwrap();
        std::fs::write(dir.join("nested").join("inter-copy.ttf"), INTER).unwrap();
        std::fs::write(dir.join("Inter.ttf"), INTER).unwrap();
        std::fs::write(dir.join("broken.otf"), b"junk").unwrap();
        std::fs::write(dir.join("notes.txt"), INTER).unwrap();
        let fonts = super::scan_fonts(&[dir.clone(), dir.join("does-not-exist")]);
        // two copies of one font are listed once, by the name inside it
        assert_eq!(fonts.len(), 1, "{fonts:?}");
        assert!(fonts[0].name.starts_with("Inter"), "{fonts:?}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn appearance_sets_accent_and_text_size_and_refuses_nonsense() {
        let mut app = LightcraftApp::new(lightcraft_engine::Session::new(), Services::default());
        let r = app.run("app.appearance", json!({"accent": "teal", "textSize": 90})).unwrap();
        assert_eq!(r, json!({"accent": "teal", "textSize": 90}));
        assert!(app.run("app.appearance", json!({"accent": "neon"})).is_err());
        assert!(app.run("app.appearance", json!({"accent": "pink", "textSize": 5})).is_err());
        let s = &app.ui.settings.custom;
        assert_eq!((s.accent, s.text_size), (super::Accent::Teal, 90), "a refused request changes nothing");
    }

    #[test]
    fn the_settings_keep_their_keys_beside_upstreams() {
        let mut s = crate::state::AppSettings::default();
        s.custom.text_size = 110;
        s.custom.accent = super::Accent::Amber;
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!((v["textSize"].clone(), v["accent"].clone(), v["uiFont"].clone()), (json!(110), json!("amber"), json!("")));
        let back: crate::state::AppSettings = serde_json::from_value(v).unwrap();
        assert_eq!(back, s);
        // settings saved before this module, and by the old Theme picker, still load
        let old: crate::state::AppSettings = serde_json::from_value(json!({"theme": "ember", "gpu": false})).unwrap();
        assert_eq!(old.custom, super::CustomSettings::default());
    }

    #[test]
    fn a_logo_is_decoded_before_it_is_used_and_a_lost_one_falls_back() {
        let icon = |path: &str| -> Result<egui::IconData, String> {
            match path {
                "" => Ok(egui::IconData { rgba: vec![0; 4], width: 1, height: 1 }),
                "logo.png" => Ok(egui::IconData { rgba: vec![255; 16], width: 2, height: 2 }),
                other => Err(format!("{other}: not an image")),
            }
        };
        let services = Services { custom: super::CustomServices { load_logo: Some(Box::new(icon)), ..Default::default() }, ..Default::default() };
        let mut app = LightcraftApp::new(lightcraft_engine::Session::new(), services);
        let ctx = egui::Context::default();

        assert!(app.run("app.logo", json!({"path": "nope.txt"})).is_err());
        assert!(app.ui.settings.custom.logo.is_empty());
        app.run("app.logo", json!({"path": "logo.png"})).unwrap();
        super::apply_logo(&mut app, &ctx);
        assert_eq!(app.custom.logo.as_ref().map(|t| t.size()), Some([2, 2]));

        app.run("app.logo", json!({"path": ""})).unwrap();
        super::apply_logo(&mut app, &ctx);
        assert!(app.custom.logo.is_none());

        // saved settings naming a file that is gone: a notice, and LightCraft's logo
        app.ui.settings.custom.logo = "moved.png".into();
        super::apply_logo(&mut app, &ctx);
        assert!(app.custom.logo.is_none() && app.ui.settings.custom.logo.is_empty());
        assert!(app.notices.iter().any(|n| n.contains("moved.png")), "{:?}", app.notices);
    }
}
