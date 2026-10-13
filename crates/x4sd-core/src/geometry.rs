//! Transforms and the snap math, verified against the game's own plans
//! (see `docs/x4-construction-plans.md`, "Snap math").
//!
//! A child module attached by its snap `c` to snap `p` of its predecessor sits at
//! `pred · p · Ry(180°) · Rz(twist) · c⁻¹`: the two snap points coincide, the
//! snaps face each other (snap axis = local +Z), and `twist` is free.

use crate::plan::{Offset, Orientation};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quat {
    pub w: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Quat {
    pub const IDENTITY: Quat = Quat { w: 1.0, x: 0.0, y: 0.0, z: 0.0 };

    pub fn axis_angle(axis: Axis, degrees: f64) -> Quat {
        let (s, c) = (degrees.to_radians() / 2.0).sin_cos();
        match axis {
            Axis::X => Quat { w: c, x: s, y: 0.0, z: 0.0 },
            Axis::Y => Quat { w: c, x: 0.0, y: s, z: 0.0 },
            Axis::Z => Quat { w: c, x: 0.0, y: 0.0, z: s },
        }
    }

    /// The game's `<rotation yaw pitch roll>` convention: `Ry(yaw) · Rx(−pitch) · Rz(−roll)`.
    pub fn from_euler(yaw: f64, pitch: f64, roll: f64) -> Quat {
        Quat::axis_angle(Axis::Y, yaw) * Quat::axis_angle(Axis::X, -pitch) * Quat::axis_angle(Axis::Z, -roll)
    }

    pub fn conj(self) -> Quat {
        Quat { w: self.w, x: -self.x, y: -self.y, z: -self.z }
    }

    pub fn rotate(self, v: [f64; 3]) -> [f64; 3] {
        let r = self * Quat { w: 0.0, x: v[0], y: v[1], z: v[2] } * self.conj();
        [r.x, r.y, r.z]
    }

    fn normalized(self) -> Quat {
        let n = (self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        Quat { w: self.w / n, x: self.x / n, y: self.y / n, z: self.z / n }
    }
}

/// Hamilton product.
impl std::ops::Mul for Quat {
    type Output = Quat;

    fn mul(self, o: Quat) -> Quat {
        Quat {
            w: self.w * o.w - self.x * o.x - self.y * o.y - self.z * o.z,
            x: self.w * o.x + self.x * o.w + self.y * o.z - self.z * o.y,
            y: self.w * o.y - self.x * o.z + self.y * o.w + self.z * o.x,
            z: self.w * o.z + self.x * o.y - self.y * o.x + self.z * o.w,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub position: [f64; 3],
    pub rotation: Quat,
}

impl Transform {
    pub const IDENTITY: Transform = Transform { position: [0.0; 3], rotation: Quat::IDENTITY };

    pub fn rotation(rotation: Quat) -> Transform {
        Transform { position: [0.0; 3], rotation }
    }

    /// `self ∘ local`: `local` expressed in `self`'s frame.
    pub fn then(&self, local: &Transform) -> Transform {
        let p = self.rotation.rotate(local.position);
        Transform {
            position: [self.position[0] + p[0], self.position[1] + p[1], self.position[2] + p[2]],
            rotation: self.rotation * local.rotation,
        }
    }

    pub fn inverse(&self) -> Transform {
        let r = self.rotation.conj();
        let p = r.rotate(self.position);
        Transform { position: [-p[0], -p[1], -p[2]], rotation: r }
    }

    /// The transform a plan `<offset>` describes; no offset means identity.
    pub fn from_offset(offset: Option<&Offset>) -> Transform {
        let Some(o) = offset else { return Transform::IDENTITY };
        let rotation = match o.orientation {
            Orientation::None => Quat::IDENTITY,
            Orientation::Euler { yaw, pitch, roll } => Quat::from_euler(yaw.into(), pitch.into(), roll.into()),
            Orientation::Quaternion { x, y, z, w } => Quat { w: w.into(), x: x.into(), y: y.into(), z: z.into() },
        };
        Transform { position: o.position.map(f64::from), rotation }
    }

    /// A plan `<offset>`. The orientation is written as a quaternion (as the game
    /// does for non-trivial rotations), or omitted when it is the identity.
    pub fn to_offset(&self) -> Offset {
        let mut q = self.rotation.normalized();
        if q.w < 0.0 {
            q = Quat { w: -q.w, x: -q.x, y: -q.y, z: -q.z };
        }
        let orientation = if 1.0 - q.w < 1e-12 {
            Orientation::None
        } else {
            Orientation::Quaternion { x: q.x as f32, y: q.y as f32, z: q.z as f32, w: q.w as f32 }
        };
        Offset { position: self.position.map(|v| v as f32), orientation }
    }
}

/// Where a child goes when its snap `child_snap` attaches to snap `pred_snap` of
/// the module at `pred`, twisted by `twist_degrees` around the snap axis.
/// Snaps are given in their module's local frame.
pub fn attach(pred: &Transform, pred_snap: &Transform, child_snap: &Transform, twist_degrees: f64) -> Transform {
    let face = Quat::axis_angle(Axis::Y, 180.0) * Quat::axis_angle(Axis::Z, twist_degrees);
    pred.then(pred_snap).then(&Transform::rotation(face)).then(&child_snap.inverse())
}

/// How well a placed child actually sits on its predecessor's snap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fit {
    /// Distance between the two snap points, in metres.
    pub gap: f64,
    /// Angle between the child's snap axis and the reverse of the predecessor's, in degrees.
    pub misalignment: f64,
    /// Twist around the snap axis, in degrees (0..360); meaningful when aligned.
    pub twist: f64,
}

pub fn fit(pred: &Transform, pred_snap: &Transform, child: &Transform, child_snap: &Transform) -> Fit {
    let a = pred.then(pred_snap);
    let b = child.then(child_snap);
    let gap = (0..3).map(|i| (a.position[i] - b.position[i]).powi(2)).sum::<f64>().sqrt();

    // Angle between the predecessor's snap axis and the reversed child snap axis
    // (atan2 form: acos loses precision near 0°).
    let za = a.rotation.rotate([0.0, 0.0, 1.0]);
    let zb = b.rotation.rotate([0.0, 0.0, -1.0]);
    let cross = [za[1] * zb[2] - za[2] * zb[1], za[2] * zb[0] - za[0] * zb[2], za[0] * zb[1] - za[1] * zb[0]];
    let sin = (cross[0].powi(2) + cross[1].powi(2) + cross[2].powi(2)).sqrt();
    let cos = za[0] * zb[0] + za[1] * zb[1] + za[2] * zb[2];
    let misalignment = sin.atan2(cos).to_degrees();

    let d = (a.rotation * Quat::axis_angle(Axis::Y, 180.0)).conj() * b.rotation;
    let twist = (2.0 * d.z.atan2(d.w)).to_degrees().rem_euclid(360.0);
    Fit { gap, misalignment, twist }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f64; 3], b: [f64; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-6)
    }

    #[test]
    fn euler_convention_matches_game() {
        // yaw 90 turns +Z into +X (Y up, left-handed); pitch and roll are negated.
        assert!(close(Quat::from_euler(90.0, 0.0, 0.0).rotate([0.0, 0.0, 1.0]), [1.0, 0.0, 0.0]));
        assert!(close(Quat::from_euler(0.0, 90.0, 0.0).rotate([0.0, 0.0, 1.0]), [0.0, 1.0, 0.0]));
    }

    #[test]
    fn attach_then_fit_round_trips() {
        // struct_arg_base_02: snaps at z = +400 (facing +Z) and z = -400 (facing -Z).
        let s1 = Transform { position: [0.0, 0.0, 400.0], rotation: Quat::IDENTITY };
        let s2 = Transform { position: [0.0, 0.0, -400.0], rotation: Quat::axis_angle(Axis::Y, 180.0) };
        let pred = Transform { position: [10.0, 20.0, 30.0], rotation: Quat::from_euler(30.0, 0.0, 0.0) };
        let child = attach(&pred, &s1, &s2, 90.0);
        let f = fit(&pred, &s1, &child, &s2);
        assert!(f.gap < 1e-9 && f.misalignment < 1e-6, "{f:?}");
        assert!((f.twist - 90.0).abs() < 1e-6, "{f:?}");
        // Straight through: the child continues 800 m along the predecessor's axis.
        let straight = attach(&Transform::IDENTITY, &s1, &s2, 0.0);
        assert!(close(straight.position, [0.0, 0.0, 800.0]));
    }
}
