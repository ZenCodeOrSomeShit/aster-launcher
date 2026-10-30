use crate::backend::SkyConfig;
use eframe::egui::{self, pos2, Color32, Pos2, Rect, Shape, Stroke, Vec2};

pub fn color(hex: &str) -> Color32 {
    let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0xb8c9ff);
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}
pub fn hex(c: Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}

// Every vertex is generated here on the CPU. This preview uses no image or shader pack.
pub fn draw(painter: &egui::Painter, r: Rect, sky: &SkyConfig, time: f64, reduced: bool) {
    let p = painter.with_clip_rect(r.intersect(painter.clip_rect()));
    let top = if sky.preset == "vanilla" {
        Color32::from_rgb(34, 66, 91)
    } else {
        color(&sky.color)
    };
    let bottom = if sky.preset == "aurora" {
        Color32::from_rgb(106, 151, 146)
    } else {
        Color32::from_rgb(160, 144, 169)
    };
    let mut mesh = egui::Mesh::default();
    for (pos, col) in [
        (r.left_top(), top),
        (r.right_top(), top),
        (r.right_bottom(), bottom),
        (r.left_bottom(), bottom),
    ] {
        mesh.vertices.push(egui::epaint::Vertex {
            pos,
            uv: egui::epaint::WHITE_UV,
            color: col,
        });
    }
    mesh.indices = vec![0, 1, 2, 0, 2, 3];
    p.add(Shape::mesh(mesh));
    if sky.preset == "aurora" {
        for j in 0..3 {
            let mut path = vec![];
            for i in 0..80 {
                let x = i as f32 / 79.;
                let y = 0.15 + 0.065 * (x * 7. + j as f32).sin() + j as f32 * 0.035;
                path.push(pos2(r.left() + x * r.width(), r.top() + y * r.height()));
            }
            p.add(Shape::line(
                path,
                Stroke::new(15.0_f32, Color32::from_rgba_unmultiplied(127, 234, 211, 22)),
            ));
        }
    }
    for i in 0..(220. * sky.star_density) as usize {
        let x = hash(i as f32 * 127.1 + 31.);
        let y = hash(i as f32 * 311.7 + 17.) * 0.68;
        let a = if reduced {
            180
        } else {
            (145. + 60. * (time as f32 * 0.6 + i as f32).sin()) as u8
        };
        p.circle_filled(
            pos2(r.left() + x * r.width(), r.top() + y * r.height()),
            if i % 11 == 0 { 1.3 } else { 0.65 },
            color(&sky.star_color).gamma_multiply(a as f32 / 255.),
        );
    }
    for body in &sky.bodies {
        if !body.enabled {
            continue;
        }
        let x = (body.azimuth + 180.) / 360.;
        let y = 0.65 - body.elevation / 90. * 0.6;
        let size = body.size * 3.;
        p.rect_filled(
            Rect::from_center_size(
                pos2(r.left() + x * r.width(), r.top() + y * r.height()),
                Vec2::splat(size),
            ),
            0.,
            color(&body.color),
        );
    }
    if sky.shooting_stars && sky.meteor_frequency > 0. && !reduced {
        let cycle = 9. - sky.meteor_frequency as f64 * 6.;
        let phase = (time % cycle) / 1.2;
        if phase < 1. {
            let head = pos2(
                r.left() + r.width() * (0.43 + phase as f32 * 0.3),
                r.top() + r.height() * (0.1 + phase as f32 * 0.24),
            );
            for i in 0..24 {
                let a = i as f32 / 24.;
                let pt = head - Vec2::new(110., 48.) * (1. - a);
                p.line_segment(
                    [pt, pt + Vec2::new(5., 2.2)],
                    Stroke::new(sky.meteor_size, color(&sky.meteor_color).gamma_multiply(a)),
                );
            }
        }
    }
    ridge(&p, r, 0.91, 0.42, 16., Color32::from_rgb(69, 76, 104), 1.4);
    ridge(&p, r, 1., 0.36, 20., Color32::from_rgb(43, 61, 79), 3.7);
    ridge(&p, r, 1.1, 0.34, 24., Color32::from_rgb(28, 48, 60), 1.);
    for i in 0..22 {
        let x = r.left() + (i as f32 * 83. + 17.) % r.width();
        let y = r.top() + r.height() * (0.88 + 0.06 * (i as f32 * 4.).sin());
        let s = 5. + (i % 4) as f32 * 2.;
        p.rect_filled(
            Rect::from_min_size(pos2(x - 2., y - 22.), Vec2::new(4., 32.)),
            0.,
            Color32::from_rgb(20, 42, 47),
        );
        for k in 0..3 {
            p.rect_filled(
                Rect::from_min_size(
                    pos2(x - s + k as f32 * 2., y - 25. + k as f32 * 7.),
                    Vec2::new(s * 2. - k as f32 * 4., 8.),
                ),
                0.,
                Color32::from_rgb(20, 42, 47),
            );
        }
    }
}
fn hash(x: f32) -> f32 {
    (x.sin() * 43_758.547).fract().abs()
}
fn ridge(p: &egui::Painter, r: Rect, base: f32, amplitude: f32, step: f32, c: Color32, seed: f32) {
    // Triangulate each solid column instead of relying on a convex-polygon renderer for a concave ridge.
    let mut x = 0.;
    while x < r.width() {
        let y = r.height()
            * (base
                - amplitude
                    * (0.5
                        + 0.3 * (x / r.width() * 9. + seed).sin()
                        + 0.2 * (x / r.width() * 19. + seed).sin()));
        p.rect_filled(
            Rect::from_min_max(
                pos2(r.left() + x, r.top() + y),
                pos2((r.left() + x + step).min(r.right()), r.bottom()),
            ),
            0.,
            c,
        );
        x += step;
    }
}

pub fn star(p: &egui::Painter, center: Pos2, size: f32, c: Color32) {
    let path = vec![
        center + Vec2::new(0., -size),
        center + Vec2::new(size * 0.23, -size * 0.23),
        center + Vec2::new(size, 0.),
        center + Vec2::new(size * 0.23, size * 0.23),
        center + Vec2::new(0., size),
        center + Vec2::new(-size * 0.23, size * 0.23),
        center + Vec2::new(-size, 0.),
        center + Vec2::new(-size * 0.23, -size * 0.23),
    ];
    // Four convex kites make the concave star render consistently.
    for i in (0..8).step_by(2) {
        p.add(Shape::convex_polygon(
            vec![center, path[(i + 7) % 8], path[i], path[(i + 1) % 8]],
            c,
            Stroke::NONE,
        ));
    }
}
