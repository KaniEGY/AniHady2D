use serde::{Deserialize, Serialize};

/// هيكل بيانات ملف اللاعب (مجهز للتوسع المستقبلي للحسابات والمزامنة السحابية)
#[derive(Serialize, Deserialize, Clone)]
pub struct UserProfile {
    pub username: String,
    pub avatar_path: Option<String>,
    pub total_seconds_spent: u64,
    pub total_frames_drawn: u64,
    pub xp: u64,
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
            xp: 0,
            cloud_account_id: None,
            sync_token: None,
        }
    }
}

impl UserProfile {
    pub fn load() -> Self {
        if let Ok(exe) = std::env::current_exe() {
            let path = exe.with_file_name("user_profile.json");
            if let Ok(data) = std::fs::read_to_string(path) {
                if let Ok(parsed) = serde_json::from_str(&data) {
                    return parsed;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        if let Ok(exe) = std::env::current_exe() {
            let path = exe.with_file_name("user_profile.json");
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    /// حساب المستوى الحالي والـ XP المطلوب (100 للمستوى 1، 150 للمستوى 2، 200 للمستوى 3...)
    pub fn get_level_progress(&self) -> (u32, u64, u64) {
        let mut level: u32 = 0;
        let mut remaining_xp = self.xp;
        loop {
            let required = 100 + (level as u64) * 50;
            if remaining_xp >= required {
                remaining_xp -= required;
                level += 1;
            } else {
                return (level, remaining_xp, required);
            }
        }
    }

    /// رتبة الصياد/الرسام حسب المستوى (E -> D -> C -> B -> A -> S -> National Level)
    pub fn get_hunter_rank(&self) -> &'static str {
        let (lvl, _, _) = self.get_level_progress();
        match lvl {
            0..=2 => "E-Rank",
            3..=5 => "D-Rank",
            6..=10 => "C-Rank",
            11..=16 => "B-Rank",
            17..=24 => "A-Rank",
            25..=35 => "S-Rank",
            _ => "MONARCH",
        }
    }

    /// إضافة XP وإرجاع true إذا ارتفع المستوى (Level Up!)
    pub fn add_xp_check_levelup(&mut self, amount: u64) -> bool {
        if amount == 0 {
            return false;
        }
        let (old_lvl, _, _) = self.get_level_progress();
        self.xp = self.xp.saturating_add(amount);
        let (new_lvl, _, _) = self.get_level_progress();
        self.save();
        new_lvl > old_lvl
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
