pub mod atomic_save;
pub mod migration;
pub mod sniff;

use crate::project::AniProject;
use atomic_save::{create_legacy_backup_copy, save_project_atomic};
use migration::{decode_and_migrate, MigrationReport};
use sniff::sniff_project_bytes;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct LoadedProjectOutcome {
    pub project: AniProject,
    pub report: MigrationReport,
    pub missing_assets: Vec<String>,
    pub legacy_backup_path: Option<PathBuf>,
}

pub fn open_project_safe(path: &Path) -> Result<LoadedProjectOutcome, String> {
    if !path.exists() {
        return Err(format!("الملف غير موجود: {}", path.display()));
    }
    let raw = fs::read(path).map_err(|e| format!("تعذر قراءة الملف من القرص: {}", e))?;
    let sniffed = sniff_project_bytes(&raw)?;
    let (project, report) = decode_and_migrate(sniffed)?;

    let legacy_backup_path = if report.was_migrated {
        create_legacy_backup_copy(path).ok()
    } else {
        None
    };

    let missing_assets = collect_missing_assets(&project);

    Ok(LoadedProjectOutcome {
        project,
        report,
        missing_assets,
        legacy_backup_path,
    })
}

pub fn save_project_safe(project: &AniProject, target_path: &Path) -> Result<(), String> {
    save_project_atomic(project, target_path)
}

pub fn collect_missing_assets(project: &AniProject) -> Vec<String> {
    let mut missing = Vec::new();
    for layer in &project.layers {
        for fd in layer.keyframes.values() {
            for m in &fd.media_items {
                if !m.path.is_empty() && !Path::new(&m.path).exists() && !missing.contains(&m.path) {
                    missing.push(m.path.clone());
                }
            }
        }
    }
    for track in &project.audio_tracks {
        if !track.file_path.is_empty()
            && !Path::new(&track.file_path).exists()
            && !missing.contains(&track.file_path)
        {
            missing.push(track.file_path.clone());
        }
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{KritaBrushPreset, Stroke, StrokePoint};

    #[test]
    fn cmp_002_legacy_v1_preserves_layers_and_strokes() {
        let legacy_v1_json = r#"{
            "width": 1280,
            "height": 720,
            "fps": 24,
            "layers": [
                {
                    "name": "طبقة_قديمة",
                    "visible": true,
                    "locked": false,
                    "frames": [
                        {
                            "strokes": [
                                {
                                    "points": [{"x": 10.0, "y": 20.0}, {"x": 100.0, "y": 150.0}],
                                    "color": [255, 0, 0, 255],
                                    "base_width": 5.0
                                }
                            ]
                        }
                    ]
                }
            ]
        }"#;

        let sniffed = sniff_project_bytes(legacy_v1_json.as_bytes()).expect("Should detect Legacy V1");
        let (proj, report) = decode_and_migrate(sniffed).expect("Should migrate Legacy V1");
        assert!(report.was_migrated);
        assert_eq!(report.old_schema, 1);
        assert_eq!(proj.layers.len(), 1);
        assert_eq!(proj.layers[0].name, "طبقة_قديمة");
        let f1 = proj.layers[0].keyframes.get(&1).expect("Frame 1 must exist");
        assert_eq!(f1.strokes.len(), 1);
        assert_eq!(f1.strokes[0].points.len(), 2);
    }

    #[test]
    fn cmp_003_rejects_unknown_json_instead_of_empty_project() {
        let unknown = br#"{"hello": 1, "random": "data"}"#;
        let res = sniff_project_bytes(unknown);
        assert!(res.is_err(), "Unknown JSON must be rejected");
    }

    #[test]
    fn cmp_005_rejects_newer_future_schema() {
        let future = br#"{"magic":"STICK_STUDIO_PROJECT","schema_version":999,"project":{}}"#;
        let sniffed = sniff_project_bytes(future).expect("Sniff detects newer schema");
        let res = decode_and_migrate(sniffed);
        assert!(res.is_err(), "Future schema must not be silently downgraded");
    }

    #[test]
    fn cmp_004_and_008_atomic_save_and_reload_roundtrip() {
        let tmp_dir = std::env::temp_dir().join(format!("stick_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&tmp_dir);
        let file_path = tmp_dir.join("مشروع_اختبار.stick");

        let mut proj = AniProject::default();
        proj.layers[0].name = "رسم_عربي".to_string();
        proj.layers[0].keyframes.get_mut(&1).unwrap().strokes.push(Stroke {
            points: vec![
                StrokePoint { x: 12.0, y: 34.0, pressure: 1.0 },
                StrokePoint { x: 56.0, y: 78.0, pressure: 1.0 },
            ],
            color: [10, 20, 30, 255],
            fill_color: None,
            base_width: 4.0,
            is_vector: false,
            brush_preset: KritaBrushPreset::GPenInk,
            opacity: 1.0,
            brush_texture_id: None,
        });

        save_project_safe(&proj, &file_path).expect("Atomic save should succeed");
        let loaded = open_project_safe(&file_path).expect("Reload should succeed");
        assert_eq!(loaded.project.layers[0].name, "رسم_عربي");
        assert_eq!(loaded.project.layers[0].keyframes[&1].strokes.len(), 1);
        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
