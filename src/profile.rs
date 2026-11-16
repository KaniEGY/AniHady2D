use serde::{Deserialize, Serialize};

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

    pub fn get_hunter_rank(&self) -> &'static str {
        let (lvl, _, _) = self.get_level_progress();
        match lvl {
            0..=9 => "E-Rank",
            10..=24 => "D-Rank",
            25..=44 => "C-Rank",
            45..=69 => "B-Rank",
            70..=99 => "A-Rank",
            _ => "S-Rank",
        }
    }

    /// رسالة النظام الخاصة بارتقاء المستوى تتغير كل 5 مستويات حتى المستوى 100
    pub fn get_level_up_system_message(level: u32) -> &'static str {
        if level >= 100 {
            return "انت من المستوى S دلوقتي انت اسطورة الرسم و الانميشن";
        }
        match (level.saturating_sub(1)) / 5 {
            0 => "مستواك ارتفع يا بطل اقتربت اكتر من انك تكون انميتور جبار",
            1 => "تقدم مذهل! خطوطك وحركتك بدأت تأخذ طابع المحترفين",
            2 => "قوة التحريك لديك تتضاعف! استمر في صناعة الإبداع",
            3 => "لقد تجاوزت مرحلة المبتدئين بنجاح، إيقاعك في الأنيميشن رائع",
            4 => "مهاراتك في الرسم والتحريك تتطور بسرعة مذهلة!",
            5 => "أصبحت تتحكم في الفريمات بمرونة عالية، واصل الصعود!",
            6 => "إنجاز جديد! لمستك الفنية بدأت تظهر بقوة في كل مشهد",
            7 => "أنت الآن في مصاف الرسامين المتميزين، لا تتوقف!",
            8 => "سرعتك ودقتك في التحريك وصلت لمستوى يثير الإعجاب",
            9 => "نصف الطريق نحو القمة! لقد أصبحت محرك رسوم محترف",
            10 => "قوة إبداعية هائلة! كل فريم ترسمه ينبض بالحياة",
            11 => "تحكمك في التوقيت والانسيابية أصبح مذهلاً بحق",
            12 => "لقد دخلت دائرة النخبة في عالم الرسم والأنيميشن!",
            13 => "إصرارك يصنع المعجزات، أعمالك أصبحت لوحات فنية متحركة",
            14 => "رتبتك ترتفع بقوة! لم يعد يفصلك عن القمة سوى القليل",
            15 => "أنت تقترب من حدود الأسطورة، استمر في تحطيم الأرقام!",
            16 => "براعة فائقة في التفاصيل والحركة، مستوى استثنائي!",
            17 => "القليل فقط يصلون إلى هذا المستوى من الإتقان!",
            18 => "خطوات أخيرة تفصلك عن الرتبة الأعلى S-Rank!",
            _ => "أوشكت على بلوغ الأسطورة! استعد لدخول المستوى S!",
        }
    }

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
