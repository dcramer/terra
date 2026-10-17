//! What a town's market sells (D352): each stall one trade (a greengrocer, a baker, a potter
//! and so on) drawn by the town's seed from the trades in `assets/data/market.ron`, so no two
//! stalls of a market sell the same; on its counter two to four of its trade's goods laid
//! side by side with room between, and on the ground beside it one or two of its trade's
//! bigger goods, where the layout kept room for them clear of every way
//! ([`ThingKind::Goods`], laid with the town's furnishing). Each good is a catalogue item,
//! a model of the goods prop pipeline (`tools/props/goods.ts`), placed by its size alone.

use super::{Thing, ThingKind, Town};
use crate::layout::{hash, unit};
use glam::Vec2;
use serde::Deserialize;

/// The file a market's trades are read from, under `assets`.
pub const FILE: &str = "data/market.ron";

/// What a market can sell, and how a stall's counter is laid.
#[derive(Clone, Debug, Deserialize)]
pub struct Market {
    pub counter: Counter,
    /// The catalogue item a stall hangs at its awning's front, and the share that do.
    pub lantern: (String, f32),
    pub trades: Vec<Trade>,
    /// The stall's own looks, catalogue items' files alike but for their awnings' dye, one
    /// drawn for each stall by the seed so neighbours differ; the first is the stall.
    #[serde(default)]
    pub stalls: Vec<String>,
}

/// How goods are laid along a stall's counter: across how much of it, how deep it is from
/// the front of its top to the back, how far apart goods keep at least, and how many it
/// shows.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Counter {
    pub wide: f32,
    pub deep: f32,
    pub gap: f32,
    pub goods: (usize, usize),
}

/// A trade: what it is called, the goods it lays on its counter and the bigger ones it
/// stands on the ground, each a catalogue item's file.
#[derive(Clone, Debug, Deserialize)]
pub struct Trade {
    pub name: String,
    pub counter: Vec<String>,
    #[serde(default)]
    pub ground: Vec<String>,
}

/// One good laid out: its catalogue item's file, the stall it belongs to (an index of the
/// town's things), where its middle stands in the town and which way its front looks, and
/// whether it stands on the stall's counter rather than the ground.
#[derive(Clone, Debug, PartialEq)]
pub struct Good {
    pub item: String,
    pub stall: usize,
    pub at: Vec2,
    pub facing: f32,
    pub on_counter: bool,
}

/// How far a good on a counter may be turned either way from square, in radians.
const TURN: f32 = 0.12;

impl Market {
    /// The market from its file's text, checked: every trade has goods for its counter.
    pub fn parse(text: &str) -> Result<Market, String> {
        let market: Market = ron::from_str(text).map_err(|why| format!("{FILE}: {why}"))?;
        if market.trades.is_empty() {
            return Err(format!("{FILE}: no trades"));
        }
        if let Some(trade) = market.trades.iter().find(|t| t.counter.is_empty()) {
            return Err(format!(
                "{FILE}: {} has nothing for its counter",
                trade.name
            ));
        }
        let (least, most) = market.counter.goods;
        if least == 0 || least > most {
            return Err(format!(
                "{FILE}: a counter's goods ({least}, {most}) are not a range"
            ));
        }
        Ok(market)
    }

    /// Which of the stall's looks each of the town's stalls is, as (the stall's index among
    /// its things, the look's file): drawn by the seed, never the same as the stall before it
    /// in its row where there are two looks or more.
    pub fn looks(&self, town: &Town, seed: u32) -> Vec<(usize, String)> {
        let mut last = usize::MAX;
        stalls(town)
            .enumerate()
            .filter_map(|(n, (index, _))| {
                let count = self.stalls.len();
                if count == 0 {
                    return None;
                }
                let mut pick = hash(&[seed, 63, n as u32]) as usize % count;
                if pick == last && count > 1 {
                    pick = (pick + 1) % count;
                }
                last = pick;
                Some((index, self.stalls[pick].clone()))
            })
            .collect()
    }

    /// The trade each of the town's stalls takes, as (the stall's index among its things,
    /// the trade's index): the trades in an order drawn by the seed, dealt to the stalls in
    /// turn, so a market of no more stalls than trades never sells one thing twice.
    pub fn trades(&self, town: &Town, seed: u32) -> Vec<(usize, usize)> {
        let mut order: Vec<usize> = (0..self.trades.len()).collect();
        shuffle(&mut order, seed, 1);
        stalls(town)
            .enumerate()
            .map(|(n, (index, _))| (index, order[n % order.len()]))
            .collect()
    }

    /// The stalls that hang a lantern, by the seed, as indices of the town's things.
    pub fn lanterns(&self, town: &Town, seed: u32) -> Vec<usize> {
        stalls(town)
            .filter(|(index, _)| unit(hash(&[seed, 64, *index as u32])) < self.lantern.1)
            .map(|(index, _)| index)
            .collect()
    }

