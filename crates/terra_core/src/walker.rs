//! The size of something walking: what the world's flora and a base's floors both ask of a
//! body, so neither names the other (D407).

/// The size of something walking through a base.
#[derive(Clone, Copy, Debug)]
pub struct Walker {
    pub radius: f32,
    pub height: f32,
    /// The tallest step that can be walked up without jumping.
    pub step_up: f32,
}

impl Default for Walker {
    fn default() -> Self {
        Walker {
            radius: 0.3,
            height: 1.8,
            step_up: 0.4,
        }
    }
}
