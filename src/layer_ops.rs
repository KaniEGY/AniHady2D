//! Small safe layer operations; called after UI events so egui rendering
//! never holds a mutable borrow of the same layer being manipulated.
use crate::project::AniProject;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerAction {
    Delete,
    Duplicate,
    MoveUp,
    MoveDown,
}

pub fn apply_layer_action(
    project: &mut AniProject,
    index: usize,
    action: LayerAction,
) -> Result<usize, &'static str> {
    if index >= project.layers.len() {
        return Err("The selected layer no longer exists.");
    }
    match action {
        LayerAction::Delete => {
            if project.layers.len() <= 1 {
                return Err("A project must keep at least one layer.");
            }
            if project.layers[index].locked {
                return Err("Unlock the layer before deleting it.");
            }
            project.layers.remove(index);
            Ok(index.min(project.layers.len() - 1))
        }
        LayerAction::Duplicate => {
            let mut copied = project.layers[index].clone();
            copied.name.push_str(" Copy");
            copied.locked = false;
            copied.xp_first_paint_rewarded = false;
            project.layers.insert(index, copied);
            Ok(index)
        }
        LayerAction::MoveUp => {
            if index == 0 {
                return Err("This layer is already on top.");
            }
            project.layers.swap(index, index - 1);
            Ok(index - 1)
        }
        LayerAction::MoveDown => {
            if index + 1 >= project.layers.len() {
                return Err("This layer is already at the bottom.");
            }
            project.layers.swap(index, index + 1);
            Ok(index + 1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::AniProject;

    #[test]
    fn deleting_only_layer_is_rejected() {
        let mut project = AniProject::default();
        assert!(apply_layer_action(&mut project, 0, LayerAction::Delete).is_err());
        assert_eq!(project.layers.len(), 1);
    }

    #[test]
    fn locked_layer_cannot_be_deleted() {
        let mut project = AniProject::default();
        apply_layer_action(&mut project, 0, LayerAction::Duplicate).unwrap();
        project.layers[0].locked = true;
        assert!(apply_layer_action(&mut project, 0, LayerAction::Delete).is_err());
        assert_eq!(project.layers.len(), 2);
    }

    #[test]
    fn duplicating_preserves_content_and_unlocks_copy() {
        let mut project = AniProject::default();
        project.layers[0].locked = true;
        project.layers[0].opacity = 0.35;
        apply_layer_action(&mut project, 0, LayerAction::Duplicate).unwrap();
        assert_eq!(project.layers.len(), 2);
        assert!(!project.layers[0].locked);
        assert!((project.layers[0].opacity - 0.35).abs() < f32::EPSILON);
        assert!(project.layers[0].name.ends_with(" Copy"));
        assert!(project.layers[1].locked);
    }

    #[test]
    fn moving_layers_swaps_order_without_renaming() {
        let mut project = AniProject::default();
        apply_layer_action(&mut project, 0, LayerAction::Duplicate).unwrap();
        project.layers[0].name = "Top".into();
        project.layers[1].name = "Bottom".into();
        let selected = apply_layer_action(&mut project, 0, LayerAction::MoveDown).unwrap();
        assert_eq!(selected, 1);
        assert_eq!(project.layers[0].name, "Bottom");
        assert_eq!(project.layers[1].name, "Top");
    }
}
