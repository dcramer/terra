//! Claims across the network (D645: anyone may build in any claim): every game is told
//! where each claim's stone stands and how far the claim has grown, so a builder is allowed
//! in a claim that is not theirs. A claim is state, sent as the most recent (D652): the host
//! tells each claim it knows, its own player's and each guest's, as a thing of its own kind
//! ([`CLAIM`]) whose one part says where its stone is and its step; a joined game asks the
//! host to hold its own player's ([`CLAIMED`]), and keeps what the host tells of the others'
//! among the claims it knows ([`Others`]).

use crate::catalogue::Builds;
use crate::claim::{Claim, Others};
use bevy::prelude::*;
use terra_net::proto::{EntityId, Kind};
use terra_net::transport::Peer;
use terra_walker::IsLocalPlayer;
use terra_walker::net::link::Link;
use terra_walker::net::{Host, tell};
use terra_world::tick::Sim;

/// The kind of a claim's entity.
pub const CLAIM: Kind = Kind(5);
/// The first byte of what a joined game asks of its own player's claim.
pub const CLAIMED: u8 = 26;
/// How often the claims are looked over, in ticks: twice a second.
const EVERY_TICKS: u32 = 32;

/// A claim as it crosses: where its stone stands, to the centimetre, and its step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Told {
    at: [i32; 3],
    step: u8,
}

impl Told {
    fn of(claim: &Claim) -> Option<Told> {
        let (_, at) = claim.stone?;
        Some(Told {
            at: at.to_array().map(|metres| (metres * 100.0).round() as i32),
            step: claim.step().min(255) as u8,
        })
    }

    fn said(&self) -> [u8; 13] {
        let mut bytes = [0; 13];
        for (i, metres) in self.at.iter().enumerate() {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&metres.to_le_bytes());
        }
        bytes[12] = self.step;
        bytes
    }

    /// The claim some bytes say, if they are one's whole and no more.
    fn heard(bytes: &[u8]) -> Option<Told> {
        let bytes: &[u8; 13] = bytes.try_into().ok()?;
        let at = [0, 4, 8]
            .map(|i| i32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]));
        Some(Told {
            at,
            step: bytes[12],
        })
    }

    fn claim(&self, builds: &Builds) -> Claim {
        Claim::shown(
            Vec3::from(self.at.map(|cm| cm as f32 / 100.0)),
            usize::from(self.step),
            &builds.claim,
        )
    }
}

/// On the host: each guest's claim as its game last said it, and what is told of each claim.
/// On a joined game: what it last said of its own.
#[derive(Resource, Default)]
struct Known {
    guests: Vec<(Peer, Option<Told>)>,
    put: Vec<(Told, EntityId)>,
    said: Option<Option<Told>>,
    since: u32,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Known>()
        .init_resource::<Others>()
        .add_systems(
            FixedUpdate,
            (
                host.before(tell).run_if(resource_exists::<Host>),
                joined
                    .after(terra_walker::net::link::show)
                    .run_if(resource_exists::<Link>),
                alone
                    .run_if(not(resource_exists::<Link>))
                    .run_if(not(resource_exists::<Host>)),
            )
                .in_set(Sim::End),
        );
}

/// On the host: a guest's claim is what its game says, every claim known is told, and the
/// guests' are the others' this game knows.
fn host(
    mut host: ResMut<Host>,
    mut known: ResMut<Known>,
    mut others: ResMut<Others>,
    builds: Res<Builds>,
    mine: Option<Single<&Claim, IsLocalPlayer>>,
) {
    let asked = std::mem::take(&mut host.asked);
    for (peer, bytes) in asked {
        let Some((&CLAIMED, rest)) = bytes.split_first() else {
            // Another layer's.
            host.asked.push((peer, bytes));
            continue;
        };
        // An empty word says the guest has no claim.
        let told = Told::heard(rest);
        match known.guests.iter_mut().find(|(each, _)| *each == peer) {
            Some(guest) => guest.1 = told,
            None => known.guests.push((peer, told)),
        }
    }
    known.since += 1;
    if known.since < EVERY_TICKS {
        return;
    }
    known.since = 0;
    let theirs: Vec<Told> = known.guests.iter().filter_map(|(_, told)| *told).collect();
    let every: Vec<Told> = (mine.and_then(|mine| Told::of(&mine)).into_iter())
        .chain(theirs.iter().copied())
        .collect();
    for told in &every {
        if !known.put.iter().any(|(put, _)| put == told) {
            let place = Vec3::from(told.at.map(|cm| cm as f32 / 100.0));
            let id = host.put(CLAIM, place);
            host.tell_of(id, place, &told.said());
            known.put.push((*told, id));
        }
    }
    known.put.retain(|(put, id)| {
        let stands = every.contains(put);
        if !stands {
            host.gone(*id);
        }
        stands
    });
    let shown: Vec<Claim> = theirs.iter().map(|told| told.claim(&builds)).collect();
    if others.0.len() != shown.len()
        || others
            .0
            .iter()
            .zip(&shown)
            .any(|(a, b)| a.keeps() != b.keeps())
    {
        others.0 = shown;
    }
}

/// On a joined game: the claims the host tells of, but this player's own, are the others'
/// it knows; and its own, when it changes, is asked of the host.
fn joined(
    mut link: ResMut<Link>,
    mut known: ResMut<Known>,
    mut others: ResMut<Others>,
    builds: Res<Builds>,
    mine: Option<Single<&Claim, IsLocalPlayer>>,
) {
    known.since += 1;
    if known.since < EVERY_TICKS || link.tick().is_none() {
        return;
    }
    known.since = 0;
    let own = mine.and_then(|mine| Told::of(&mine));
    let shown: Vec<Claim> = (link.shown_of(CLAIM, 0).into_iter())
        .filter_map(|(_, _, newest, _)| Told::heard(newest))
        .filter(|told| Some(told.at) != own.map(|own| own.at))
        .map(|told| told.claim(&builds))
        .collect();
    if others.0.len() != shown.len()
        || others
            .0
            .iter()
            .zip(&shown)
            .any(|(a, b)| a.keeps() != b.keeps())
    {
        others.0 = shown;
    }
    if known.said != Some(own) {
        let mut ask = vec![CLAIMED];
        if let Some(own) = own {
            ask.extend(own.said());
        }
        link.ask(&ask);
        known.said = Some(own);
    }
}

/// With no session there are no others' claims, and what was told is forgotten.
fn alone(mut known: ResMut<Known>, mut others: ResMut<Others>) {
    if !others.0.is_empty() {
        others.0.clear();
    }
    if !(known.guests.is_empty() && known.put.is_empty() && known.said.is_none()) {
        *known = Known::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A claim crosses whole, and nonsense is none: cut short or run long it is not heard,
    /// and an empty word is no claim (a guest saying it has none).
    #[test]
    fn a_claim_crosses_whole_and_nonsense_is_none() {
        let told = Told {
            at: [-1250, 100_325, 4_075],
            step: 2,
        };
        let bytes = told.said();
        assert_eq!(Told::heard(&bytes), Some(told));
        assert_eq!(Told::heard(&bytes[..12]), None);
        assert_eq!(Told::heard(&[bytes.to_vec(), vec![0]].concat()), None);
        assert_eq!(Told::heard(&[]), None);
    }
}