    /// Every good the town's market shows. `counter` is the middle of a stall's counter top,
    /// in its own metres (across its front, and out of it), from its model's `counter`
    /// socket; `size` a good's width and depth, from its catalogue entry, for each item
    /// the trades name (one it does not know is left out).
    pub fn goods(
        &self,
        town: &Town,
        seed: u32,
        counter: Vec2,
        size: &dyn Fn(&str) -> Option<Vec2>,
    ) -> Vec<Good> {
        let trades = self.trades(town, seed);
        let mut laid = Vec::new();
        for &(index, trade) in &trades {
            let stall = &town.things[index];
            let salt = index as u32;
            laid.extend(
                self.counter_goods(&self.trades[trade], seed, salt, size)
                    .into_iter()
                    .map(|(item, local, turn)| Good {
                        item,
                        stall: index,
                        at: stall.place(counter + local),
                        facing: stall.facing + turn,
                        on_counter: true,
                    }),
            );
        }
        // On the ground: in the places the layout kept beside each stall, the stall's trade's
        // bigger goods, a different one in each where it has more than one.
        let (box_low, box_high) = ThingKind::Goods.ground().unwrap_or_default();
        let room = box_high - box_low;
        let mut beside: Vec<usize> = vec![0; town.things.len()];
        for thing in &town.things {
            if thing.kind != ThingKind::Goods {
                continue;
            }
            let Some(&(stall, trade)) = trades.iter().min_by(|a, b| {
                let da = town.things[a.0].at.distance(thing.at);
                let db = town.things[b.0].at.distance(thing.at);
                da.total_cmp(&db)
            }) else {
                continue;
            };
            // Each that fits its room, square or turned a quarter, as a crate stands
            // lengthwise beside the seller.
            let mut choices: Vec<(&String, f32)> = self.trades[trade]
                .ground
                .iter()
                .filter_map(|item| fits(size(item)?, room).map(|turn| (item, turn)))
                .collect();
            if choices.is_empty() {
                continue;
            }
            let first = hash(&[seed, 61, stall as u32]) as usize;
            let turn = (first + beside[stall]) % choices.len();
            choices.rotate_left(turn);
            beside[stall] += 1;
            let (item, turn) = choices[0];
            laid.push(Good {
                item: item.clone(),
                stall,
                at: thing.at,
                facing: thing.facing + turn,
                on_counter: false,
            });
        }
        laid
    }

    /// A trade's goods on one counter: how many the seed says, as many of them as fit across
    /// it with room between, spread evenly along it with a little play, each a little
    /// forward or back and turned a little. Each is (its item, its middle from the counter's
    /// middle, its turn).
    fn counter_goods(
        &self,
        trade: &Trade,
        seed: u32,
        salt: u32,
        size: &dyn Fn(&str) -> Option<Vec2>,
    ) -> Vec<(String, Vec2, f32)> {
        let counter = self.counter;
        let roll = |n: u32| unit(hash(&[seed, 62, salt, n]));
        let mut order: Vec<usize> = (0..trade.counter.len()).collect();
        shuffle(&mut order, seed, 100 + salt);
        let (least, most) = counter.goods;
        let wanted = least + (roll(0) * (most - least + 1) as f32) as usize;
        let mut chosen: Vec<(String, Vec2)> = Vec::new();
        let mut across = 0.0;
        // Round its goods twice, so a counter is laid full, as a market's are, a good shown
        // again where the trade has fewer than fit.
        for &k in order.iter().chain(&order) {
            if chosen.len() >= wanted.min(most) {
                break;
            }
            let item = &trade.counter[k];
            let Some(s) = size(item) else { continue };
            if s.y > counter.deep {
                continue;
            }
            // What it takes along the counter, turned its little: a corner of it swings out.
            let takes = s.x + 2.0 * s.y * TURN;
            let width = across + takes + counter.gap * chosen.len() as f32;
            if width <= counter.wide {
                across += takes;
                chosen.push((item.clone(), s));
            }
        }
        // The room left shared out between them and at the ends, each moved within its share.
        let spare = (counter.wide - across).max(0.0) / (chosen.len() + 1) as f32;
        let mut x = -counter.wide / 2.0;
        chosen
            .into_iter()
            .enumerate()
            .map(|(n, (item, s))| {
                let n = n as u32 + 1;
                x += spare;
                let takes = s.x + 2.0 * s.y * TURN;
                let play = (roll(n * 3) - 0.5) * (spare - counter.gap).max(0.0) * 0.8;
                let middle = x + takes / 2.0 + play;
                x += takes;
                let depth = (roll(n * 3 + 1) - 0.5) * (counter.deep - s.y).max(0.0) * 0.7;
                let turn = (roll(n * 3 + 2) - 0.5) * 2.0 * TURN;
                (item, Vec2::new(middle, depth), turn)
            })
            .collect()
    }
}

