use crate::{fix_ar, tr, AppLang};
use eframe::egui;

// The release page and first-launch window use the SAME checked-in notes.
// Contributors edit RELEASE_NOTES.md for every feature/fix before merging.
// Notes are embedded into Stick.exe so it also works offline.
const NOTES: &str = include_str!("../../RELEASE_NOTES.md");

#[derive(Clone, Copy, PartialEq)]
enum NoteSection {
    None,
    Features,
    Bugfixes,
}

fn notes_for_language(markdown: &str, lang: AppLang) -> (Vec<String>, Vec<String>) {
    let mut selected = false;
    let mut section = NoteSection::None;
    let mut features = Vec::new();
    let mut bugfixes = Vec::new();

    for raw in markdown.lines() {
        let line = raw.trim();
        if line.starts_with("## ") {
            selected = match lang {
                AppLang::English => line == "## English (US)",
                AppLang::Arabic => line == "## العربية (المصري)",
            };
            section = NoteSection::None;
            continue;
        }
        if !selected { continue; }
        if line.starts_with("### ") {
            section = if line == "### What's new" || line == "### إيه الجديد؟" {
                NoteSection::Features
            } else if line == "### Bugs fixed" || line == "### المشاكل اللي اتصلحت" {
                NoteSection::Bugfixes
            } else {
                NoteSection::None
            };
            continue;
        }
        if let Some(item) = line.strip_prefix("- ") {
            if item.trim().is_empty() { continue; }
            match section {
                NoteSection::Features => features.push(item.trim().to_string()),
                NoteSection::Bugfixes => bugfixes.push(item.trim().to_string()),
                NoteSection::None => {}
            }
        }
    }
    (features, bugfixes)
}

pub fn render_whats_new_modal(
    ctx: &egui::Context,
    lang: AppLang,
    installed_version: &str,
    show_modal: &mut bool,
    on_acknowledge: &mut bool,
) {
    if !*show_modal { return; }

    let (features, bugfixes) = notes_for_language(NOTES, lang);
    let title = tr(lang, "What's New in Stick", "إيه الجديد في Stick");
    egui::Window::new(format!("{} v{}", title, installed_version))
        .collapsible(false)
        .resizable(false)
        .default_width(490.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(egui::Color32::from_rgb(26, 28, 34))
                .stroke(egui::Stroke::new(1.8, egui::Color32::from_rgb(255, 215, 25)))
                .rounding(10.0)
                .inner_margin(16.0),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 215, 25),
                    egui::RichText::new(format!("✨ Stick v{}", installed_version))
                        .size(19.0)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.colored_label(
                        egui::Color32::from_rgb(56, 189, 248),
                        tr(lang, "Release Notes", "تفاصيل التحديث"),
                    );
                });
            });
            ui.separator();

            egui::ScrollArea::vertical().max_height(315.0).show(ui, |ui| {
                ui.label(
                    egui::RichText::new(tr(lang, "🚀 What's New:", "🚀 إيه الجديد؟"))
                        .size(15.5)
                        .color(egui::Color32::from_rgb(34, 197, 94))
                        .strong(),
                );
                if features.is_empty() {
                    ui.label(tr(lang, "No feature notes were provided.", "مفيش تفاصيل إضافية للمميزات في الإصدار ده."));
                }
                for item in &features {
                    ui.horizontal_wrapped(|ui| {
                        ui.colored_label(egui::Color32::from_rgb(34, 197, 94), "•");
                        ui.label(match lang {
                            AppLang::English => item.clone(),
                            AppLang::Arabic => fix_ar(item),
                        });
                    });
                }
                ui.add_space(10.0);
                ui.separator();
                ui.label(
                    egui::RichText::new(tr(lang, "🛠 Bugs Fixed:", "🛠 المشاكل اللي اتصلحت:"))
                        .size(15.5)
                        .color(egui::Color32::from_rgb(249, 115, 22))
                        .strong(),
                );
                if bugfixes.is_empty() {
                    ui.label(tr(lang, "No additional bug fixes were listed.", "مفيش إصلاحات إضافية متسجلة للإصدار ده."));
                }
                for item in &bugfixes {
                    ui.horizontal_wrapped(|ui| {
                        ui.colored_label(egui::Color32::from_rgb(249, 115, 22), "✔");
                        ui.label(match lang {
                            AppLang::English => item.clone(),
                            AppLang::Arabic => fix_ar(item),
                        });
                    });
                }
            });

            ui.add_space(10.0);
            ui.separator();
            ui.vertical_centered(|ui| {
                let label = tr(lang, "Got it, let's create! 🚀", "تمام، يلا نبدأ! 🚀");
                let button = egui::Button::new(
                    egui::RichText::new(label).size(15.0).color(egui::Color32::BLACK).strong(),
                )
                .min_size(egui::vec2(220.0, 34.0))
                .fill(egui::Color32::from_rgb(255, 215, 25));
                if ui.add(button).clicked() {
                    *show_modal = false;
                    *on_acknowledge = true;
                }
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_languages_have_feature_and_fix_sections() {
        for lang in [AppLang::English, AppLang::Arabic] {
            let (features, fixes) = notes_for_language(NOTES, lang);
            assert!(!features.is_empty(), "Missing features in release notes");
            assert!(!fixes.is_empty(), "Missing bug fixes in release notes");
        }
    }

    #[test]
    fn user_language_selects_only_the_matching_notes() {
        let (en, _) = notes_for_language(NOTES, AppLang::English);
        let (ar, _) = notes_for_language(NOTES, AppLang::Arabic);
        assert!(en.iter().any(|s| s.contains("Check for updates")));
        assert!(ar.iter().any(|s| s.contains("تحديث")));
        assert_ne!(en, ar);
    }
}
