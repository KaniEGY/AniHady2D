use crate::{fix_ar, tr, AppLang};
use eframe::egui;

pub struct PatchReleaseNote {
    pub version: &'static str,
    pub title_en: &'static str,
    pub title_ar: &'static str,
    pub features: &'static [(&'static str, &'static str)],
    pub bugfixes: &'static [(&'static str, &'static str)],
}

/// سجل التحديثات: عدّل هذه القائمة مع كل تحديث جديد لـ Stick وسيظهر تلقائياً مرة واحدة للمستخدم
pub fn get_latest_patch_note() -> PatchReleaseNote {
    PatchReleaseNote {
        version: env!("CARGO_PKG_VERSION"),
        title_en: "What's New in Stick",
        title_ar: "إيه الجديد في Stick",
        features: &[
            ("One-click update check with a clear no-update message.",
             "زرار تحديثات أوضح، وبيقولك لما ميبقاش فيه تحديث جديد."),
            ("Verified downloads and automatic restart after installation.",
             "البرنامج بيتأكد إن التحديث سليم، وبعد التثبيت بيعيد التشغيل."),
            ("What's New opens once per installed version in your language.",
             "نافذة إيه الجديد بتظهر مرة واحدة لكل إصدار وباللغة اللي إنت مختارها."),
        ],
        bugfixes: &[
            ("Fixed update version comparison to stop offering the same build repeatedly.",
             "صلحنا مقارنة الإصدارات عشان نفس التحديث ميتعرضش كل شوية."),
            ("Fixed update checks getting stuck when GitHub can't be reached.",
             "صلحنا مشكلة التحديث اللي كان ممكن يفضل واقف على جاري البحث."),
            ("Unsaved projects are protected before restarting to update.",
             "البرنامج بيتأكد إن مشروعك متحفظ قبل ما يقفل ويحدّث نفسه."),
        ],
    }
}

/// نافذة عرض التحديثات مرة واحدة فقط بعد التحديث
pub fn render_whats_new_modal(ctx: &egui::Context, lang: AppLang, installed_version: &str, show_modal: &mut bool, on_acknowledge: &mut bool) {
    if !*show_modal {
        return;
    }

    let note = get_latest_patch_note();
    egui::Window::new(format!("{} v{}", tr(lang, note.title_en, note.title_ar), installed_version))
        .collapsible(false)
        .resizable(false)
        .default_width(480.0)
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
                    egui::RichText::new(format!("✨ Stick v{}", installed_version)).size(19.0).strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.colored_label(
                        egui::Color32::from_rgb(56, 189, 248),
                        tr(lang, "Patch Notes", "ملاحظات التحديث"),
                    );
                });
            });

            ui.separator();
            ui.add_space(4.0);

            egui::ScrollArea::vertical().max_height(310.0).show(ui, |ui| {
                // قسم المميزات الجديدة
                ui.label(
                    egui::RichText::new(tr(lang, "🚀 New Features:", "🚀 المزايا الجديدة:"))
                        .size(15.5)
                        .color(egui::Color32::from_rgb(34, 197, 94))
                        .strong(),
                );
                ui.add_space(4.0);
                for (en, ar) in note.features {
                    ui.horizontal_wrapped(|ui| {
                        ui.colored_label(egui::Color32::from_rgb(34, 197, 94), "•");
                        ui.label(tr(lang, en, ar));
                    });
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(4.0);

                // قسم الإصلاحات والقلتشات
                ui.label(
                    egui::RichText::new(tr(lang, "🛠 Bug Fixes:", "🛠 المشاكل اللي اتصلحت:"))
                        .size(15.5)
                        .color(egui::Color32::from_rgb(249, 115, 22))
                        .strong(),
                );
                ui.add_space(4.0);
                for (en, ar) in note.bugfixes {
                    ui.horizontal_wrapped(|ui| {
                        ui.colored_label(egui::Color32::from_rgb(249, 115, 22), "✔");
                        ui.label(tr(lang, en, ar));
                    });
                }
            });

            ui.add_space(10.0);
            ui.separator();
            ui.vertical_centered(|ui| {
                let btn = egui::Button::new(
                    egui::RichText::new(tr(lang, "Awesome, Let's Create! 🚀", "تمام، يلا نبدأ! 🚀"))
                        .size(15.0)
                        .color(egui::Color32::BLACK)
                        .strong(),
                )
                .min_size(egui::vec2(220.0, 34.0))
                .fill(egui::Color32::from_rgb(255, 215, 25));

                if ui.add(btn).clicked() {
                    *show_modal = false;
                    *on_acknowledge = true;
                }
            });
        });
}
