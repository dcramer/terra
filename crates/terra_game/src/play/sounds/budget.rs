//! How many of one animal voice are heard at once (D657,docs/audio.md).
//!
//! Twenty-four chaffinches a hectare each singing every 6 to 20 s is a song every second or
//! so within earshot, and every one played is a wall of the same few takes. Shipped games do
//! not voice every animal: each sound has a concurrency group with a most at once and a rule
//! for the one over (Unreal's Sound Concurrency: Max Count, "Stop Farthest Then Prevent
//! New", Retrigger Time,
//! <https://dev.epicgames.com/documentation/en-us/unreal-engine/sound-concurrency-reference-guide>;
//! Wwise's playback limit and FMOD's Max Instances with stealing are the same idea). Ours,
//! since a call is a second or two long and never stopped once begun: of one voice, at most
//! `MOST` begin in any `SPAN_S` seconds, the nearest of a frame first and the rest not
//! played; and one near at hand (within its clip's reach, where it is as loud as it gets) is
//! budgeted apart, so the bird put up at a walker's feet is heard however many call far off.
//! The place's chorus (`play::soundscape`) carries the rest.

/// At most so many of one voice begin from afar in any span of so many seconds, and as many
/// again from near at hand. Guesses (D654) until a walk is listened to: a call lasting one
/// to two seconds, two in six leaves a voice sounding under half the time.
pub const MOST: usize = 2;
pub const SPAN_S: f32 = 6.0;

/// The voices begun lately: each one's name, when, and whether it was near at hand.
#[derive(Default)]
pub struct Budget(Vec<(&'static str, f32, bool)>);

impl Budget {
    /// Whether one more of a voice may begin now, and if so counts it. A far one waits on
    /// all of its voice begun within the span, a near one only on the near.
    pub fn lets(&mut self, name: &'static str, now_s: f32, near: bool) -> bool {
        self.0.retain(|&(_, began, _)| now_s - began < SPAN_S);
        let begun = self
            .0
            .iter()
            .filter(|&&(voice, _, was_near)| voice == name && (was_near || !near))
            .count();
        if begun >= MOST {
            return false;
        }
        self.0.push((name, now_s, near));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How many of a voice asked for every so often over a minute are let begin.
    fn let_in_a_minute(budget: &mut Budget, name: &'static str, every_s: f32, near: bool) -> usize {
        let asked = (60.0 / every_s) as usize;
        (0..asked)
            .filter(|&i| budget.lets(name, i as f32 * every_s, near))
            .count()
    }

    #[test]
    fn a_voice_asked_for_every_half_second_is_heard_twice_in_six() {
        let per_minute = (60.0 / SPAN_S) as usize * MOST;
        // The wood of 2026-11-10: a chaffinch's song asked for 100 times a minute.
        let heard = let_in_a_minute(&mut Budget::default(), "song", 0.6, false);
        assert_eq!(heard, per_minute);
        // One asked for less often than the budget is all heard.
        assert_eq!(
            let_in_a_minute(&mut Budget::default(), "song", 7.0, false),
            8
        );
        // Another voice has its own budget.
        let mut both = Budget::default();
        assert_eq!(let_in_a_minute(&mut both, "song", 0.6, false), per_minute);
        assert_eq!(let_in_a_minute(&mut both, "caw", 0.6, false), per_minute);
    }

    #[test]
    fn one_near_at_hand_is_heard_however_many_call_far_off() {
        let mut budget = Budget::default();
        assert!(budget.lets("alarm", 0.0, false));
        assert!(budget.lets("alarm", 0.1, false));
        assert!(!budget.lets("alarm", 0.2, false));
        assert!(budget.lets("alarm", 0.3, true));
        assert!(budget.lets("alarm", 0.4, true));
        assert!(!budget.lets("alarm", 0.5, true));
        // Near ones fill the far ones' budget too: four in a span at the very most.
        assert!(!budget.lets("alarm", 0.6, false));
        assert!(budget.lets("alarm", 0.4 + SPAN_S, true));
    }
}
