//! Things that take time: you press to start, hold to carry on, and it is done when you
//! have held for long enough. Letting go early leaves nothing done.
//!
//! Building, taking apart and cutting all work this way. How long each takes is data
//! beside the thing itself (what it costs to build, how tough it is to cut), and the
//! tool in hand can make it quicker or slower.

/// One task being held down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hold {
    done: f32,
    needed: f32,
}

impl Hold {
    /// A task that takes so many seconds of holding.
    pub fn new(seconds: f32) -> Self {
        Hold {
            done: 0.0,
            needed: seconds.max(0.0),
        }
    }

    /// Holds for a little longer. Returns whether that finished it.
    pub fn advance(&mut self, seconds: f32) -> bool {
        self.done += seconds.max(0.0);
        self.done >= self.needed
    }

    /// How far along it is, from 0 to 1.
    pub fn fraction(&self) -> f32 {
        if self.needed <= 0.0 {
            1.0
        } else {
            (self.done / self.needed).clamp(0.0, 1.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_task_is_done_once_it_has_been_held_long_enough() {
        let mut hold = Hold::new(1.0);
        assert_eq!(hold.fraction(), 0.0);
        assert!(!hold.advance(0.4));
        assert!((hold.fraction() - 0.4).abs() < 1e-6);
        assert!(!hold.advance(0.5));
        assert!(hold.advance(0.2));
        assert_eq!(hold.fraction(), 1.0);
    }

    #[test]
    fn a_task_that_takes_no_time_is_done_at_once() {
        let mut hold = Hold::new(0.0);
        assert_eq!(hold.fraction(), 1.0);
        assert!(hold.advance(0.0));
        // Time never runs backwards.
        let mut slow = Hold::new(1.0);
        assert!(!slow.advance(-5.0));
        assert_eq!(slow.fraction(), 0.0);
    }
}
