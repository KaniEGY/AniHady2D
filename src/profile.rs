use serde::{Deserialize, Serialize};

/// Hunter-style progress is purely cosmetic and never changes project files.
#[derive(Serialize, Deserialize, Clone)]
pub struct UserProfile {
    pub username: String,
    pub avatar_path: Option<String>,
    pub total_seconds_spent: u64,
    pub total_frames_drawn: u64,
    pub xp: u64,
    #[serde(default)]
    pub total_painted_layers: u64,
    #[serde(default)]
    pub cloud_account_id: Option<String>,
    #[serde(default)]
    pub sync_token: Option<String>,
}

impl Default for UserProfile {
    fn default() -> Self {
        Self {
            username: "Shadow Artist".to_string(),
            avatar_path: None,
            total_seconds_spent: 0,
            total_frames_drawn: 0,
            total_painted_layers: 0,
            xp: 0,
            cloud_account_id: None,
            sync_token: None,
        }
    }
}

/// Level 0 -> 1 requires 100 XP, 1 -> 2 requires 150 XP,
/// and each successive requirement grows by 15%, rounded up.
pub fn xp_required_for_level(level: u32) -> u64 {
    if level == 0 { return 100; }
    let mut xp = 150u64;
    for _ in 1..level {
        let next = xp.saturating_mul(115).saturating_add(99) / 100;
        if next == xp { return xp; } // u64 saturation guard
        xp = next;
    }
    xp.max(150)
}

pub fn rank_for_level(level: u32) -> &'static str {
    match level {
        0..=5 => "F",
        6..=10 => "E",
        11..=15 => "D",
        16..=20 => "C",
        21..=25 => "B",
        26..=30 => "A",
        31..=35 => "A+",
        _ => "S",
    }
}

// 40 openings x 25 endings = 1,000 unique bilingual motivational lines.
// Every combination is generated locally; no extra sound/image downloads.
const EN_OPENINGS: [&str; 40] = [
    "Great work!", "You're on a roll!", "Another level cleared!",
    "Your skills are growing!", "What a comeback!", "Nice progress!",
    "Keep that momentum!", "Your next scene is waiting!", "You're improving!",
    "That's a clean move!", "Look at those frames!", "Your ideas matter!",
    "Your focus paid off!", "The next level is yours!", "A new milestone!",
    "That was sharp!", "You earned this!", "Progress unlocked!",
    "Your timing is getting better!", "You're building something great!",
    "Keep drawing!", "You nailed that!", "Your art is taking shape!",
    "Every detail counts!", "You're getting stronger!", "That's your style!",
    "Your storyboard is coming alive!", "Good instincts!",
    "The hard work shows!", "You're one step closer!",
    "Great timing!", "That takes skill!", "You've got creative energy!",
    "Your animation is leveling up!", "That's a strong finish!",
    "Keep exploring!", "Another win for your art!", "Your practice pays off!",
    "That deserves a celebration!", "New skills unlocked!"
];
const AR_OPENINGS: [&str; 40] = [
    "عاش يا فنان!", "إنت ماشي جامد!", "مستوى جديد يا بطل!",
    "مهارتك بتزيد!", "رجعة قوية!", "تقدم ممتاز!",
    "كمّل بنفس الحماس!", "المشهد الجاي مستنيك!", "مستواك بيتحسن!",
    "الحركة دي مظبوطة!", "بص على الفريمات دي!", "أفكارك ليها قيمة!",
    "تركيزك جاب نتيجة!", "المستوى الجاي ليك!", "إنجاز جديد!",
    "دي حركة شاطرة!", "إنت تستاهل!", "تقدم جديد اتفتح!",
    "توقيتك بقى أحسن!", "إنت بتعمل حاجة مميزة!",
    "كمّل رسم!", "ظبطتها صح!", "رسوماتك بدأت تظهر!",
    "كل تفصيلة بتفرق!", "إنت بتقوى!", "ده ستايلك!",
    "الأنيميشن بتاعك بيدب فيه الحياة!", "اختيار موفق!",
    "تعبك باين!", "قربت أكتر!",
    "توقيت ممتاز!", "شغل محتاج مهارة!", "إبداعك عالي!",
    "الأنيميشن بيتطور معاك!", "قفلة قوية!",
    "جرّب أفكار جديدة!", "إنجاز جديد لرسمك!", "تمرينك جاب نتيجة!",
    "تستاهل احتفال!", "مهارات جديدة اتفتحت!"
];
const EN_ENDINGS: [&str; 25] = [
    "Keep going!", "Stay creative!", "Try another idea!",
    "One frame at a time.", "You've got this!", "Make something new!",
    "Keep your own style.", "That effort counts.", "On to the next shot!",
    "Your best work is ahead.", "Enjoy the process.", "Practice makes progress.",
    "There's more to explore.", "Trust your eye.", "Keep experimenting.",
    "Take your time and create.", "The canvas is yours.", "Make every frame count.",
    "New skills take practice.", "Your creativity leads.", "Build your own world.",
    "Go for the next challenge.", "Make it move.", "Stay curious.",
    "Turn that idea into a scene."
];
const AR_ENDINGS: [&str; 25] = [
    "كمّل!", "خليك مبدع!", "جرّب فكرة جديدة!",
    "فريم ورا فريم.", "إنت قدها!", "اعمل حاجة جديدة!",
    "خليك على ستايلك.", "تعبك مش بيروح.", "يلا على اللقطة الجاية!",
    "أحلى شغل لسه جاي.", "استمتع بالرسم.", "التمرين بيجيب نتيجة.",
    "لسه فيه كتير تتعلمه.", "ثق في عينك.", "كمّل تجاربك.",
    "خد راحتك وابدع.", "اللوحة لوحتك.", "خلي كل فريم ليه قيمة.",
    "المهارة محتاجة تدريب.", "إبداعك هو الأساس.", "ابني عالمك بنفسك.",
    "شد حيلك في التحدي الجاي.", "خلّي الرسمة تتحرك.", "خليك فضولي.",
    "حوّل الفكرة لمشهد."
];

