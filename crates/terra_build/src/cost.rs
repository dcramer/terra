//! What building costs (D193, construction-001 gap 25): what each step of a cube and each
//! fitted prop takes, in exact fractions, so taking apart gives back exactly what went in,
//! no more and no less.
//!
//! A log fills its course in every cube it spans (D194), so a cube's share of it is a
//! fraction of a log; fractions add up exactly here, where floats would drift and could be
//! farmed.

use crate::Kind;
use std::collections::BTreeMap;

/// The finest share of an item an amount kept in a base may be: a power of two, as a
/// kit's costs are (a quarter of a stone, a thirty-second of wood) and a cube's steps
/// are sixteenths. Shares of powers of two add up without their denominator growing.
pub const FINEST_SHARE: u64 = 1 << 16;

/// The most of one item an amount kept in a base may be, in whole items.
pub const MOST_OF_ONE: u64 = 1 << 24;

/// An exact amount of a material: a fraction, kept in lowest terms.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(into = "(u64, u64)", try_from = "(u64, u64)")]
pub struct Amount {
    num: u64,
    den: u64,
}

impl Amount {
    pub const ZERO: Amount = Amount { num: 0, den: 1 };

    /// `num` parts in `den`; `den` may not be zero.
    pub fn new(num: u64, den: u64) -> Amount {
        assert!(den > 0, "an amount over nothing");
        let common = gcd(num, den);
        Amount {
            num: num / common,
            den: den / common,
        }
    }

    pub fn whole(count: u64) -> Amount {
        Amount::new(count, 1)
    }

    pub fn is_zero(self) -> bool {
        self.num == 0
    }

    /// The top and bottom of the fraction, in lowest terms.
    pub fn parts(self) -> (u64, u64) {
        (self.num, self.den)
    }

    /// Whether a base may keep it: a share no finer than [`FINEST_SHARE`] and no more
    /// than [`MOST_OF_ONE`], so what a base's costs come to cannot overflow. What a
    /// snapshot holds that is not is refused.
    pub fn in_bounds(self) -> bool {
        self.den.is_power_of_two() && self.den <= FINEST_SHARE && self.num <= MOST_OF_ONE * self.den
    }

    /// So many times this.
    pub fn times(self, count: u64) -> Amount {
        Amount::new(self.num * count, self.den)
    }

    /// A share of this: so much of it as `share` is of one.
    pub fn of(self, share: Amount) -> Amount {
        Amount::new(self.num * share.num, self.den * share.den)
    }

    /// What is left taking `other` away, or `None` if it is more than this.
    pub fn minus(self, other: Amount) -> Option<Amount> {
        let den = lcm(self.den, other.den);
        let (a, b) = (self.num * (den / self.den), other.num * (den / other.den));
        a.checked_sub(b).map(|num| Amount::new(num, den))
    }
}

impl std::ops::Add for Amount {
    type Output = Amount;
    fn add(self, other: Amount) -> Amount {
        let den = lcm(self.den, other.den);
        Amount::new(
            self.num * (den / self.den) + other.num * (den / other.den),
            den,
        )
    }
}

impl PartialOrd for Amount {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Amount {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (u128::from(self.num) * u128::from(other.den))
            .cmp(&(u128::from(other.num) * u128::from(self.den)))
    }
}

impl From<Amount> for (u64, u64) {
    fn from(amount: Amount) -> Self {
        (amount.num, amount.den)
    }
}

impl TryFrom<(u64, u64)> for Amount {
    type Error = &'static str;
    fn try_from((num, den): (u64, u64)) -> Result<Self, Self::Error> {
        if den == 0 {
            return Err("an amount over nothing");
        }
        Ok(Amount::new(num, den))
    }
}

impl std::fmt::Display for Amount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.den == 1 {
            write!(f, "{}", self.num)
        } else {
            write!(f, "{}/{}", self.num, self.den)
        }
    }
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.max(1)
}

fn lcm(a: u64, b: u64) -> u64 {
    a / gcd(a, b) * b
}

/// Materials and how much of each, by their item names in the catalogue.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Bill(pub BTreeMap<String, Amount>);