/// The turn a good of a size takes to fit a room (both across, then deep), square or a
/// quarter turned, if it fits either way.
pub fn fits(size: Vec2, room: Vec2) -> Option<f32> {
    if size.x <= room.x && size.y <= room.y {
        Some(0.0)
    } else if size.y <= room.x && size.x <= room.y {
        Some(core::f32::consts::FRAC_PI_2)
    } else {
        None
    }
}

/// The town's stalls, in the order the layout laid them along their rows.
fn stalls(town: &Town) -> impl Iterator<Item = (usize, &Thing)> {
    town.things
        .iter()
        .enumerate()
        .filter(|(_, thing)| thing.kind == ThingKind::Stall)
}

/// A seeded shuffle (Fisher and Yates').
fn shuffle(items: &mut [usize], seed: u32, salt: u32) {
    for i in (1..items.len()).rev() {
        let j = hash(&[seed, 63, salt, i as u32]) as usize % (i + 1);
        items.swap(i, j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Kind;
    use crate::village::Kinds;
    use std::collections::HashMap;

    const KINDS: Kinds = Kinds {
        timber: Kind(1),
        masonry: Kind(2),
        beams: Kind(3),
        cob: Kind(4),
        framed: Kind(5),
        timber_roof: Kind(0x800),
        masonry_roof: Kind(0x801),
        beams_roof: Kind(0x802),
    };

    fn flat_town(seed: u32) -> Town {
        super::super::town(seed, KINDS, &|_: Vec2| 0.0)
    }

    fn market() -> Market {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/data/market.ron"
        ))
        .expect("the market's file");
        Market::parse(&text).expect("the market reads")
    }

    /// Each good's width and depth, read from the catalogue's entries as the game reads
    /// them: a different way to the code's of knowing what fits.
    fn sizes() -> HashMap<String, Vec2> {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/data/builds.ron"
        ))
        .expect("the catalogue");
        let mut sizes = HashMap::new();
        for line in text.lines() {
            let (Some(file), Some(dims)) =
                (line.split("file: \"").nth(1), line.split("dims: (").nth(1))
            else {
                continue;
            };
            let file = file.split('"').next().unwrap_or_default().to_string();
            let numbers: Vec<f32> = dims
                .split(')')
                .next()
                .unwrap_or_default()
                .split(',')
                .filter_map(|n| n.trim().parse().ok())
                .collect();
            if numbers.len() == 3 {
                sizes.insert(file, Vec2::new(numbers[0], numbers[2]));
            }
        }
        sizes
    }

    #[test]
    fn every_good_a_trade_names_is_in_the_catalogue_and_fits_where_it_goes() {
        let market = market();
        let sizes = sizes();
        let (low, high) = ThingKind::Goods.ground().expect("goods take ground");
        for trade in &market.trades {
            for item in &trade.counter {
                let size = sizes
                    .get(item)
                    .unwrap_or_else(|| panic!("{item} is not in the catalogue"));
                assert!(
                    size.x <= market.counter.wide && size.y <= market.counter.deep,
                    "{item} is bigger than a counter"
                );
            }
            for item in &trade.ground {
                let size = sizes
                    .get(item)
                    .unwrap_or_else(|| panic!("{item} is not in the catalogue"));
                assert!(
                    fits(*size, high - low).is_some(),
                    "{item} is bigger than its room by a stall"
                );
            }
        }
    }

    #[test] // Slow: run before a push, not a commit (SLOW_TESTS in xtask).
    fn neighbouring_stalls_sell_different_things_and_seeds_differ() {
        let market = market();
        let mut deals = Vec::new();
        for seed in 1..=8 {
            let town = flat_town(seed);
            let trades = market.trades(&town, seed);
            assert!(!trades.is_empty(), "town {seed} has stalls");
            for (n, &(a, ta)) in trades.iter().enumerate() {
                for &(b, tb) in &trades[n + 1..] {
                    if town.things[a].at.distance(town.things[b].at) < 7.0 {
                        assert_ne!(
                            ta, tb,
                            "town {seed}: two stalls side by side both sell {}",
                            market.trades[ta].name
                        );
                    }
                }
            }
            deals.push(trades.iter().map(|t| t.1).collect::<Vec<_>>());
        }
        deals.dedup();
        assert!(
            deals.len() > 4,
            "the seeds deal the trades alike: {deals:?}"
        );
    }

    #[test] // Slow: run before a push, not a commit (SLOW_TESTS in xtask).
    fn a_counters_goods_are_its_trades_and_lie_on_it_apart() {
        let market = market();
        let sizes = sizes();
        let size = |item: &str| sizes.get(item).copied();
        let counter = Vec2::new(0.0, 0.33);
        for seed in 1..=8 {
            let town = flat_town(seed);
            let trades: HashMap<usize, usize> = market.trades(&town, seed).into_iter().collect();
            let goods = market.goods(&town, seed, counter, &size);
            for (&stall, &trade) in &trades {
                let on: Vec<&Good> = goods
                    .iter()
                    .filter(|g| g.stall == stall && g.on_counter)
                    .collect();
                let (least, most) = market.counter.goods;
                assert!(
                    (least.min(market.trades[trade].counter.len())..=most).contains(&on.len()),
                    "town {seed}: {} goods on a counter",
                    on.len()
                );
                let thing = &town.things[stall];
                // In the stall's own metres, from the counter's middle: across, and out.
                let local = |p: Vec2| {
                    let d = p - thing.at;
                    let front = thing.front();
                    Vec2::new(d.dot(-front.perp()), d.dot(front)) - counter
                };
                let mut spans: Vec<(f32, f32)> = Vec::new();
                for good in &on {
                    assert!(
                        market.trades[trade].counter.contains(&good.item),
                        "town {seed}: {} on a {}",
                        good.item,
                        market.trades[trade].name
                    );
                    let s = sizes[&good.item];
                    let p = local(good.at);
                    // Turned a little, so a little wider than it stands square.
                    let half = s.x / 2.0 + s.y * TURN;
                    assert!(
                        p.x.abs() + s.x / 2.0 <= market.counter.wide / 2.0 + 0.02,
                        "town {seed}: {} off the counter's end at {p}",
                        good.item
                    );
                    assert!(
                        p.y.abs() + s.y / 2.0 <= market.counter.deep / 2.0 + 0.02,
                        "town {seed}: {} off the counter's front or back at {p}",
                        good.item
                    );
                    spans.push((p.x - half, p.x + half));
                }
                spans.sort_by(|a, b| a.0.total_cmp(&b.0));
                for pair in spans.windows(2) {
                    assert!(
                        pair[1].0 > pair[0].1 - 0.02,
                        "town {seed}: two goods on one counter overlap: {spans:?}"
                    );
                }
            }
        }
    }

    #[test] // Slow: run before a push, not a commit (SLOW_TESTS in xtask).
    fn the_ground_beside_a_stall_shows_its_own_trades_goods() {
        let market = market();
        let sizes = sizes();
        let size = |item: &str| sizes.get(item).copied();
        let mut shown = 0;
        for seed in 1..=8 {
            let town = flat_town(seed);
            let trades: HashMap<usize, usize> = market.trades(&town, seed).into_iter().collect();
            for good in market
                .goods(&town, seed, Vec2::ZERO, &size)
                .iter()
                .filter(|g| !g.on_counter)
            {
                let trade = &market.trades[trades[&good.stall]];
                assert!(
                    trade.ground.contains(&good.item),
                    "town {seed}: {} beside a {}",
                    good.item,
                    trade.name
                );
                assert!(
                    good.at.distance(town.things[good.stall].at) < 2.5,
                    "town {seed}: {} far from its stall",
                    good.item
                );
                shown += 1;
            }
        }
        assert!(
            shown >= 8,
            "only {shown} goods on the ground beside the stalls of eight towns"
        );
    }

    #[test] // Slow: run before a push, not a commit (SLOW_TESTS in xtask).
    fn neighbouring_stalls_wear_different_awnings() {
        let mut market = market();
        market.stalls = vec!["A".into(), "B".into(), "C".into()];
        for seed in 1..6 {
            let town = flat_town(seed);
            let looks = market.looks(&town, seed);
            assert_eq!(looks.len(), stalls(&town).count());
            for pair in looks.windows(2) {
                assert_ne!(pair[0].1, pair[1].1, "town {seed}: {looks:?}");
            }
            let kinds: std::collections::BTreeSet<_> = looks.iter().map(|(_, l)| l).collect();
            assert!(kinds.len() >= 2, "town {seed}: {looks:?}");
        }
    }

    #[test] // Slow: run before a push, not a commit (SLOW_TESTS in xtask).
    fn some_stalls_hang_a_lantern_and_others_not() {
        let market = market();
        let (mut hung, mut all) = (0, 0);
        for seed in 1..=8 {
            let town = flat_town(seed);
            let lanterns = market.lanterns(&town, seed);
            assert!(
                lanterns
                    .iter()
                    .all(|&i| town.things[i].kind == ThingKind::Stall)
            );
            hung += lanterns.len();
            all += town
                .things
                .iter()
                .filter(|t| t.kind == ThingKind::Stall)
                .count();
        }
        let share = hung as f32 / all as f32;
        assert!(
            (share - market.lantern.1).abs() < 0.25,
            "{hung} of {all} stalls hang a lantern"
        );
    }
}
