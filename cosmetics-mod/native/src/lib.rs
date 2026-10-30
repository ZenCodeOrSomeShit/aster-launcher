//! Aster's original cosmetics engine. Fabric owns JVM hooks; all procedural sky
//! geometry and HUD placement run here in Rust. The JNI layer returns plain vertices.
use jni::{
    objects::JClass,
    sys::{jboolean, jdouble, jfloat, jfloatArray, jint, jintArray},
    JNIEnv,
};
use std::sync::OnceLock;

#[no_mangle]
pub extern "system" fn Java_dev_aster_cosmetics_NativeCosmetics_advancedSky(env:JNIEnv,_class:JClass,time:jdouble,density:jdouble,frequency:jdouble,size:jdouble,preset:jint,meteors:jboolean,reduced:jboolean,sky:jint,stars_color:jint,meteor_color:jint,bodies:jni::objects::JFloatArray)->jfloatArray {
    let rgba=|c:i32|[(c>>16&255)as f32,(c>>8&255)as f32,(c&255)as f32,255.];
    let mut out=Vec::new();let s=150.;
    for face in [[V(-s,-s,-s),V(s,-s,-s),V(s,s,-s),V(-s,s,-s)],[V(-s,-s,s),V(s,-s,s),V(s,s,s),V(-s,s,s)],[V(-s,-s,-s),V(-s,-s,s),V(-s,s,s),V(-s,s,-s)],[V(s,-s,-s),V(s,-s,s),V(s,s,s),V(s,s,-s)],[V(-s,s,-s),V(s,s,-s),V(s,s,s),V(-s,s,s)],[V(-s,-s,-s),V(s,-s,-s),V(s,-s,s),V(-s,-s,s)]]{quad(&mut out,face,rgba(sky));}
    let mut stars=sky_geometry(time,1.,density,0.,preset,false,reduced!=0);
    for v in stars.chunks_exact_mut(7){v[3..6].copy_from_slice(&rgba(stars_color)[..3]);}out.extend(stars);
    let mut streaks=sky_geometry(time,1.,0.,frequency,0,meteors!=0,reduced!=0);
    for group in streaks.chunks_exact_mut(28){let cx=(group[0]+group[7]+group[14]+group[21])/4.;let cy=(group[1]+group[8]+group[15]+group[22])/4.;let cz=(group[2]+group[9]+group[16]+group[23])/4.;for v in group.chunks_exact_mut(7){v[0]=cx+(v[0]-cx)*size.clamp(0.2,5.)as f32;v[1]=cy+(v[1]-cy)*size.clamp(0.2,5.)as f32;v[2]=cz+(v[2]-cz)*size.clamp(0.2,5.)as f32;v[3..6].copy_from_slice(&rgba(meteor_color)[..3]);}}out.extend(streaks);
    let len=env.get_array_length(&bodies).unwrap_or(0).min(384);let mut values=vec![0.;len as usize];let _=env.get_float_array_region(&bodies,0,&mut values);
    for b in values.chunks_exact(6){if !b.iter().all(|v|v.is_finite()){continue}let a=(b[0]as f64).to_radians();let e=(b[1]as f64).to_radians();let n=V(a.cos()*e.cos(),e.sin(),a.sin()*e.cos());let c=n.mul(95.);let r=n.cross(V(0.,1.,0.)).norm().mul(b[2].clamp(0.1,30.)as f64);let u=n.cross(r).norm().mul(b[2].clamp(0.1,30.)as f64);quad(&mut out,[c.sub(r).sub(u),c.add(r).sub(u),c.add(r).add(u),c.sub(r).add(u)],[b[3],b[4],b[5],255.]);}
    let Ok(array)=env.new_float_array(out.len()as i32)else{return std::ptr::null_mut()};let _=env.set_float_array_region(&array,0,&out);array.into_raw()
}

