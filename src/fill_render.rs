//! Polygon fill rendering shared by ordinary shapes and bucket-generated contours.
//! egui::Shape::convex_polygon only supports convex input; flood-fill boundaries
//! routinely contain concavities (and formerly appeared as white streaks/notches).
//! Earcut triangulates the actual simple polygon rather than a convex fan.
use eframe::egui;

const MAX_FILL_VERTICES: usize = 20_000;

pub fn solid_polygon_mesh(points: &[egui::Pos2], color: egui::Color32) -> Option<egui::Mesh> {
    solid_polygon_mesh_rings(&[points.to_vec()], color)
}

/// Triangulate one filled region with optional hole rings. This is used by
/// Bucket fills so a closed contour inside another closed contour stays an
/// independent paintable region instead of being swallowed by the outer fill.
pub fn solid_polygon_mesh_rings(
    rings: &[Vec<egui::Pos2>], color: egui::Color32,
) -> Option<egui::Mesh> {
    if rings.is_empty() || color.a()==0 { return None; }
    let mut vertices=Vec::<f64>::new();
    let mut positions=Vec::<egui::Pos2>::new();
    let mut holes=Vec::<usize>::new();
    for (ring_index,ring) in rings.iter().enumerate() {
        if ring.len()<3 || ring.iter().any(|p|!p.x.is_finite()||!p.y.is_finite()) {
            return None;
        }
        let mut end=ring.len();
        if ring[0].distance_sq(ring[end-1])<0.0001 {end-=1;}
        if end<3 {return None;}
        if ring_index>0 {holes.push(positions.len());}
        for pos in &ring[..end] {
            positions.push(*pos);
            vertices.extend([pos.x as f64,pos.y as f64]);
        }
        if positions.len()>MAX_FILL_VERTICES {return None;}
    }
    let triangles=earcutr::earcut(&vertices,&holes,2).ok()?;
    if triangles.len()<3 || triangles.len()%3!=0 {return None;}
    let mut mesh=egui::Mesh::default();
    for pos in positions {
        mesh.vertices.push(egui::epaint::Vertex {
            pos,uv:egui::epaint::WHITE_UV,color,
        });
    }
    for idx in triangles {mesh.indices.push(u32::try_from(idx).ok()?);}
    Some(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x:f32,y:f32)->egui::Pos2 { egui::pos2(x,y) }
    fn total_triangle_area(mesh: &egui::Mesh) -> f64 {
        mesh.indices.chunks_exact(3).map(|idx| {
            let a=mesh.vertices[idx[0] as usize].pos;
            let b=mesh.vertices[idx[1] as usize].pos;
            let c=mesh.vertices[idx[2] as usize].pos;
            ((b.x-a.x)*(c.y-a.y)-(b.y-a.y)*(c.x-a.x)).abs() as f64 * 0.5
        }).sum()
    }
    #[test]
    fn concave_notch_is_not_filled_as_a_convex_hull() {
        let pts=[p(0.,0.),p(100.,0.),p(100.,30.),p(30.,30.),
                 p(30.,100.),p(0.,100.),p(0.,0.)];
        let mesh=solid_polygon_mesh(&pts,egui::Color32::RED).expect("triangulate L-shape");
        // 100*30 + 30*70 = 5100, not the convex-hull area.
        assert!((total_triangle_area(&mesh)-5100.0).abs()<0.05);
    }
    #[test]
    fn convex_shape_with_duplicate_endpoint_keeps_correct_area() {
        let pts=[p(0.,0.),p(50.,0.),p(50.,50.),p(0.,50.),p(0.,0.)];
        let mesh=solid_polygon_mesh(&pts,egui::Color32::YELLOW).unwrap();
        assert_eq!(mesh.vertices.len(),4);
        assert!((total_triangle_area(&mesh)-2500.0).abs()<0.01);
    }
    #[test]
    fn rejects_invalid_and_transparent_fill_meshes() {
        assert!(solid_polygon_mesh(&[p(0.,0.),p(1.,1.)],egui::Color32::RED).is_none());
        assert!(solid_polygon_mesh(&[p(0.,0.),p(f32::NAN,1.),p(3.,1.)],egui::Color32::RED).is_none());
        assert!(solid_polygon_mesh(&[p(0.,0.),p(1.,0.),p(1.,1.)],egui::Color32::TRANSPARENT).is_none());
    }
    #[test]
    fn nested_ring_remains_a_real_hole() {
        let outer=vec![p(0.,0.),p(100.,0.),p(100.,100.),p(0.,100.),p(0.,0.)];
        let hole=vec![p(30.,30.),p(30.,70.),p(70.,70.),p(70.,30.),p(30.,30.)];
        let mesh=solid_polygon_mesh_rings(&[outer,hole],egui::Color32::RED).unwrap();
        assert!((total_triangle_area(&mesh)-8400.0).abs()<0.05);
    }
}
