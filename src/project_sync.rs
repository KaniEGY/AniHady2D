//! Small, transport-neutral project edits for LAN collaboration.
//! Paths address the serialized document rather than UI selection indices.
//! This keeps erasing, keyframes, layer changes, text and project settings in one pipeline.
//! Independent edits to different fields/frames can be merged by the host.
use crate::project::AniProject;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectEdit {
    Set { path: Vec<String>, value: Value },
    Delete { path: Vec<String> },
    Insert { path: Vec<String>, index: usize, value: Value },
    Remove { path: Vec<String>, index: usize },
}

fn diff_value(old: &Value, new: &Value, path: &mut Vec<String>, ops: &mut Vec<ProjectEdit>) {
    if old == new { return; }
    match (old, new) {
        (Value::Object(a), Value::Object(b)) => {
            for key in a.keys().filter(|key| !b.contains_key(*key)) {
                let mut child = path.clone();
                child.push(key.to_string());
                ops.push(ProjectEdit::Delete { path: child });
            }
            for (key, value) in b {
                path.push(key.clone());
                match a.get(key) {
                    Some(prior) => diff_value(prior, value, path, ops),
                    None => ops.push(ProjectEdit::Set { path: path.clone(), value: value.clone() }),
                }
                path.pop();
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            if b.len() > a.len() {
                let count = b.len() - a.len();
                for at in 0..=a.len() {
                    if a[..at] == b[..at] && a[at..] == b[at + count..] {
                        for (offset, item) in b[at..at + count].iter().enumerate() {
                            ops.push(ProjectEdit::Insert {
                                path: path.clone(), index: at + offset, value: item.clone()
                            });
                        }
                        return;
                    }
                }
            } else if a.len() > b.len() {
                let count = a.len() - b.len();
                for at in 0..=b.len() {
                    if a[..at] == b[..at] && a[at + count..] == b[at..] {
                        for _ in 0..count {
                            ops.push(ProjectEdit::Remove { path: path.clone(), index: at });
                        }
                        return;
                    }
                }
            } else {
                for (idx, (before, after)) in a.iter().zip(b.iter()).enumerate() {
                    path.push(idx.to_string());
                    diff_value(before, after, path, ops);
                    path.pop();
                }
                return;
            }
            // Ambiguous multi-element transformation: replace just this array.
            ops.push(ProjectEdit::Set { path: path.clone(), value: new.clone() });
        }
        _ => ops.push(ProjectEdit::Set { path: path.clone(), value: new.clone() }),
    }
}

pub fn diff_projects(old: &AniProject, new: &AniProject) -> Result<Vec<ProjectEdit>, String> {
    let before = serde_json::to_value(old).map_err(|e| e.to_string())?;
    let after = serde_json::to_value(new).map_err(|e| e.to_string())?;
    let mut ops = Vec::new();
    diff_value(&before, &after, &mut Vec::new(), &mut ops);
    Ok(ops)
}

fn at_path_mut<'a>(root: &'a mut Value, path: &[String]) -> Result<&'a mut Value, String> {
    let mut node = root;
    for key in path {
        node = match node {
            Value::Object(map) => map.get_mut(key).ok_or_else(|| format!("Missing field: {key}"))?,
            Value::Array(array) => {
                let index = key.parse::<usize>().map_err(|_| "Invalid array index")?;
                array.get_mut(index).ok_or("Array index out of bounds")?
            }
            _ => return Err("Edit path traverses a scalar".into()),
        };
    }
    Ok(node)
}

fn set_value(root: &mut Value, path: &[String], value: Value, delete: bool) -> Result<(), String> {
    let (last, parent) = path.split_last().ok_or("Refusing to replace whole project")?;
    match at_path_mut(root, parent)? {
        Value::Object(map) => {
            if delete {
                if map.remove(last).is_none() { return Err("Missing field for deletion".into()); }
            } else { map.insert(last.clone(), value); }
        }
        Value::Array(items) => {
            let index = last.parse::<usize>().map_err(|_| "Invalid array index")?;
            if index >= items.len() { return Err("Array index out of bounds".into()); }
            if delete { items.remove(index); } else { items[index] = value; }
        }
        _ => return Err("Edit parent is not a collection".into()),
    }
    Ok(())
}

