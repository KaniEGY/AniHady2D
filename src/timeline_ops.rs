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
    /// Duplicate the current frame's visible content into the next frame
    /// (frame + 1), inserting a new keyframe there if one doesn't already
    /// exist.  The timeline length stays the same.
    Duplicate,
    /// Insert a blank frame at `frame`, shifting every keyframe at or after
    /// that position forward by one.  `total_frames` grows by one.
    InsertEmpty,
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

    // ── InsertEmpty ────────────────────────────────────────────────────────
    // Inserts a blank keyframe at `frame` on *every* layer, shifting keyframes
    // at or after that position forward by one, and grows total_frames by one.
    if action == FrameAction::InsertEmpty {
        if project.layers.iter().any(|layer| layer.locked) {
            return Err("Unlock all layers before inserting time across the timeline.");
        }
        for layer in project.layers.iter_mut() {
            // Collect keys that need shifting, in descending order so we never
            // overwrite a key that hasn't been shifted yet.
            let mut to_shift: Vec<usize> = layer
                .keyframes
                .keys()
                .copied()
                .filter(|&k| k >= frame)
                .collect();
            to_shift.sort_unstable_by(|a, b| b.cmp(a)); // descending
            for k in to_shift {
                let data = layer.keyframes.remove(&k).unwrap();
                layer.keyframes.insert(k + 1, data);
            }
            // Guarantee a blank keyframe at the insertion point.
            layer.keyframes.entry(frame).or_insert_with(FrameData::default);
        }
        project.total_frames += 1;
        return Ok(());
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
    let evaluated = if matches!(action, FrameAction::Keyframe | FrameAction::Duplicate) {
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
        FrameAction::Duplicate => {
            // `frame + 1` must exist in the timeline.
            if frame >= project.total_frames {
                return Err("Cannot duplicate the last frame — no next frame exists.");
            }
            let next = frame + 1;
            // Clone the evaluated content of `frame` into `next`.
            // We only create a new keyframe at `next` if one doesn't already
            // live there; existing keyframes intentionally override held content.
            layer.keyframes.entry(next).or_insert_with(|| evaluated.unwrap_or_default());
        }
        FrameAction::DeleteFrame | FrameAction::Copy | FrameAction::Paste
        | FrameAction::InsertEmpty => unreachable!(),
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

    // ── Duplicate tests ───────────────────────────────────────────────────

    #[test]
    fn duplicate_copies_visible_content_into_next_frame() {
        let mut p = AniProject::default();
        p.total_frames = 10;
        p.layers[0].keyframes.get_mut(&1).unwrap().strokes.push(dab());
        // Duplicate frame 1 — its content should appear as a new keyframe at 2.
        apply_frame_edit(&mut p, 0, 1, FrameAction::Duplicate).unwrap();
        assert_eq!(p.total_frames, 10, "Duplicate must not change timeline length");
        assert_eq!(p.layers[0].keyframes[&2].strokes.len(), 1,
            "frame 2 should hold a copy of the stroke from frame 1");
        // The original must be unchanged.
        assert_eq!(p.layers[0].keyframes[&1].strokes.len(), 1,
            "original frame 1 must not be modified");
    }

    #[test]
    fn duplicate_uses_held_content_not_just_explicit_keyframes() {
        let mut p = AniProject::default();
        p.total_frames = 10;
        // Frame 1 has a stroke; frame 5 is not a keyframe — it holds frame 1's art.
        p.layers[0].keyframes.get_mut(&1).unwrap().strokes.push(dab());
        apply_frame_edit(&mut p, 0, 5, FrameAction::Duplicate).unwrap();
        // Frame 6 should now have a keyframe with the held content of frame 5.
        assert!(p.layers[0].keyframes.contains_key(&6),
            "frame 6 must become a keyframe");
        assert_eq!(p.layers[0].keyframes[&6].strokes.len(), 1,
            "frame 6 should hold the stroke that was visible at frame 5");
    }

    #[test]
    fn duplicate_last_frame_returns_error() {
        let mut p = AniProject::default();
        p.total_frames = 5;
        let err = apply_frame_edit(&mut p, 0, 5, FrameAction::Duplicate);
        assert!(err.is_err(), "duplicating the last frame should fail");
    }

    #[test]
    fn duplicate_respects_locked_layer() {
        let mut p = AniProject::default();
        p.total_frames = 10;
        p.layers[0].locked = true;
        let err = apply_frame_edit(&mut p, 0, 1, FrameAction::Duplicate);
        assert!(err.is_err(), "locked layer should reject Duplicate");
    }

    // ── InsertEmpty tests ─────────────────────────────────────────────────

    #[test]
    fn insert_empty_shifts_keyframes_forward_and_grows_timeline() {
        let mut p = AniProject::default();
        p.total_frames = 10;
        // Existing keyframe at frame 5.
        p.layers[0].keyframes.insert(5, FrameData { strokes: vec![dab()], ..Default::default() });
        apply_frame_edit(&mut p, 0, 3, FrameAction::InsertEmpty).unwrap();
        assert_eq!(p.total_frames, 11, "total_frames must grow by 1");
        // Old frame 5 should now be at 6.
        assert!(p.layers[0].keyframes.contains_key(&6),
            "keyframe that was at 5 should now be at 6");
        assert!(!p.layers[0].keyframes.contains_key(&5),
            "nothing should sit at old position 5 after shift — except the new blank");
        // The insertion point must have a blank keyframe.
        assert!(p.layers[0].keyframes.contains_key(&3),
            "a blank keyframe must exist at the insertion point");
        assert!(p.layers[0].keyframes[&3].strokes.is_empty(),
            "inserted frame must be blank");
    }

    #[test]
    fn insert_empty_at_frame1_shifts_all_existing_keyframes() {
        let mut p = AniProject::default();
        p.total_frames = 5;
        p.layers[0].keyframes.get_mut(&1).unwrap().strokes.push(dab());
        apply_frame_edit(&mut p, 0, 1, FrameAction::InsertEmpty).unwrap();
        assert_eq!(p.total_frames, 6);
        // Old frame 1 (with the stroke) must now be at frame 2.
        assert_eq!(p.layers[0].keyframes[&2].strokes.len(), 1,
            "content of old frame 1 should now live at frame 2");
        // Frame 1 must be the newly inserted blank.
        assert!(p.layers[0].keyframes[&1].strokes.is_empty(),
            "new frame 1 must be blank");
    }

    #[test]
    fn insert_empty_affects_all_layers_simultaneously() {
        let mut p = AniProject::default();
        p.total_frames = 8;
        // Add a second layer with its own keyframe at frame 4.
        let mut layer2 = p.layers[0].clone();
        layer2.keyframes.clear();
        layer2.keyframes.insert(4, FrameData { strokes: vec![dab()], ..Default::default() });
        p.layers.push(layer2);
        apply_frame_edit(&mut p, 0, 2, FrameAction::InsertEmpty).unwrap();
        assert_eq!(p.total_frames, 9);
        // Both layers must have the keyframe that was at 4 shifted to 5.
        for (i, layer) in p.layers.iter().enumerate() {
            assert!(layer.keyframes.contains_key(&5),
                "layer {i}: keyframe from pos 4 should be at pos 5 after insert");
        }
    }

    #[test]
    fn insert_empty_locked_layer_returns_error() {
        let mut p = AniProject::default();
        p.total_frames = 5;
        p.layers[0].locked = true;
        let err = apply_frame_edit(&mut p, 0, 2, FrameAction::InsertEmpty);
        assert!(err.is_err(), "locked layer should block InsertEmpty");
        assert_eq!(p.total_frames, 5, "total_frames must not change on error");
    }
}