impl Bill {
    /// A bill of one material.
    pub fn of(material: &str, amount: Amount) -> Bill {
        let mut bill = Bill::default();
        bill.add_one(material, amount);
        bill
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The first of its amounts a base may not keep ([`Amount::in_bounds`]), if any.
    pub fn out_of_bounds(&self) -> Option<Amount> {
        self.0.values().copied().find(|amount| !amount.in_bounds())
    }

    fn add_one(&mut self, material: &str, amount: Amount) {
        if amount.is_zero() {
            return;
        }
        let entry = self.0.entry(material.to_string()).or_insert(Amount::ZERO);
        *entry = *entry + amount;
    }

    /// Adds another bill to this one.
    pub fn add(&mut self, other: &Bill) {
        for (material, &amount) in &other.0 {
            self.add_one(material, amount);
        }
    }

    /// This bill so many times.
    pub fn times(&self, count: u64) -> Bill {
        let mut bill = Bill::default();
        for (material, amount) in &self.0 {
            bill.add_one(material, amount.times(count));
        }
        bill
    }

    /// A share of this bill: so much of each as `share` is of one.
    pub fn share(&self, share: Amount) -> Bill {
        let mut bill = Bill::default();
        for (material, amount) in &self.0 {
            bill.add_one(material, amount.of(share));
        }
        bill
    }

    /// Takes another bill from this one, or `None`, changing nothing, if it holds more of
    /// something than this does.
    pub fn take(&mut self, other: &Bill) -> Option<()> {
        let mut left = self.0.clone();
        for (material, &amount) in &other.0 {
            let have = left.get(material).copied().unwrap_or(Amount::ZERO);
            let rest = have.minus(amount)?;
            if rest.is_zero() {
                left.remove(material);
            } else {
                left.insert(material.clone(), rest);
            }
        }
        self.0 = left;
        Some(())
    }

    /// Whether it holds at least as much of everything as another.
    pub fn covers(&self, other: &Bill) -> bool {
        self.clone().take(other).is_some()
    }
}

impl std::fmt::Display for Bill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lines: Vec<String> = self
            .0
            .iter()
            .map(|(material, amount)| format!("{amount} {material}"))
            .collect();
        write!(f, "{}", lines.join(", "))
    }
}

/// How a kind of construction goes in: in how many steps a cube fills (D194: a log
/// fills a cube in two courses, a block in one), and what each step of a cube takes.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Terms {
    /// How many steps fill a cube: 1, 2, 4, 8 or 16, so each is a whole number of
    /// sixteenths.
    pub steps: u8,
    pub per_step: Bill,
}

impl Default for Terms {
    /// In one step, for nothing: what the shortcuts and the yard build with.
    fn default() -> Self {
        Terms {
            steps: 1,
            per_step: Bill::default(),
        }
    }
}

impl Terms {
    /// So many steps, each taking a bill; the steps rounded to the nearest that divides
    /// a cube's sixteenths.
    pub fn new(steps: u8, per_step: Bill) -> Terms {
        let steps = [1u8, 2, 4, 8, 16]
            .into_iter()
            .min_by_key(|&s| s.abs_diff(steps))
            .unwrap_or(1);
        Terms { steps, per_step }
    }

    /// How many sixteenths of a cube one step fills.
    pub fn step(&self) -> u8 {
        crate::cubes::FULL / self.steps.max(1)
    }
}

/// A kind of fitted prop: a door, a window, shutters (construction-001, gap 15).
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct PropKind(pub u16);

/// How a kind stands up to harm (D193, D305): how much damage a cube of it takes before it is
/// destroyed, and the share of its cost a repair takes for a cube brought back from nothing,
/// a quarter of that for each quarter of its health it has lost. The share is a power of two,
/// so what it comes to stays exact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Toughness {
    pub health: u16,
    pub repair: Amount,
}

impl Default for Toughness {
    /// A kind no kit says anything of: a hundred, and half its cost to repair.
    fn default() -> Self {
        Toughness {
            health: 100,
            repair: Amount::new(1, 2),
        }
    }
}

