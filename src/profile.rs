use serde::{Deserialize, Serialize};

/// هيكل بيانات ملف المستخدم (مجهز للتوسع المستقبلي للحسابات والمزامنة السحابية)
#[derive(Serialize, Deserialize, Clone)]
pub struct UserProfile {
    pub username: String,
    pub avatar_path: Option<String>,
    pub total_seconds_spent: u64,
    pub total_frames_drawn: u64,
    pub xp: u64,
    // حقول محجوزة للتطوير المستقبلي (الأونلاين والحفظ السحابي)
    #[serde(default)]
    pub cloud_account_id: Option<String>,
    #[serde(default)]
    pub sync_token: Option<String>,
}

impl Default for UserProfile {
    fn default() -> Self {
        Self {
            username: "Hady Artist".to_string(),
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

    /// حساب المستوى الحالي والـ XP المطلوب للمستوى التالي (100 للمستوى 1، 150 للمستوى 2، 200 للمستوى 3...)
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

    pub fn add_xp_silent(&mut self, amount: u64) {
        if amount > 0 {
            self.xp = self.xp.saturating_add(amount);
            self.save();
        }
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
