//! Settings ▸ Interface: the accent, font, text size and logo rows (see `crate::custom`). A child
//! of the settings panel so it shares its row helpers.

use egui::RichText;
use serde_json::json;

use super::{choices, heading, hint, row};
use crate::LightcraftApp;
use crate::custom::Accent;
use crate::theme::Tokens;
use crate::widgets::register;

/// The section, shown below the theme cards.
pub(super) fn interface_section(app: &mut LightcraftApp, ui: &mut egui::Ui, t: &Tokens) {
    heading(ui, t, crate::i18n::tr("Look"));
    row(ui, t, crate::i18n::tr("Accent"), |ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        // the theme's own accent, without the override
        let themed = crate::theme::current(ui.ctx()).map(|k| Tokens::for_kind(k).accent).unwrap_or(t.accent);
        for (i, accent) in Accent::ALL.into_iter().enumerate() {
            let colour = accent.colour().unwrap_or(themed);
            let (rect, r) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
            let p = ui.painter();
            p.circle_filled(rect.center(), 8.0, colour);
            if accent == Accent::Theme {
                // "the theme's own": a ringed swatch
                p.circle_stroke(rect.center(), 4.0, egui::Stroke::new(1.5, egui::Color32::WHITE));
            }
            if app.ui.settings.custom.accent == accent {
                p.circle_stroke(rect.center(), 10.0, egui::Stroke::new(1.5, t.text));
            }
            register(ui.ctx(), format!("button:settingsAccent-{i}"), rect);
            if r.on_hover_text(crate::i18n::tr(accent.label())).clicked() {
                app.ui.settings.custom.accent = accent;
            }
        }
    });
    row(ui, t, crate::i18n::tr("Font"), |ui| {
        if let Some(path) = font_combo(app, ui) {
            let _ = app.run("app.uiFont", json!({"path": path}));
        }
    });
    row(ui, t, crate::i18n::tr("Text size"), |ui| {
        let sizes: Vec<(u32, String)> = crate::custom::TEXT_SIZES.iter().map(|p| (*p, format!("{p}%"))).collect();
        let options: Vec<(u32, &str)> = sizes.iter().map(|(p, l)| (*p, l.as_str())).collect();
        choices(ui, "settingsTextSize", &options, &mut app.ui.settings.custom.text_size);
    });
    row(ui, t, crate::i18n::tr("Logo"), |ui| {
        match &app.custom.logo {
            Some(tex) => {
                ui.add(egui::Image::new(tex).fit_to_exact_size(egui::vec2(24.0, 24.0)));
            }
            None => {
                ui.label(RichText::new(crate::i18n::tr("LightCraft (built-in)")).color(t.text));
            }
        }
        let services = &app.services.custom;
        let r = ui.add_enabled(services.pick_logo.is_some() && services.load_logo.is_some(), egui::Button::new(crate::i18n::tr("Choose…")));
        register(ui.ctx(), "button:settingsLogo", r.rect);
        if r.clicked() {
            let _ = app.run("app.logo", json!({}));
        }
        if !app.ui.settings.custom.logo.is_empty() {
            let r = ui.button(crate::i18n::tr("Use LightCraft logo"));
            register(ui.ctx(), "button:settingsLogoReset", r.rect);
            if r.clicked() {
                let _ = app.run("app.logo", json!({"path": ""}));
            }
        }
    });
    hint(
        ui,
        t,
        crate::i18n::tr(
            "The font is a .ttf, .otf or .ttc file; characters it lacks fall back to Inter. The logo (PNG, JPEG, WebP…) is the window, taskbar and Dock icon and appears in About; the macOS Dock shows it from the next launch.",
        ),
    );
}

/// The interface font dropdown: Inter, the fonts installed on this system (found in the
/// background the first time it shows), and Other… for any font file. The choice, when made:
/// a path, or `""` for Inter.
fn font_combo(app: &mut LightcraftApp, ui: &mut egui::Ui) -> Option<String> {
    let current = app.ui.settings.custom.ui_font.clone();
    let installed = crate::custom::installed_fonts(ui.ctx());
    let inter = crate::i18n::tr("Inter (built-in)").to_string();
    let selected = if current.is_empty() {
        inter.clone()
    } else {
        installed
            .and_then(|fonts| fonts.iter().find(|f| f.path == current))
            .map(|f| f.name.clone())
            .or_else(|| file_name(&current))
            .unwrap_or_default()
    };
    let filter_id = egui::Id::new("settingsUiFontFilter");
    let mut filter: String = ui.data(|d| d.get_temp(filter_id)).unwrap_or_default();
    let mut out = None;
    let mut pick_other = false;
    let r = egui::ComboBox::from_id_salt("settingsUiFont").width(260.0).height(360.0).selected_text(selected).show_ui(ui, |ui| {
        ui.add(egui::TextEdit::singleline(&mut filter).hint_text(crate::i18n::tr("Search fonts")).desired_width(f32::INFINITY));
        if ui.selectable_label(current.is_empty(), &inter).clicked() {
            out = Some(String::new());
        }
        ui.separator();
        match installed {
            None => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(crate::i18n::tr("Finding installed fonts…"));
                });
            }
            Some(fonts) => {
                let needle = filter.to_lowercase();
                for font in fonts.iter().filter(|f| needle.is_empty() || f.name.to_lowercase().contains(&needle)) {
                    if ui.selectable_label(font.path == current, &font.name).on_hover_text(&font.path).clicked() {
                        out = Some(font.path.clone());
                    }
                }
            }
        }
        if app.services.custom.pick_font.is_some() {
            ui.separator();
            if ui.selectable_label(false, crate::i18n::tr("Other…")).clicked() {
                pick_other = true;
            }
        }
    });
    register(ui.ctx(), "combo:settingsUiFont", r.response.rect);
    ui.data_mut(|d| d.insert_temp(filter_id, filter));
    if pick_other {
        // the dialog runs after the popup closes
        let _ = app.run("app.uiFont", json!({}));
    }
    out.filter(|p| *p != current)
}

/// The file name of `path` (`None` when `path` is empty).
fn file_name(path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    Some(std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string()))
}