pub fn apply_edits(project: &mut AniProject, ops: &[ProjectEdit]) -> Result<(), String> {
    if ops.len() > 20_000 { return Err("Too many edits in a single batch".into()); }
    let mut root = serde_json::to_value(&*project).map_err(|e| e.to_string())?;
    for op in ops {
        match op {
            ProjectEdit::Set { path, value } => set_value(&mut root, path, value.clone(), false)?,
            ProjectEdit::Delete { path } => set_value(&mut root, path, Value::Null, true)?,
            ProjectEdit::Insert { path, index, value } => {
                let items = at_path_mut(&mut root, path)?.as_array_mut()
                    .ok_or("Insert target is not an array")?;
                if *index > items.len() { return Err("Insert index out of bounds".into()); }
                items.insert(*index, value.clone());
            }
            ProjectEdit::Remove { path, index } => {
                let items = at_path_mut(&mut root, path)?.as_array_mut()
                    .ok_or("Remove target is not an array")?;
                if *index >= items.len() { return Err("Remove index out of bounds".into()); }
                items.remove(*index);
            }
        }
    }
    // A malicious or outdated peer must not be able to destroy the live document.
    let updated: AniProject = serde_json::from_value(root).map_err(|e| e.to_string())?;
    if updated.layers.is_empty() || updated.layers.len() > 256 ||
        updated.width == 0 || updated.height == 0 ||
        updated.width > 16_384 || updated.height > 16_384 ||
        updated.total_frames == 0 || updated.total_frames > 20_000 ||
        updated.layers.iter().any(|l| l.keyframes.keys().any(|f| *f == 0 || *f > 20_000)) {
        return Err("Invalid project dimensions, layers or frames".into());
    }
    *project = updated;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::FrameData;

    #[test]
    fn synchronizes_layer_create_delete_and_frame_changes() {
        let base = AniProject::default();
        let mut edited = base.clone();
        let mut layer = edited.layers[0].clone();
        layer.name = "New layer".into();
        edited.layers.insert(0, layer);
        edited.layers[1].keyframes.insert(7, FrameData::default());
        edited.total_frames = 7;
        let ops = diff_projects(&base, &edited).unwrap();
        let mut replica = base.clone();
        apply_edits(&mut replica, &ops).unwrap();
        assert_eq!(serde_json::to_value(replica).unwrap(), serde_json::to_value(&edited).unwrap());

        let before_delete = edited.clone();
        edited.layers.remove(0);
        edited.layers[0].keyframes.remove(&7);
        edited.total_frames = 1;
        let reverse = diff_projects(&before_delete, &edited).unwrap();
        assert!(!reverse.is_empty());
        let mut replica = before_delete;
        apply_edits(&mut replica, &reverse).unwrap();
        assert_eq!(serde_json::to_value(replica).unwrap(), serde_json::to_value(edited).unwrap());
    }

    #[test]
    fn independent_keyframe_edits_merge_in_either_order() {
        let mut base = AniProject::default();
        base.total_frames = 10;
        let mut first = base.clone();
        let mut second = base.clone();
        first.layers[0].keyframes.insert(3, FrameData { label: "A".into(), ..Default::default() });
        second.layers[0].keyframes.insert(8, FrameData { label: "B".into(), ..Default::default() });
        let a = diff_projects(&base, &first).unwrap();
        let b = diff_projects(&base, &second).unwrap();
        let mut merged = base.clone();
        apply_edits(&mut merged, &a).unwrap();
        apply_edits(&mut merged, &b).unwrap();
        assert_eq!(merged.layers[0].keyframes.get(&3).unwrap().label, "A");
        assert_eq!(merged.layers[0].keyframes.get(&8).unwrap().label, "B");
    }

    #[test]
    fn eraser_removes_strokes_and_propagates_the_deletion() {
        use crate::project::{Stroke, StrokePoint};
        let mut before = AniProject::default();
        let dab = |x: f32| Stroke {
            points: vec![StrokePoint { x, y: 42.0, pressure: 1.0 }],
            color: [0, 0, 0, 255], fill_color: None, base_width: 4.0,
            is_vector: false, brush_preset: Default::default(), opacity: 1.0,
            brush_texture_id: None, tip_shape: Default::default(),
        };
        before.layers[0].keyframes.get_mut(&1).unwrap().strokes =
            vec![dab(10.0), dab(20.0), dab(30.0)];
        let mut erased = before.clone();
        erased.layers[0].keyframes.get_mut(&1).unwrap().strokes.remove(1);
        let edits = diff_projects(&before, &erased).expect("compute eraser edits");
        let mut peer = before.clone();
        apply_edits(&mut peer, &edits).expect("apply eraser edits on peer");
        let result = &peer.layers[0].keyframes[&1].strokes;
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].points[0].x, 10.0);
        assert_eq!(result[1].points[0].x, 30.0);
    }

    #[test]
    fn concurrent_stroke_appends_keep_both_items() {
        use crate::project::{Stroke, StrokePoint};
        let mut base = AniProject::default();
        let mut a = base.clone();
        let mut b = base.clone();
        let stroke = |x| Stroke {
            points: vec![StrokePoint { x, y: 10., pressure: 1. }],
            color: [0, 0, 0, 255], fill_color: None, base_width: 5.,
            is_vector: false, brush_preset: Default::default(), opacity: 1.,
            brush_texture_id: None, tip_shape: Default::default(),
        };
        a.layers[0].keyframes.get_mut(&1).unwrap().strokes.push(stroke(11.));
        b.layers[0].keyframes.get_mut(&1).unwrap().strokes.push(stroke(99.));
        let a_ops = diff_projects(&base, &a).unwrap();
        let b_ops = diff_projects(&base, &b).unwrap();
        apply_edits(&mut base, &a_ops).unwrap();
        apply_edits(&mut base, &b_ops).unwrap();
        assert_eq!(base.layers[0].keyframes[&1].strokes.len(), 2);
    }
}
