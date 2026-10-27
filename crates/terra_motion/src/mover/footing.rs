//! What a body may not step onto (D53, docs/controller.md "Step up"): Unreal's
//! `CanCharacterStepUpOn`. A thing of flora taller than the kind's step (a stump, a log, a
//! rock) is something to jump onto, never a step, however it is come at: by a lower piece of
//! it (a log's thin end), from a slope's high side, or by the round of its edge, which a field's
//! contact faces upward where a mesh's face would stand upright. A body whose feet are not up
//! on such a thing meets each of its contacts as a wall: the contact's normal is its level part,
//! as Unreal judges a hit by the face it struck (`ImpactNormal`), not the capsule's round.

use super::query::{Capsule, Contact, Hit, Query};
use glam::{DVec3, Vec3};

/// A body's query, the things it may not step onto met as walls.
pub struct NoStep<'a> {
    pub query: &'a dyn Query,
    pub up: Vec3,
    pub step: f32,
}

impl<'a> NoStep<'a> {
    pub fn new(query: &'a dyn Query, up: Vec3, step: f32) -> Self {
        NoStep { query, up, step }
    }

    /// A contact with what the body may not step onto, its normal laid level.
    fn level(&self, capsule: &Capsule, contact: &mut Contact) {
        let rise = contact.normal.dot(self.up);
        if rise <= 1.0e-4 {
            return;
        }
        let feet = capsule.a - self.up * capsule.radius;
        let (at, feet) = (capsule.world(contact.at), capsule.world(feet));
        if self.query.steps_onto(at, feet, self.step) {
            return;
        }
        if let Some(level) = (contact.normal - self.up * rise).try_normalize() {
            contact.normal = level;
        }
    }
}

impl Query for NoStep<'_> {
    fn contacts(&self, capsule: &Capsule, within: f32, out: &mut Vec<Contact>) {
        let from = out.len();
        self.query.contacts(capsule, within, out);
        for contact in &mut out[from..] {
            self.level(capsule, contact);
        }
    }

    fn sweep(&self, capsule: &Capsule, motion: Vec3) -> Option<Hit> {
        let mut hit = self.query.sweep(capsule, motion)?;
        self.level(&capsule.moved(motion * hit.fraction), &mut hit.contact);
        Some(hit)
    }

    fn apart(&self, capsule: &Capsule, within: f32) -> Option<f32> {
        self.query.apart(capsule, within)
    }

    fn steps_onto(&self, at: DVec3, feet: DVec3, step: f32) -> bool {
        self.query.steps_onto(at, feet, step)
    }

    fn grip(&self, at: DVec3) -> f32 {
        self.query.grip(at)
    }
}
