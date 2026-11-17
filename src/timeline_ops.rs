//! Timeline editing logic independent of egui, shared by the frame context menu.
//! Keyframes are per layer; deleting a whole timeline frame is global.
use crate::project::{AniProject, FrameData};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameAction {
    Copy,
    Paste,
    Clear,
    DeleteKeyframe,
    DeleteFrame,
    Keyframe,
    BlankKeyframe,
}

pub fn apply_frame_edit(
    project: &mut AniProject,
    layer_index: usize,
    frame: usize,
    action: FrameAction,
) -> Result<(), &'static str> {
    if frame == 0 || frame > project.total_frames {
        return Err("Frame is outside the timeline.");
    }
    if layer_index >= project.layers.len() {
        return Err("Layer is missing.");
    }
    if matches!(action, FrameAction::Copy | FrameAction::Paste) {
        return Err("Clipboard operations need the editor clipboard.");
    }
    if action == FrameAction::DeleteFrame {
        if project.total_frames <= 1 {
            return Err("Cannot delete the last frame.");
        }
        if project.layers.iter().any(|layer| layer.locked) {
            return Err("Unlock all layers before deleting time across the timeline.");
        }
        // The new first frame must show what the old second frame displayed.
        // Otherwise deleting frame 1 turns a held drawing into a blank cel.
        let first_after_delete: Vec<FrameData> = if frame == 1 {
            project.layers.iter().map(|layer| project.evaluate_layer_at_frame(layer, 2)).collect()
        } else { Vec::new() };
        for (layer_index, layer) in project.layers.iter_mut().enumerate() {
            let mut shifted = HashMap::with_capacity(layer.keyframes.len());
            for (&index, data) in &layer.keyframes {
                if index < frame {
                    shifted.insert(index, data.clone());
                } else if index > frame {
                    shifted.insert(index - 1, data.clone());
                }
            }
            if !shifted.contains_key(&1) {
                // Preserve the visible exposure when time is deleted at start.
                shifted.insert(1, first_after_delete.get(layer_index).cloned().unwrap_or_default());
            }
            layer.keyframes = shifted;
        }
        project.total_frames -= 1;
        return Ok(());
    }

    if project.layers[layer_index].locked {
        return Err("This layer is locked.");
    }

    // Compute from the old timeline before changing this layer's keyframes.
    let evaluated = if action == FrameAction::Keyframe {
        let layer = &project.layers[layer_index];
        Some(project.evaluate_layer_at_frame(layer, frame))
    } else {
        None
    };

    let layer = &mut project.layers[layer_index];
    match action {
        FrameAction::Clear | FrameAction::BlankKeyframe => {
            layer.keyframes.insert(frame, FrameData::default());
        }
        FrameAction::Keyframe => {
            layer.keyframes.insert(frame, evaluated.unwrap_or_default());
        }
        FrameAction::DeleteKeyframe => {
            if layer.keyframes.remove(&frame).is_none() {
                return Err("There is no keyframe to delete here.");
            }
            if !layer.keyframes.contains_key(&1) {
                layer.keyframes.insert(1, FrameData::default());
            }
        }
        FrameAction::DeleteFrame | FrameAction::Copy | FrameAction::Paste => unreachable!(),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{Layer, Stroke, StrokePoint, KritaBrushPreset};

    fn dab() -> Stroke {
        Stroke {
            points: vec![StrokePoint { x: 15.0, y: 20.0, pressure: 1.0 }],
            color: [0, 0, 0, 255],
            fill_color: None,
            base_width: 5.0,
            is_vector: false,
            brush_preset: KritaBrushPreset::GPenInk,
            opacity: 1.0,
            brush_texture_id: None,
            tip_shape: crate::project::BrushTipShape::Round,
        }
    }

    #[test]
    fn keyframe_copies_current_visible_content_without_removing_original() {
        let mut p = AniProject::default();
        p.total_frames = 12;
        p.layers[0].keyframes.get_mut(&1).unwrap().strokes.push(dab());
        apply_frame_edit(&mut p, 0, 6, FrameAction::Keyframe).unwrap();
        assert_eq!(p.layers[0].keyframes[&1].strokes.len(), 1);
        assert_eq!(p.layers[0].keyframes[&6].strokes.len(), 1);
        apply_frame_edit(&mut p, 0, 7, FrameAction::BlankKeyframe).unwrap();
        assert!(p.layers[0].keyframes[&7].strokes.is_empty());
    }

    #[test]
    fn clearing_a_frame_creates_an_empty_keyframe_here_only() {
        let mut p = AniProject::default();
        p.total_frames = 10;
        p.layers[0].keyframes.get_mut(&1).unwrap().strokes.push(dab());
        apply_frame_edit(&mut p, 0, 5, FrameAction::Clear).unwrap();
        assert_eq!(p.layers[0].keyframes[&1].strokes.len(), 1);
        assert!(p.layers[0].keyframes[&5].strokes.is_empty());
    }

    #[test]
    fn deleting_time_shifts_all_layers_and_preserves_alignment() {
        let mut p = AniProject::default();
        p.total_frames = 10;
        let mut another = p.layers[0].clone();
        another.keyframes.clear();
        another.keyframes.insert(5, FrameData::default());
        p.layers[0].keyframes.insert(5, FrameData::default());
        p.layers.push(another);
        apply_frame_edit(&mut p, 0, 3, FrameAction::DeleteFrame).unwrap();
        assert_eq!(p.total_frames, 9);
        assert!(p.layers.iter().all(|l| l.keyframes.contains_key(&4)));
        assert!(p.layers.iter().all(|l| !l.keyframes.contains_key(&5)));
    }

    #[test]
    fn deleting_first_frame_preserves_the_second_frames_held_drawing() {
        let mut p = AniProject::default();
        p.total_frames = 4;
        p.layers[0].keyframes.get_mut(&1).unwrap().strokes.push(dab());
        apply_frame_edit(&mut p, 0, 1, FrameAction::DeleteFrame).unwrap();
        assert_eq!(p.total_frames, 3);
        assert_eq!(p.layers[0].keyframes[&1].strokes.len(), 1,
            "deleting frame 1 should not erase artwork held on old frame 2");
    }

    #[test]
    fn locked_layer_rejects_changes_and_global_deletion() {
        let mut p = AniProject::default();
        p.total_frames = 10;
        p.layers[0].locked = true;
        assert!(apply_frame_edit(&mut p, 0, 5, FrameAction::BlankKeyframe).is_err());
        assert!(apply_frame_edit(&mut p, 0, 5, FrameAction::DeleteFrame).is_err());
        assert_eq!(p.total_frames, 10);
        assert_eq!(p.layers[0].keyframes.len(), 1);
    }

    #[test]
    fn a_keyframe_can_be_deleted_without_removing_timeline_time() {
        let mut p = AniProject::default();
        p.total_frames = 10;
        p.layers[0].keyframes.insert(5, FrameData::default());
        apply_frame_edit(&mut p, 0, 5, FrameAction::DeleteKeyframe).unwrap();
        assert_eq!(p.total_frames, 10);
        assert!(!p.layers[0].keyframes.contains_key(&5));
    }
}
