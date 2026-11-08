//! The world's clock across the network (docs/netcode.md; D540: "nothing travels but the
//! clock and the seed"): the sky, the weather and the seasons are each game's own work from
//! the seed and the clock, so a joined game's clock must be the host's. The host says its
//! clock every two seconds where each player stands, as a thing that happens, not a record
//! kept: the day, the hour and how fast the clock runs; a joined game takes it as its own.
//! Found by the two-player walk (`tools/sandbox/land.ts`): a joined game's clock ran from
//! its own start, half an hour of the day behind its host's.

use super::link::Link;
use super::{Host, ROOM};
use crate::Player;
use bevy::prelude::*;
use terra_net::client::Event as Heard;
use terra_world::daylight::{Clock, PACE};

/// The first byte of the clock as the host says it.
pub const CLOCK: u8 = 3;
/// How often the host says it, in ticks: every two seconds.
const EVERY_TICKS: u32 = 128;
/// A joined game's clock is set to the host's when it is this far off, in hours: nearer,
/// it is left to run, so the sun does not twitch at every word.
const OFF_H: f32 = 0.02;

/// The clock as it crosses: the day, the hour, and how many times as fast as play it runs.
pub fn said(clock: &Clock, pace: f32) -> Vec<u8> {
    let mut bytes = vec![CLOCK];
    bytes.extend(clock.day.to_le_bytes());
    bytes.extend(clock.hour.to_le_bytes());
    bytes.extend(pace.to_le_bytes());
    bytes
}

/// The day, hour and pace some bytes say, if they are a clock's.
pub fn heard(bytes: &[u8]) -> Option<(i64, f32, f32)> {
    let (&CLOCK, rest) = bytes.split_first()? else {
        return None;
    };
    let rest: [u8; 16] = rest.try_into().ok()?;
    let day = i64::from_le_bytes(rest[..8].try_into().unwrap());
    let hour = f32::from_le_bytes(rest[8..12].try_into().unwrap());
    let pace = f32::from_le_bytes(rest[12..].try_into().unwrap());
    ((0.0..24.0).contains(&hour) && (0.0..=60.0).contains(&pace)).then_some((day, hour, pace))
}

/// On the host: its clock, said where each player stands, every two seconds.
pub fn tell(
    mut host: ResMut<Host>,
    clock: Option<Res<Clock>>,
    movers: Res<crate::movement::Movers>,
    players: Query<Entity, With<Player>>,
    mut since: Local<u32>,
) {
    *since += 1;
    let Some(clock) = clock.filter(|_| *since >= EVERY_TICKS) else {
        return;
    };
    *since = 0;
    let bytes = said(&clock, PACE.get());
    // Where each body is as the mover has it, which is where the server has it: said
    // there, it is near whoever's body it is.
    for body in players.iter().filter_map(|player| movers.body(player)) {
        host.server.happen(ROOM, body.position.to_array(), &bytes);
    }
}

/// On a joined game: the host's clock is this game's.
pub fn take(mut link: ResMut<Link>, clock: Option<ResMut<Clock>>) {
    let (clocks, rest): (Vec<Heard>, Vec<Heard>) =
        std::mem::take(&mut link.events).into_iter().partition(
            |event| matches!(event, Heard::Happened { bytes, .. } if bytes.first() == Some(&CLOCK)),
        );
    link.events = rest;
    let (Some(mut clock), Some(Heard::Happened { bytes, .. })) = (clock, clocks.last()) else {
        return;
    };
    let Some((day, hour, pace)) = heard(bytes) else {
        return;
    };
    if PACE.get() != pace {
        let _ = PACE.set(pace, terra_gen::tune::SetBy::Remote);
    }
    // Round the day's end the hours are a day apart and near all the same.
    let off = (clock.hour - hour).abs();
    if clock.day != day && off < 23.0 || off.min(24.0 - off) > OFF_H {
        (clock.day, clock.hour) = (day, hour);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clock_crosses_whole_and_nonsense_is_no_clock() {
        let clock = Clock {
            day: 412,
            hour: 17.25,
        };
        assert_eq!(heard(&said(&clock, 60.0)), Some((412, 17.25, 60.0)));
        assert_eq!(heard(&[CLOCK, 1, 2]), None);
        let mut late = said(&clock, 1.0);
        late[9..13].copy_from_slice(&31.0f32.to_le_bytes());
        assert_eq!(heard(&late), None, "an hour past the day's end is no hour");
        assert_eq!(heard(&said(&clock, 1.0)[1..]), None);
    }
}
