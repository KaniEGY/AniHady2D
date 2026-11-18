//! Procedural brush tips in stroke coordinates. Preview and raster exports
//! use the same alpha mask, orientation, pressure and spacing.
use crate::brush_engine::{dab_polygon, visit_pressure_dabs, BrushDab};
use crate::project::{BrushTipShape, KritaBrushPreset, Stroke};
use eframe::egui;
use std::sync::OnceLock;
use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

const SIZE: usize = 32;
const AREA: usize = SIZE * SIZE;

#[derive(Clone, Copy, PartialEq)]
pub enum TextureBrush {
    Dry,
    Chalk,
    Charcoal,
    Pastel,
}

impl TextureBrush {
    pub fn for_stroke(stroke: &Stroke) -> Option<Self> {
        if stroke.is_vector {
            return None;
        }
        match stroke.brush_preset {
            KritaBrushPreset::DryBrush => Some(Self::Dry),
            KritaBrushPreset::Chalk => Some(Self::Chalk),
            KritaBrushPreset::Charcoal => Some(Self::Charcoal),
            KritaBrushPreset::SoftPastel => Some(Self::Pastel),
            _ => None,
        }
    }

    fn mask(self, tip: BrushTipShape) -> &'static [u8; AREA] {
        static MASKS: [OnceLock<[u8; AREA]>; 24] = [const { OnceLock::new() }; 24];
        MASKS[self as usize * 6 + tip as usize].get_or_init(|| self.make_mask(tip))
    }

    fn make_mask(self, tip: BrushTipShape) -> [u8; AREA] {
        // Rectangular aspect ratios are applied by the stamp transform.
        let footprint = match tip {
            BrushTipShape::TallRectangle | BrushTipShape::WideRectangle => BrushTipShape::Square,
            other => other,
        };
        let polygon = dab_polygon(
            footprint,
            BrushDab {
                x: 0.0,
                y: 0.0,
                radius: 1.0,
            },
        );
        std::array::from_fn(|index| {
            let (x, y) = (index % SIZE, index / SIZE);
            let u = (2.0 * x as f32 + 1.0) / SIZE as f32 - 1.0;
            let v = (2.0 * y as f32 + 1.0) / SIZE as f32 - 1.0;
            // Clockwise vertices in canvas coordinates; signed distance to the
            // nearest side also gives polygon tips a naturally broken edge.
            let inside = if polygon.is_empty() {
                1.0 - u.hypot(v)
            } else {
                polygon
                    .iter()
                    .zip(polygon.iter().cycle().skip(1))
                    .map(|(&(ax, ay), &(bx, by))| {
                        let (dx, dy) = (bx - ax, by - ay);
                        (dx * (v - ay) - dy * (u - ax)) / dx.hypot(dy)
                    })
                    .fold(f32::INFINITY, f32::min)
            };
            let grain = noise(index as u32);
            let edge = ((inside - 0.04) * 8.0 - grain * 0.65).clamp(0.0, 1.0);
            let ink = match self {
                Self::Dry => {
                    let bristle = noise(y as u32 + 1024);
                    let split = noise((x / 5 + y * SIZE + AREA) as u32);
                    if bristle < 0.23 || split < 0.13 {
                        0.0
                    } else {
                        (0.30 + 0.55 * bristle) * (0.70 + 0.30 * grain)
                    }
                }
                Self::Chalk => {
                    if grain < 0.35 {
                        0.0
                    } else {
                        0.18 + 0.58 * grain
                    }
                }
                Self::Charcoal => {
                    let scratch = noise((x * 17 + y * 9 + 40_003) as u32);
                    if scratch < 0.28 || grain < 0.13 { 0.0 }
                    else { (0.28 + 0.72 * grain) * (0.65 + 0.35 * scratch) }
                }
                Self::Pastel => {
                    let soft = (1.0 - u.hypot(v) * 0.38).clamp(0.20, 1.0);
                    if grain < 0.17 { 0.0 }
                    else { (0.34 + 0.48 * grain) * soft }
                }
            };
            // Keep taps visible even when the tip is smaller than a pixel.
            let ink = if u.abs() < 0.07 && v.abs() < 0.07 {
                ink.max(0.45)
            } else {
                ink
            };
            (255.0 * ink * edge).round() as u8
        })
    }

    fn stamps(self, stroke: &Stroke, mut emit: impl FnMut(Stamp)) {
        let (wide, tall) = match stroke.tip_shape {
            BrushTipShape::TallRectangle => (0.5, 1.5),
            BrushTipShape::WideRectangle => (1.5, 0.5),
            _ => (1.0, 1.0),
        };
        let mut ordinal = 0u32;
        visit_pressure_dabs(&stroke.points, stroke.base_width, |dab, [dx, dy]| {
            let pressure = (2.0 * dab.radius / stroke.base_width).clamp(0.0, 1.0);
            let variation = noise(ordinal.wrapping_add(3201));
            // Bristles follow the hand. Chalk rolls slightly about the center.
            let (sin, cos) = if matches!(self, Self::Chalk | Self::Charcoal | Self::Pastel) {
                ((variation - 0.5) * 0.8).sin_cos()
            } else {
                (0.0, 1.0)
            };
            let (tx, ty) = (dx * cos - dy * sin, dx * sin + dy * cos);
            let r = dab.radius;
            emit(Stamp {
                center: [dab.x, dab.y],
                along: [tx * r * wide, ty * r * wide],
                across: [-ty * r * tall, tx * r * tall],
                flow: (0.38 + 0.62 * pressure) * (0.72 + 0.28 * variation),
            });
            ordinal = ordinal.wrapping_add(1);
        });
    }

    pub fn preview(
        self,
        painter: &egui::Painter,
        stroke: &Stroke,
        rgba: [u8; 4],
        to_screen: impl Fn(f32, f32) -> egui::Pos2,
    ) {
        let ctx = painter.ctx();
        let key = egui::Id::new(("stick_brush_mask", self as u8, stroke.tip_shape as u8));
        let cached = ctx.data(|data| data.get_temp::<egui::TextureHandle>(key));
        let texture = cached.unwrap_or_else(|| {
            let pixels = self
                .mask(stroke.tip_shape)
                .iter()
                .map(|&a| egui::Color32::from_rgba_unmultiplied(255, 255, 255, a))
                .collect();
            let texture = ctx.load_texture(
                "brush tip",
                egui::ColorImage {
                    size: [SIZE, SIZE],
                    pixels,
                },
                egui::TextureOptions::LINEAR,
            );
            ctx.data_mut(|data| data.insert_temp(key, texture.clone()));
            texture
        });
        let mut mesh = egui::Mesh::with_texture(texture.id());
        self.stamps(stroke, |stamp| {
            let start = mesh.vertices.len() as u32;
            let color = egui::Color32::from_rgba_unmultiplied(
                rgba[0],
                rgba[1],
                rgba[2],
                (rgba[3] as f32 * stamp.flow).round() as u8,
            );
            for (u, v) in [(0., 0.), (1., 0.), (1., 1.), (0., 1.)] {
                let [x, y] = stamp.corner(u, v);
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: to_screen(x, y),
                    uv: egui::pos2(u, v),
                    color,
                });
            }
            mesh.indices
                .extend([start, start + 1, start + 2, start, start + 2, start + 3]);
        });
        painter.add(egui::Shape::mesh(mesh));
    }

    pub fn export(self, target: &mut Pixmap, stroke: &Stroke, alpha: u8, transform: Transform) {
        let mut tip = Pixmap::new(SIZE as u32, SIZE as u32).unwrap();
        for (px, &a) in tip
            .data_mut()
            .chunks_exact_mut(4)
            .zip(self.mask(stroke.tip_shape))
        {
            for (out, &c) in px[..3].iter_mut().zip(&stroke.color[..3]) {
                *out = ((c as u16 * a as u16 + 127) / 255) as u8;
            }
            px[3] = a;
        }
        self.stamps(stroke, |stamp| {
            let origin = stamp.corner(0., 0.);
            let k = 2.0 / SIZE as f32;
            let position = Transform::from_row(
                stamp.along[0] * k,
                stamp.along[1] * k,
                stamp.across[0] * k,
                stamp.across[1] * k,
                origin[0],
                origin[1],
            )
            .post_concat(transform);
            let paint = PixmapPaint {
                opacity: (alpha as f32 * stamp.flow).round() / 255.0,
                quality: FilterQuality::Bilinear,
                ..PixmapPaint::default()
            };
            target.draw_pixmap(0, 0, tip.as_ref(), &paint, position, None);
        });
    }
}

#[derive(Clone, Copy)]
struct Stamp {
    center: [f32; 2],
    along: [f32; 2],
    across: [f32; 2],
    flow: f32,
}

impl Stamp {
    fn corner(&self, u: f32, v: f32) -> [f32; 2] {
        let (u, v) = (2.0 * u - 1.0, 2.0 * v - 1.0);
        [
            self.center[0] + self.along[0] * u + self.across[0] * v,
            self.center[1] + self.along[1] * u + self.across[1] * v,
        ]
    }
}

// Integer avalanche hash. Fixed seeds reproduce the same paper and bristles
// after undo, zoom, export, and reopening a project.
fn noise(mut seed: u32) -> f32 {
    seed ^= seed >> 16;
    seed = seed.wrapping_mul(0x7feb352d);
    seed ^= seed >> 15;
    seed = seed.wrapping_mul(0x846ca68b);
    ((seed ^ (seed >> 16)) >> 8) as f32 / 16_777_216.0
}
