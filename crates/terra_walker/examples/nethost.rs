//! The instance host with no window (D640, D51): realms of a seed's land hosted over UDP for
//! scripted runs and soaks (`tools/sandbox/net.ts --soak MINUTES --headless`), each walking
//! whoever joins it by the walker's layer alone (the ground, the mover, the walk).
//!
//!   nethost PORT SEED RADIUS_M SECONDS [REALMS]
//!
//! It says `listening` once its realms are made, counts SECONDS from the first connection,
//! then says, as one line of JSON, where each guest of the first realm stands, how long its
//! ticks took and how many began late, and what a realm costs with nobody in it (its memory,
//! and its tick while it still ticks); then it tells everyone the host left. A script
//! steers it by lines on its input (`tools/sandbox/crossing.ts`): `where` says each realm's
//! guests now, `hand` hands everyone in the first realm on to the second (a crossing), `end`
//! ends it with its report.

use bevy::prelude::{Transform, With};
use std::net::Ipv4Addr;
use std::time::{Duration, Instant};
use terra_core::{PlanetSpec, Seed};
use terra_net::server::Config;
use terra_net::transport::udp::UdpServer;
use terra_walker::net::instance::{Instances, trees, walker};
use terra_walker::net::session::VERSION;
use terra_walker::net::{Guest, Host};
use terra_world::planet::Planet;
use terra_world::start::Start;

const TICK: Duration = Duration::from_micros(15_625);
/// The ticks an empty realm's cost is measured over.
const MEASURED: u32 = 256;

/// This process's resident memory, MB (Linux; nothing elsewhere).
fn resident_mb() -> f64 {
    let pages = std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|text| text.split(' ').nth(1)?.parse::<f64>().ok());
    pages.map_or(0.0, |pages| pages * 4096.0 / 1.0e6)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let number = |at: usize| args.get(at).and_then(|text| text.parse::<f64>().ok());
    let (Some(port), Some(seed), Some(radius_m), Some(seconds)) =
        (number(0), number(1), number(2), number(3))
    else {
        eprintln!("usage: nethost PORT SEED RADIUS_M SECONDS [REALMS]");
        std::process::exit(2);
    };
    let realms = number(4).map_or(1, |realms| realms as u64);
    let wire = match UdpServer::bind((Ipv4Addr::UNSPECIFIED, port as u16).into(), 64) {
        Ok(wire) => wire,
        Err(why) => {
            eprintln!("Port {port} could not be opened: {why}. Is another host on it?");
            std::process::exit(1);
        }
    };
    let mut host = Instances::new(wire, Config::new(VERSION), vec![walker, trees]);
    let before_mb = resident_mb();
    for realm in 0..realms {
        let planet = Planet::new(PlanetSpec {
            seed: Seed(seed as u64 + realm),
            radius_m: radius_m as f32,
        });
        let start = Start::choose(&planet, false);
        host.open(planet, start.translation + start.right() * 2.0);
    }
    // What a realm costs with nobody in it: its tick, while it still ticks, and its memory.
    let began = Instant::now();
    for _ in 0..MEASURED {
        host.tick(TICK);
    }
    let empty_tick_us = began.elapsed().as_secs_f64() * 1.0e6 / f64::from(MEASURED) / realms as f64;
    let realm_mb = (resident_mb() - before_mb) / realms as f64;
    println!("listening");

    let (mut first, mut next) = (None::<Instant>, Instant::now());
    let (mut ticks, mut late, mut spent, mut worst) = (0u32, 0u32, Duration::ZERO, Duration::ZERO);
    let total = Duration::from_secs_f64(seconds);
    // Lines on its input, from a script that waits on a game.
    let (lines, asked) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lines().map_while(Result::ok) {
            if lines.send(line).is_err() {
                break;
            }
        }
    });
    let guests = |host: &mut Instances<UdpServer>, realm: usize| -> Vec<[f32; 3]> {
        let world = host.realm_mut(realm).world_mut();
        let mut guests = world.query_filtered::<&Transform, With<Guest>>();
        guests
            .iter(world)
            .map(|at| at.translation.to_array())
            .collect()
    };
    loop {
        let began = Instant::now();
        // A tick begun more than a tick after its time is late: the host is behind.
        if began > next + TICK {
            late += 1;
        }
        host.tick(TICK);
        let took = began.elapsed();
        if host.realm(0).world().resource::<Host>().server().clients() > 0 && first.is_none() {
            first = Some(began);
        }
        if first.is_some() {
            (ticks, spent, worst) = (ticks + 1, spent + took, worst.max(took));
        }
        let mut ended = first.is_some_and(|first| first.elapsed() >= total);
        for line in asked.try_iter() {
            match line.trim() {
                "where" => {
                    let each: Vec<_> = (0..host.realms())
                        .map(|realm| guests(&mut host, realm))
                        .collect();
                    println!("{}", serde_json::json!({ "where": each }));
                }
                "hand" if host.realms() > 1 => {
                    for peer in host.realm(0).world().resource::<Host>().peers() {
                        host.hand(peer, 1);
                    }
                }
                "end" => ended = true,
                other => eprintln!("nethost: \"{other}\" is not where, hand or end"),
            }
        }
        if ended {
            break;
        }
        next += TICK;
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
    }
    let at: Vec<[f32; 3]> = guests(&mut host, 0);
    let report = serde_json::json!({
        "guests": at,
        "ticks": ticks,
        "late_ticks": late,
        "tick_us_mean": (spent.as_secs_f64() * 1.0e6 / f64::from(ticks.max(1))).round(),
        "tick_us_worst": (worst.as_secs_f64() * 1.0e6).round(),
        "realms": realms,
        "empty_realm_tick_us": (empty_tick_us * 10.0).round() / 10.0,
        "realm_mb": (realm_mb * 10.0).round() / 10.0,
    });
    println!("{report}");
    host.close();
}
