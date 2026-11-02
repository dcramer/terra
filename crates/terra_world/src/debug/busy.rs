//! How busy the GPU was over the log's last minute: the GPU's time summed over the frames,
//! a share of the time they took. A GPU's time per frame is only its cost while it is busy:
//! one left idle most of each frame, as the game holding frames back leaves it, lowers its
//! clocks, and the same frame's passes then take longer (play at the lake, 2026-11-02: the
//! GPU's median rose 26.6 to 53.9 ms as the frames fell 644 to 237 a minute, while its busy
//! time a minute fell 17.1 to 12.8 s, so it was drawing no more than before).

use super::Sample;

/// The clause the minute's line ends with: the GPU's busy share, and when it idled over half
/// the time what that means for its times. None when the GPU was not timed.
pub(super) fn clause(samples: &[Sample]) -> Option<String> {
    let busy_ms: f32 = samples.iter().filter_map(|s| s.gpu_ms).sum();
    let frames_ms: f32 = samples
        .iter()
        .filter(|s| s.gpu_ms.is_some())
        .map(|s| s.frame_ms)
        .sum();
    if busy_ms <= 0.0 || frames_ms <= 0.0 {
        return None;
    }
    let share = 100.0 * (busy_ms / frames_ms).min(1.0);
    Some(if share < 50.0 {
        format!(
            " The GPU was busy {share:.0}% of the time: idle the rest, it may have lowered its clocks, so its times read longer than its work; the frames were held back by the game's work, vsync or a frame limit, not by the GPU."
        )
    } else {
        format!(" The GPU was busy {share:.0}% of the time.")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn sample(frame_ms: f32, gpu_ms: Option<f32>) -> Sample {
        Sample {
            at: Instant::now(),
            frame_ms,
            game_ms: 0.0,
            render_ms: None,
            gpu_ms,
        }
    }

    #[test]
    fn a_gpu_idle_most_of_the_time_is_said_to_read_long() {
        // Frames of 200 ms, the GPU 40 ms of each: busy a fifth of the time.
        let slow = vec![sample(200.0, Some(40.0)); 10];
        let said = clause(&slow).unwrap();
        assert!(
            said.starts_with(" The GPU was busy 20% of the time: idle"),
            "{said}"
        );
        // Frames the GPU holds back: busy all the time, nothing more to say.
        let bound = vec![sample(12.0, Some(11.5)); 10];
        assert_eq!(
            clause(&bound).unwrap(),
            " The GPU was busy 96% of the time."
        );
        // Untimed, as on macOS.
        assert_eq!(clause(&[sample(10.0, None)]), None);
    }
}