impl Toughness {
    /// How many quarters of its health a cube with so much damage has lost, rounded up: 0
    /// for none, 4 for all of it. What a repair is charged by, and how the damage is drawn.
    pub fn quarters(&self, damage: u16) -> u8 {
        let health = u32::from(self.health.max(1));
        (u32::from(damage) * 4).div_ceil(health).min(4) as u8
    }
}

/// What everything costs: the terms of each kind of construction, and each kind of prop;
/// and how each kind stands up to harm. What is not listed is free, in one step, as in the
/// yard (docs/building-tech.md), and as tough as [`Toughness::default`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rules {
    pub kinds: BTreeMap<Kind, Terms>,
    pub props: BTreeMap<PropKind, Bill>,
    pub toughness: BTreeMap<Kind, Toughness>,
}

impl Rules {
    /// The terms a kind is built on.
    pub fn of(&self, kind: Kind) -> Terms {
        self.kinds.get(&kind).cloned().unwrap_or_default()
    }

    /// How a kind stands up to harm.
    pub fn toughness(&self, kind: Kind) -> Toughness {
        self.toughness.get(&kind).copied().unwrap_or_default()
    }

    /// What a prop takes to fit.
    pub fn prop(&self, kind: PropKind) -> Bill {
        self.props.get(&kind).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractions_add_up_exactly() {
        let eighth = Amount::new(1, 8);
        let total = (0..8).fold(Amount::ZERO, |sum, _| sum + eighth);
        assert_eq!(total, Amount::whole(1));
        assert_eq!(Amount::new(2, 6), Amount::new(1, 3));
        assert_eq!(Amount::new(1, 3) + Amount::new(1, 6), Amount::new(1, 2));
        assert!(Amount::new(1, 3) < Amount::new(1, 2));
        assert_eq!(
            Amount::new(1, 2).minus(Amount::new(1, 3)),
            Some(Amount::new(1, 6))
        );
        assert_eq!(Amount::new(1, 3).minus(Amount::new(1, 2)), None);
    }

    #[test]
    fn amounts_stay_exact_where_the_working_is_past_64_bits() {
        let big = u64::MAX;
        // Compared across: each product is past 64 bits.
        assert!(Amount::new(big - 1, big) < Amount::new(big, big - 1));
        assert!(Amount::new(big, big - 1) > Amount::whole(1));
        // Shares of large denominators, whose product is past 64 bits: the common
        // denominator is the larger, not the product.
        let tiny = Amount::new(1, 1 << 40);
        assert_eq!(tiny + Amount::new(1, 1 << 41), Amount::new(3, 1 << 41));
        assert_eq!(
            Amount::new(1, 1 << 33).minus(Amount::new(1, 1 << 34)),
            Some(Amount::new(1, 1 << 34))
        );
        // A large number of a small share.
        assert_eq!(tiny.times(1 << 50), Amount::whole(1 << 10));
        assert_eq!(
            Amount::whole(big - 1) + Amount::whole(1),
            Amount::whole(big)
        );
    }

    #[test]
    fn a_bill_is_taken_whole_or_not_at_all() {
        let mut stock = Bill::of("Log", Amount::whole(1));
        stock.add(&Bill::of("Stone", Amount::new(1, 2)));
        let mut want = Bill::of("Log", Amount::new(1, 8));
        want.add(&Bill::of("Stone", Amount::whole(1)));
        let before = stock.clone();
        assert_eq!(stock.take(&want), None);
        assert_eq!(stock, before);
        assert!(stock.take(&Bill::of("Log", Amount::new(1, 8))).is_some());
        assert_eq!(stock.0["Log"], Amount::new(7, 8));
        assert_eq!(format!("{}", Bill::of("Log", Amount::new(1, 8))), "1/8 Log");
    }

    #[test]
    fn steps_are_whole_sixteenths() {
        assert_eq!(Terms::new(2, Bill::default()).step(), 8);
        assert_eq!(Terms::new(3, Bill::default()).steps, 2);
        assert_eq!(Terms::default().step(), 16);
    }
}
