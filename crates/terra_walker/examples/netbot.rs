//! A player with no game behind it (D73): joins a hosted world over UDP as any client does
//! and walks by a script, for a soak against a running game (`tools/sandbox/net.ts --soak`,
//! docs/v0-tracks.md's Server bar: clients for thirty minutes at 100 ms and 1% loss).
//!
//!   netbot ADDRESS NAME MINUTES [LATENCY_MS LOSS]
//!
//! It walks, sprints, turns and stands by turns, then stands for its last ten seconds; three
//! seconds before the end it says, as one line of JSON, where the server last put it (at rest
//! by then), how long the longest silence was, and the bytes a second it sent and received;
//! then it leaves. A connection the server ends is said, and is a failure.

use bevy::math::Vec3;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use terra_motion::mover::net::Own;
use terra_net::client::{Client, Event};
use terra_net::clock::Clock;
use terra_net::transport::ClientTransport;
use terra_net::transport::conditioned::Conditioned;
use terra_net::transport::loopback::Conditions;
use terra_net::transport::udp::UdpClient;
use terra_net::wire::Wire;
use terra_walker::View;
use terra_walker::controls::Action;
use terra_walker::intent::Intent;
use terra_walker::net::remote::{VERSION, scripted_identity};
use terra_walker::net::{OWN, Steering};

const TICK: Duration = Duration::from_micros(15_625);

/// What it holds down so many seconds in: eight seconds of walking, a sprint, standing, a
/// step aside and back, round again.
fn held(seconds: f32) -> &'static [Action] {
    match seconds as u32 % 8 {
        0 | 1 => &[Action::Forward],
        2 => &[Action::Forward, Action::Sprint],
        4 => &[Action::Left],
        5 => &[Action::Back],
        _ => &[],
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "netbot ADDRESS NAME MINUTES [LATENCY_MS LOSS]";
    let address: SocketAddr = args
        .first()
        .and_then(|text| text.parse().ok())
        .expect(usage);
    let name = args.get(1).expect(usage);
    let minutes: f32 = args.get(2).and_then(|text| text.parse().ok()).expect(usage);
    let latency_ms: f64 = args
        .get(3)
        .and_then(|text| text.parse().ok())
        .unwrap_or(0.0);
    let loss: f64 = args
        .get(4)
        .and_then(|text| text.parse().ok())
        .unwrap_or(0.0);

    let identity = scripted_identity(name);
    let number = u64::from_le_bytes(identity.public()[..8].try_into().expect("eight bytes"));
    // The latency is one way: a round trip is twice it. A tenth of it again as jitter.
    let conditions = Conditions {
        latency_s: latency_ms / 1000.0,
        jitter_s: latency_ms / 10_000.0,
        loss,
        copies: loss,
        seed: number,
    };
    let socket = UdpClient::connect(address, number).expect("a socket");
    let mut wire = Conditioned::new(socket, conditions);
    let mut client = Client::new();
    client.hello(&mut wire, VERSION, &identity, name);

    let (mut clock, mut heard) = (Clock::default(), Vec::new());
    let total = Duration::from_secs_f32(minutes * 60.0);
    let (began, mut next) = (Instant::now(), Instant::now());
    let (mut at, mut answers, mut newest, mut silence) = (Vec3::ZERO, 0u64, 0u32, 0u32);
    let (mut up, mut down, mut samples, mut said) = (0.0, 0.0, 0u32, false);
    let mut last_second = 0;
    while began.elapsed() < total {
        wire.advance(TICK);
        client.poll(&mut wire, &mut heard);
        for event in heard.drain(..) {
            match event {
                Event::Welcome { tick, .. } => clock.start(tick),
                Event::Closed(reason) => {
                    println!(
                        "{}",
                        serde_json::json!({ "name": name, "closed": reason.message() })
                    );
                    std::process::exit(1);
                }
                _ => {}
            }
        }
        if let Some(stamp) = client.stamp() {
            clock.hear(stamp);
            if newest != 0 {
                silence = silence.max(stamp.tick.saturating_sub(newest));
            }
            newest = stamp.tick;
        }
        let mine = client.entities().find(|(_, entity)| entity.mine);
        if let Some(own) = mine
            .and_then(|(_, entity)| entity.part(OWN))
            .and_then(|samples| Own::decoded(&samples.newer.bytes).ok())
        {
            at = own.position.as_vec3();
            answers += 1;
        }
        let elapsed = began.elapsed();
        if let Some(tick) = clock.advance() {
            let mut intent = Intent::default();
            // Its last ten seconds it stands, so where it is told it is, is where it is.
            if elapsed + Duration::from_secs(10) < total {
                for &action in held(elapsed.as_secs_f32()) {
                    intent.keep_held(action);
                }
            }
            // It looks along the ground, turning slowly round.
            let upward = at.normalize_or(Vec3::Y);
            let east = upward.any_orthonormal_vector();
            let north = upward.cross(east);
            let turn = elapsed.as_secs_f32() * 0.1;
            let forward = east * libm::cosf(turn) + north * libm::sinf(turn);
            let view = View {
                forward,
                pitch: 0.0,
            };
            client.intent(tick, Steering { intent, view }.encoded());
        }
        client.send(&mut wire);
        if elapsed.as_secs() != last_second {
            last_second = elapsed.as_secs();
            let (sent, received) = wire.inner().rates();
            (up, down, samples) = (up + sent, down + received, samples + 1);
        }
        if !said && elapsed + Duration::from_secs(3) >= total {
            said = true;
            let rate = |sum: f64| (sum / f64::from(samples.max(1))).round();
            let report = serde_json::json!({
                "name": name,
                "at": at.to_array(),
                "answers": answers,
                "longest_silence_ticks": silence,
                "up_bytes_s": rate(up),
                "down_bytes_s": rate(down),
            });
            println!("{report}");
        }
        // Ticks at the clock's rate, a little fast or slow to hold its margin (D651).
        next += TICK.div_f64(clock.rate());
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
    }
    wire.close();
    wire.flush();
}