#[derive(Clone, Copy, Debug)]
struct V(f64, f64, f64);
impl V {
    fn add(self, b: V) -> V {
        V(self.0 + b.0, self.1 + b.1, self.2 + b.2)
    }
    fn sub(self, b: V) -> V {
        V(self.0 - b.0, self.1 - b.1, self.2 - b.2)
    }
    fn mul(self, n: f64) -> V {
        V(self.0 * n, self.1 * n, self.2 * n)
    }
    fn cross(self, b: V) -> V {
        V(
            self.1 * b.2 - self.2 * b.1,
            self.2 * b.0 - self.0 * b.2,
            self.0 * b.1 - self.1 * b.0,
        )
    }
    fn norm(self) -> V {
        let length = (self.0 * self.0 + self.1 * self.1 + self.2 * self.2).sqrt();
        if length < 1e-10 {
            V(1., 0., 0.)
        } else {
            self.mul(1. / length)
        }
    }
}
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / ((1u64 << 53) as f64)
    }
}
struct Star {
    p: V,
    r: V,
    u: V,
    brightness: f64,
}
fn stars() -> &'static Vec<Star> {
    static STARS: OnceLock<Vec<Star>> = OnceLock::new();
    STARS.get_or_init(|| {
        let mut random = Rng(0xA57E2);
        (0..720)
            .map(|_| {
                let n = V(
                    random.next() * 2. - 1.,
                    random.next() * 0.93 + 0.07,
                    random.next() * 2. - 1.,
                )
                .norm();
                let size = 0.055 + random.next() * 0.10;
                let r = n.cross(V(0., 1., 0.)).norm().mul(size);
                Star {
                    p: n.mul(100.),
                    r,
                    u: n.cross(r).norm().mul(size),
                    brightness: 0.35 + random.next() * 0.65,
                }
            })
            .collect()
    })
}
fn vertex(out: &mut Vec<f32>, p: V, color: [f32; 4]) {
    out.extend_from_slice(&[
        p.0 as f32, p.1 as f32, p.2 as f32, color[0], color[1], color[2], color[3],
    ]);
}
fn quad(out: &mut Vec<f32>, p: [V; 4], color: [f32; 4]) {
    for v in p {
        vertex(out, v, color);
    }
}

fn sky_geometry(
    time: f64,
    night: f64,
    density: f64,
    frequency: f64,
    preset: i32,
    meteors: bool,
    reduced: bool,
) -> Vec<f32> {
    if !time.is_finite() || !night.is_finite() {
        return Vec::new();
    }
    let night = night.clamp(0., 1.);
    let density = if density.is_finite() {
        density.clamp(0., 1.)
    } else {
        0.
    };
    let frequency = if frequency.is_finite() {
        frequency.clamp(0., 1.)
    } else {
        0.
    };
    let mut out = Vec::with_capacity(32_000);
    if preset != 0 {
        for s in stars().iter().take((720. * density) as usize) {
            quad(
                &mut out,
                [
                    s.p.sub(s.r).sub(s.u),
                    s.p.add(s.r).sub(s.u),
                    s.p.add(s.r).add(s.u),
                    s.p.sub(s.r).add(s.u),
                ],
                [193., 211., 255., (night * s.brightness * 220.) as f32],
            );
        }
        if preset == 2 {
            for layer in 0..5 {
                for i in 0..64 {
                    let angle = i as f64 * std::f64::consts::TAU / 64.;
                    let next = (i + 1) as f64 * std::f64::consts::TAU / 64.;
                    let drift = if reduced { 0. } else { time * 0.025 };
                    let y = 34. + (angle * 3. + drift).sin() * 5. + layer as f64 * 2.;
                    let y2 = 34. + (next * 3. + drift).sin() * 5. + layer as f64 * 2.;
                    quad(
                        &mut out,
                        [
                            V(angle.cos() * 90., y, angle.sin() * 90.),
                            V(next.cos() * 90., y2, next.sin() * 90.),
                            V(next.cos() * 90., y2 + 7., next.sin() * 90.),
                            V(angle.cos() * 90., y + 7., angle.sin() * 90.),
                        ],
                        [
                            (65 + layer * 18) as f32,
                            (170 - layer * 8) as f32,
                            (155 + layer * 16) as f32,
                            (night * (8 - layer) as f64) as f32,
                        ],
                    );
                }
            }
        }
    }
    if meteors && !reduced && frequency > 0. {
        let period = 8. / (0.15 + frequency * 2.7);
        let epoch = (time / period).floor();
        let age = time - epoch * period;
        if age < 1.4 {
            let mut seed = Rng((epoch as u64).wrapping_mul(97471).wrapping_add(8619));
            let azimuth = seed.next() * std::f64::consts::TAU;
            let start = V(
                azimuth.cos() * 60.,
                75. + seed.next() * 15.,
                azimuth.sin() * 60.,
            );
            let velocity = V(-azimuth.sin() * 32., -14., azimuth.cos() * 32.);
            for i in 0..32 {
                let head = age - i as f64 * 0.015;
                let tail = head - 0.022;
                if head < 0. {
                    continue;
                }
                let p = start.add(velocity.mul(head));
                let q = start.add(velocity.mul(tail.max(0.)));
                let side = p
                    .norm()
                    .cross(velocity)
                    .norm()
                    .mul(0.095 * (1. - i as f64 / 36.));
                let fade =
                    ((age / 1.4).min(1.) * std::f64::consts::PI).sin() * (1. - i as f64 / 32.);
                quad(
                    &mut out,
                    [p.add(side), p.sub(side), q.sub(side), q.add(side)],
                    [215., 230., 255., (night * fade * 255.) as f32],
                );
            }
        }
    }
    out
}

