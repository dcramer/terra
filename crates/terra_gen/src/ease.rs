//! Easing between two edges, as GLSL's and WGSL's `smoothstep` does, written once here so the
//! Rust that matches a shader, and the places that must agree with each other, use one
//! definition (D396).

/// 0 at or below `from`, 1 at or above `to`, and an S-curve between them with no slope at
/// either end (Hermite, `3t² − 2t³`). `from` may be above `to`, which runs the curve
/// downward.
pub fn smoothstep(from: f32, to: f32, value: f32) -> f32 {
    let t = ((value - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_flat_past_its_edges_and_even_about_its_middle() {
        assert_eq!(smoothstep(2.0, 4.0, 1.0), 0.0);
        assert_eq!(smoothstep(2.0, 4.0, 5.0), 1.0);
        assert_eq!(smoothstep(2.0, 4.0, 3.0), 0.5);
        // 3t² − 2t³ at t = 1/4 is 3/16 − 2/64 = 10/64.
        assert!((smoothstep(0.0, 4.0, 1.0) - 10.0 / 64.0).abs() < 1e-6);
        assert!((smoothstep(0.0, 4.0, 3.0) - (1.0 - 10.0 / 64.0)).abs() < 1e-6);
    }

    #[test]
    fn edges_given_high_to_low_run_it_downward() {
        assert_eq!(smoothstep(4.0, 2.0, 5.0), 0.0);
        assert_eq!(smoothstep(4.0, 2.0, 1.0), 1.0);
    }
}