pub fn level_up_message(variant: usize, is_arabic: bool) -> String {
    let idx = variant % 1000;
    let first = idx / 25;
    let second = idx % 25;
    if is_arabic {
        format!("{} {}", AR_OPENINGS[first], AR_ENDINGS[second])
    } else {
        format!("{} {}", EN_OPENINGS[first], EN_ENDINGS[second])
    }
}

/// 0..99 variants select 10 distinct pitch bands x 10 melodic/rhythmic patterns.
pub fn level_up_sound_variant(level: u32, xp: u64, seed: u64) -> u8 {
    (seed.wrapping_add(xp).wrapping_add(level as u64 * 131) % 100) as u8
}

impl UserProfile {
    pub fn load() -> Self {
        if let Ok(exe) = std::env::current_exe() {
            if let Ok(data) = std::fs::read_to_string(exe.with_file_name("user_profile.json")) {
                if let Ok(parsed) = serde_json::from_str(&data) { return parsed; }
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        if let Ok(exe) = std::env::current_exe() {
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(exe.with_file_name("user_profile.json"), json);
            }
        }
    }

    pub fn get_level_progress(&self) -> (u32, u64, u64) {
        let mut level: u32 = 0;
        let mut remaining = self.xp;
        loop {
            let required = xp_required_for_level(level);
            if remaining < required || level == u32::MAX {
                return (level, remaining, required);
            }
            remaining -= required;
            level += 1;
        }
    }

    pub fn get_hunter_rank(&self) -> &'static str {
        rank_for_level(self.get_level_progress().0)
    }

    pub fn add_xp_check_levelup(&mut self, amount: u64) -> bool {
        if amount == 0 { return false; }
        let old = self.get_level_progress().0;
        self.xp = self.xp.saturating_add(amount);
        let new = self.get_level_progress().0;
        self.save();
        new > old
    }

    pub fn format_playtime(&self, is_arabic: bool) -> String {
        let hours = self.total_seconds_spent / 3600;
        let minutes = (self.total_seconds_spent % 3600) / 60;
        if is_arabic {
            format!("{} ساعة و {} دقيقة", hours, minutes)
        } else {
            format!("{}h {}m", hours, minutes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_match_requested_growth() {
        assert_eq!(xp_required_for_level(0), 100);
        assert_eq!(xp_required_for_level(1), 150);
        assert_eq!(xp_required_for_level(2), 173);
        assert_eq!(xp_required_for_level(3), 199);
        for i in 2..35 {
            assert!(xp_required_for_level(i) > xp_required_for_level(i - 1));
        }
    }

    #[test]
    fn ranks_match_every_requested_boundary() {
        for (level, rank) in [(0,"F"), (5,"F"), (6,"E"), (10,"E"), (11,"D"),
            (15,"D"), (16,"C"), (20,"C"), (21,"B"), (25,"B"), (26,"A"),
            (30,"A"), (31,"A+"), (35,"A+"), (36,"S"), (40,"S")] {
            assert_eq!(rank_for_level(level), rank, "level {level}");
        }
    }

    #[test]
    fn starts_at_zero_and_needs_100_xp_for_first_level() {
        let mut profile = UserProfile::default();
        assert_eq!(profile.get_level_progress(), (0,0,100));
        profile.xp = 99;
        assert_eq!(profile.get_level_progress().0, 0);
        profile.xp = 100;
        assert_eq!(profile.get_level_progress(), (1,0,150));
        profile.xp = 250;
        assert_eq!(profile.get_level_progress(), (2,0,173));
    }

    #[test]
    fn exactly_one_thousand_distinct_messages_in_each_language() {
        use std::collections::HashSet;
        for arabic in [false, true] {
            let all: HashSet<_> = (0..1000).map(|i| level_up_message(i, arabic)).collect();
            assert_eq!(all.len(),1000);
        }
    }

    #[test]
    fn old_saved_profiles_migrate_without_dropping_xp() {
        let profile: UserProfile = serde_json::from_str(
            r#"{"username":"Artist","avatar_path":null,"total_seconds_spent":12,"total_frames_drawn":20,"xp":235}"#
        ).unwrap();
        assert_eq!(profile.xp,235);
        assert_eq!(profile.total_painted_layers,0);
    }

    #[test]
    fn sound_pool_has_one_hundred_variants() {
        let all: std::collections::HashSet<_> = (0..100).map(
            |i| level_up_sound_variant(i, 100, 0)
        ).collect();
        assert_eq!(all.len(),100);
    }
}