fn widget_bounds(w: i32, h: i32, base_w: i32, base_h: i32, x: f64, y: f64, scale: f64) -> [i32; 4] {
    let scale = if scale.is_finite() {
        scale.clamp(0.5, 2.)
    } else {
        1.
    };
    let bw = (base_w as f64 * scale).ceil() as i32;
    let bh = (base_h as f64 * scale).ceil() as i32;
    let x = if x.is_finite() { x.clamp(0., 1.) } else { 0. };
    let y = if y.is_finite() { y.clamp(0., 1.) } else { 0. };
    [
        (x * (w - bw).max(0) as f64) as i32,
        (y * (h - bh).max(0) as f64) as i32,
        bw,
        bh,
    ]
}

#[no_mangle]
pub extern "system" fn Java_dev_aster_cosmetics_NativeCosmetics_skyVertices(
    env: JNIEnv,
    _class: JClass,
    time: jdouble,
    night: jfloat,
    density: jdouble,
    frequency: jdouble,
    preset: jint,
    meteors: jboolean,
    reduced: jboolean,
) -> jfloatArray {
    let vertices = sky_geometry(
        time,
        night as f64,
        density,
        frequency,
        preset,
        meteors != 0,
        reduced != 0,
    );
    let Ok(array) = env.new_float_array(vertices.len() as i32) else {
        return std::ptr::null_mut();
    };
    if env.set_float_array_region(&array, 0, &vertices).is_err() {
        return std::ptr::null_mut();
    }
    array.into_raw()
}

#[no_mangle]
pub extern "system" fn Java_dev_aster_cosmetics_NativeCosmetics_widgetBounds(
    env: JNIEnv,
    _class: JClass,
    w: jint,
    h: jint,
    bw: jint,
    bh: jint,
    x: jdouble,
    y: jdouble,
    scale: jdouble,
) -> jintArray {
    let bounds = widget_bounds(w, h, bw, bh, x, y, scale);
    let Ok(array) = env.new_int_array(4) else {
        return std::ptr::null_mut();
    };
    if env.set_int_array_region(&array, 0, &bounds).is_err() {
        return std::ptr::null_mut();
    }
    array.into_raw()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stars_are_deterministic_and_have_expected_vertex_count() {
        let a = sky_geometry(10., 1., 0.5, 0., 1, false, false);
        let b = sky_geometry(10., 1., 0.5, 0., 1, false, false);
        assert_eq!(a, b);
        assert_eq!(a.len(), 360 * 4 * 7);
        assert!(a.iter().all(|v| v.is_finite()));
    }
    #[test]
    fn reduced_motion_disables_meteors_and_freezes_ribbon() {
        assert!(sky_geometry(0.5, 1., 0., 1., 0, true, true).is_empty());
        assert_eq!(
            sky_geometry(0., 1., 0., 1., 2, true, true),
            sky_geometry(5., 1., 0., 1., 2, true, true)
        );
        assert!(!sky_geometry(0.5, 1., 0., 1., 0, true, false).is_empty());
    }
    #[test]
    fn density_and_hud_positions_are_bounded() {
        assert_eq!(
            sky_geometry(10., 1., 20., 0., 1, false, false).len(),
            720 * 4 * 7
        );
        assert_eq!(
            widget_bounds(1000, 600, 80, 24, 1., 1., 1.),
            [920, 576, 80, 24]
        );
        assert_eq!(
            widget_bounds(1000, 600, 80, 24, -5., f64::NAN, 1.),
            [0, 0, 80, 24]
        );
    }
    #[test]
    fn malformed_time_does_not_reach_gpu() {
        assert!(sky_geometry(f64::NAN, 1., 1., 1., 1, true, false).is_empty());
    }
}
