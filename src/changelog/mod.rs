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
        version: "4.1.0",
        title_en: "What's New in Stick v4.1.0",
        title_ar: "ما الجديد في تحديث Stick v4.1.0",
        features: &[
            (
                "Icon-Based Brush Presets: Quick-select buttons with custom icons for G-Pen, Watercolor, Marker, Airbrush, and Pencil.",
                "أزرار الفرش بالأيقونات: اختيار سريع بأيقونة مخصصة لكل فرشة (G-Pen، ألوان مائية، ماركر، بخاخ، قلم رصاص).",
            ),
            (
                "Advanced Layers: Added Layer Lock, Opacity slider, Guide Layers, and Merge Down.",
                "نظام طبقات متطور: إضافة قفل الطبقة، التحكم في الشفافية، الطبقات الإرشادية، والدمج لأسفل.",
            ),
            (
                "AniCraft Timeline Mechanics: Continuous Playhead scrubbing, Loop Range, and Tween arrows.",
                "ميكانيك التايم لاين الجديد: سحب مستمر للمؤشر الأحمر، تكرار المشهد (Loop)، وأسهم التحريك التلقائي.",
            ),
        ],
        bugfixes: &[
            (
                "Fixed 'Snake' stroke lag: Stabilizer now keeps long strokes locked to their true path.",
                "إصلاح ظاهرة 'الثعبان' وانزياح الخطوط الطويلة أثناء الرسم السريع.",
            ),
            (
                "Fixed Watercolor & Marker rendering: Rendered as continuous smooth paths instead of dotted circles.",
                "إصلاح تقطع الدوائر في الألوان المائية والماركر لتصبح خطوطاً متصلة ناعمة.",
            ),
            (
                "Removed tapered tails on G-Pen & Vector Ink Pen for clean, uniform line art.",
                "إزالة الذيل المدبب من بداية ونهاية فرشة التحبير وقلم الفيكتور.",
            ),
        ],
    }
}

/// نافذة عرض التحديثات مرة واحدة فقط بعد التحديث
pub fn render_whats_new_modal(ctx: &egui::Context, lang: AppLang, show_modal: &mut bool, on_acknowledge: &mut bool) {
    if !*show_modal {
        return;
    }

    let note = get_latest_patch_note();
    egui::Window::new(tr(lang, note.title_en, note.title_ar))
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
                    egui::RichText::new(format!("✨ Stick v{}", note.version)).size(19.0).strong(),
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
                    egui::RichText::new(tr(lang, "🚀 New Features:", "🚀 المزايا الجديدة التي تمت إضافتها:"))
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
                    egui::RichText::new(tr(lang, "🛠 Bug Fixes & Glitches Resolved:", "🛠 القلتشات والمشاكل التي تم إصلاحها:"))
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
                    egui::RichText::new(tr(lang, "Awesome, Let's Create! 🚀", "ممتاز، لنبدأ العمل! 🚀"))
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
