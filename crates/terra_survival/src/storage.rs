//! Storage: chests and whatever else is built to hold things (D84, docs/items.md), each a
//! grid of cells as the backpack is, on the item standing in the world (D83). F at one
//! opens it beside the backpack (`storage_screen`).
//!
//! Building draws on it (D141): what is built in a claim, a plan's parts and the items set
//! down, takes what it needs from the builder's pack first and then from the storage
//! standing in that claim, nearest first, as Enshrouded's base storage serves its builders.
//! [`Stores`] counts and takes from it for `terra_building::act` and the hammer.
//!
//! Storage is taken apart or moved only when it is empty, as nothing is taken away while
//! anything rests on it (D193); a player is told to empty it first.
//!
//! Over the remote protocol (D69), places in metres in the base's own space:
//!
//! - `storage.place`: the hammer's (`tool::place`), setting down an item from the build menu.
//! - `storage.put {item, count?}`: puts so many of a kind from the player's pack, all of
//!   them if no count, into the storage nearest them within reach.
//! - `storage.state`: the storage in the player's claim: what each is, where, and what it
//!   holds, by kind.

use crate::claim::Claim;
use crate::interact::{Focus, Interactable, REACH_M};
use crate::inventory::{Cost, Inventory, InventoryScreen};
use crate::props::Prop;
use bevy::{
    ecs::system::{SystemParam, SystemState},
    prelude::*,
    remote::{BrpResult, builtin_methods::parse_some},
    window::{CursorGrabMode, CursorOptions},
};
use serde::Deserialize;
use serde_json::{Value, json};
use terra_items::{Grid, ItemKind};
use terra_walker::IsLocalPlayer;
use terra_walker::camera::grab_mouse;
use terra_walker::controls::{Action, Settings};
use terra_walker::options::OptionsMenu;
use terra_walker::{LocalPlayer, Player};
use terra_world::remote::{method, refused};

/// What a piece of storage holds: a grid of cells, as the backpack is.
#[derive(Component, Clone, Debug)]
pub struct Storage(pub Grid);

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (open, leave).chain());
    method(app, "storage.put", put);
    method(app, "storage.state", state);
}

/// The storage a claim covers and what is in it, for building to draw on.
#[derive(SystemParam)]
pub struct Stores<'w, 's> {
    stores: Query<'w, 's, (Entity, &'static mut Storage, &'static Transform)>,
}

impl Stores<'_, '_> {
    /// The storage standing in a claim, nearest a point first: none without a claim.
    fn in_claim(&self, claim: Option<&Claim>, near: Vec3) -> Vec<Entity> {
        let Some(claim) = claim else {
            return Vec::new();
        };
        let mut found: Vec<(f32, Entity)> = self
            .stores
            .iter()
            .filter(|(_, _, at)| claim.covers(at.translation))
            .map(|(entity, _, at)| (at.translation.distance_squared(near), entity))
            .collect();
        found.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        found.into_iter().map(|(_, entity)| entity).collect()
    }

    /// How many of a kind the storage in a claim holds.
    pub fn count(&self, claim: Option<&Claim>, kind: ItemKind) -> u32 {
        self.in_claim(claim, Vec3::ZERO)
            .into_iter()
            .filter_map(|entity| self.stores.get(entity).ok())
            .map(|(_, storage, _)| storage.0.count(kind))
            .sum()
    }

    /// Takes up to so many of a kind from the storage in a claim, nearest a point first.
    /// Returns how many it took.
    pub fn take(&mut self, claim: Option<&Claim>, near: Vec3, kind: ItemKind, count: u32) -> u32 {
        let mut taken = 0;
        for entity in self.in_claim(claim, near) {
            if taken == count {
                break;
            }
            // Only storage that holds some is touched, so the rest is not marked changed.
            if let Ok((_, mut storage, _)) = self.stores.get_mut(entity)
                && storage.0.count(kind) > 0
            {
                taken += take_from([&mut storage.0], kind, count - taken);
            }
        }
        taken
    }

    /// The first kind of item a cost so many times over is short of, and by how many,
    /// counting what is carried and what is stored in the claim.
    pub fn short(
        &self,
        claim: Option<&Claim>,
        carried: &Inventory,
        cost: &Cost,
        times: u32,
    ) -> Option<(ItemKind, u32)> {
        wanted(cost, times).into_iter().find_map(|(kind, wanted)| {
            let have = carried.count(kind) + self.count(claim, kind);
            wanted
                .checked_sub(have)
                .filter(|&short| short > 0)
                .map(|short| (kind, short))
        })
    }

    /// Pays a cost so many times over, if it can be paid in full: from what is carried
    /// first, then the storage in the claim, nearest first. Returns whether it was.
    pub fn pay(
        &mut self,
        claim: Option<&Claim>,
        near: Vec3,
        carried: &mut Inventory,
        cost: &Cost,
        times: u32,
    ) -> bool {
        if self.short(claim, carried, cost, times).is_some() {
            return false;
        }
        for (kind, wanted) in wanted(cost, times) {
            let from_pack = carried.count(kind).min(wanted);
            carried.spend(kind, from_pack);
            self.take(claim, near, kind, wanted - from_pack);
        }
        true
    }

    /// Whether something is storage with anything in it, which is not taken apart or
    /// moved until it is emptied.
    pub fn holding(&self, entity: Entity) -> bool {
        self.stores
            .get(entity)
            .is_ok_and(|(_, storage, _)| !storage.0.is_empty())
    }
}

/// What a cost so many times over comes to, by kind.
fn wanted(cost: &Cost, times: u32) -> Vec<(ItemKind, u32)> {
    terra_items::by_kind(
        &cost
            .iter()
            .map(|&(kind, each)| (kind, each * times))
            .collect::<Vec<_>>(),
    )
}

/// Takes up to so many of a kind from grids in turn, each as far as it holds them. Returns
/// how many it took.
pub fn take_from<'a>(
    grids: impl IntoIterator<Item = &'a mut Grid>,
    kind: ItemKind,
    count: u32,
) -> u32 {
    let mut left = count;
    for grid in grids {
        if left == 0 {
            break;
        }
        let here = grid.count(kind).min(left);
        if here > 0 && grid.take(&[(kind, here)]).is_ok() {
            left -= here;
        }
    }
    count - left
}

/// The interact key at storage opens it beside the backpack; again closes it.
fn open(
    settings: Res<Settings>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    menu: Res<OptionsMenu>,
    focus: Single<&Focus, IsLocalPlayer>,
    window: Single<(&Window, &mut CursorOptions)>,
    mut screen: ResMut<InventoryScreen>,
) {
    let (window, mut cursor) = window.into_inner();
    // Closed some other way, such as with Tab, it is closed.
    if !screen.open {
        screen.storage = None;
    }
    let captured = cursor.grab_mode != CursorGrabMode::None;
    if menu.open || !settings.went_down(Action::Interact, &keys, &mouse, captured) {
        return;
    }
    if screen.storage.is_some() {
        screen.open = false;
        screen.storage = None;
        grab_mouse(&mut cursor, window.focused);
    } else if let (Some(Interactable::Storage), Some(entity)) = (focus.what, focus.entity)
        && !screen.open
    {
        screen.open = true;
        screen.storage = Some(entity);
        grab_mouse(&mut cursor, false);
    }
}

/// Walking away from storage, or its going, closes it.
fn leave(
    player: Single<&Transform, IsLocalPlayer>,
    stores: Query<(&Prop, &Transform), With<Storage>>,
    window: Single<(&Window, &mut CursorOptions)>,
    mut screen: ResMut<InventoryScreen>,
) {
    let Some(entity) = screen.storage else { return };
    let near = stores.get(entity).is_ok_and(|(prop, at)| {
        let up = at.translation.normalize();
        let across = prop.half.x.max(prop.half.z);
        (player.translation - at.translation)
            .reject_from(up)
            .length()
            < REACH_M + across
    });
    if !near {
        screen.open = false;
        screen.storage = None;
        let (window, mut cursor) = window.into_inner();
        grab_mouse(&mut cursor, window.focused);
    }
}

#[derive(Deserialize)]
struct Put {
    item: String,
    count: Option<u32>,
}

/// Puts things from the player's pack into the storage nearest them within reach.
fn put(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Put { item, count } = parse_some(params)?;
    let mut state = SystemState::<(
        crate::inventory::Belongings,
        Query<(&mut Storage, &Transform), Without<Player>>,
        Query<&Transform, (With<Player>, With<LocalPlayer>)>,
    )>::new(world);
    let answer = {
        let (mut belongings, mut stores, players) = state
            .get_mut(world)
            .map_err(|problem| refused(format!("cannot store now: {problem}")))?;
        let feet = players
            .single()
            .map_err(|_| refused("there is no player yet"))?
            .translation;
        let mut carried = belongings
            .mine()
            .ok_or_else(|| refused("there is no player yet"))?;
        let wanted = item.replace('_', " ");
        let kind = carried
            .items()
            .kinds()
            .find(|&kind| carried.items().name(kind).eq_ignore_ascii_case(&wanted))
            .ok_or_else(|| refused(format!("there is no kind of item called {item}")))?;
        let (mut storage, _) = stores
            .iter_mut()
            .filter(|(_, at)| at.translation.distance(feet) < REACH_M + 1.0)
            .min_by(|a, b| {
                a.1.translation
                    .distance(feet)
                    .total_cmp(&b.1.translation.distance(feet))
            })
            .ok_or_else(|| refused("there is no storage within reach"))?;
        let ids: Vec<terra_items::ItemId> = carried
            .backpack()
            .iter()
            .filter(|placed| placed.item.kind == kind)
            .map(|placed| placed.item.id)
            .take(count.unwrap_or(u32::MAX) as usize)
            .collect();
        let put = ids
            .into_iter()
            .filter(|&id| carried.put_into(&mut storage.0, id).is_ok())
            .count();
        info!("Stored {put} {} by script.", carried.items().name(kind));
        json!({ "put": put })
    };
    state.apply(world);
    Ok(answer)
}

/// The storage in the player's claim, what each is, where in the base, and what it holds.
fn state(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    let claim = world
        .query_filtered::<&Claim, (With<Player>, With<LocalPlayer>)>()
        .single(world)
        .map_err(|_| refused("there is no player yet"))?
        .clone();
    let frame = world.resource::<terra_bases::Site>().frame;
    let to_base = frame.compute_affine().inverse();
    let mut stores = world.query::<(&Storage, &Prop, &Transform)>();
    let items = world.resource::<crate::inventory::Items>();
    let mut found: Vec<Value> = stores
        .iter(world)
        .filter(|(_, _, at)| claim.covers(at.translation))
        .map(|(storage, prop, at)| {
            let holds: serde_json::Map<String, Value> = items
                .kinds()
                .filter_map(|kind| {
                    let count = storage.0.count(kind);
                    (count > 0).then(|| (items.name(kind).to_string(), json!(count)))
                })
                .collect();
            let local = to_base.transform_point3(at.translation);
            // To the centimetre, so a place read again after loading reads the same.
            let cm = |value: f32| (f64::from(value) * 100.0).round() / 100.0;
            json!({
                "item": prop.item.name,
                "at": [cm(local.x), cm(local.y), cm(local.z)],
                "holds": holds,
            })
        })
        .collect();
    found.sort_by_key(|store| store["at"].to_string());
    Ok(json!({ "stores": found }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_items::{Catalogue, IdSource, KindDef};

    fn filled(catalogue: &Catalogue, ids: &mut IdSource, kind: ItemKind, count: u32) -> Grid {
        let mut grid = Grid::new(4, 4);
        for _ in 0..count {
            grid.insert(catalogue, ids.make(kind), None).unwrap();
        }
        grid
    }

    #[test]
    fn taking_draws_on_each_store_in_turn_and_no_more_than_asked() {
        let mut catalogue = Catalogue::default();
        let wood = catalogue.add(KindDef::new("Wood"));
        let stone = catalogue.add(KindDef::new("Stone"));
        let mut ids = IdSource::default();
        let mut near = filled(&catalogue, &mut ids, wood, 3);
        let mut far = filled(&catalogue, &mut ids, wood, 5);
        far.insert(&catalogue, ids.make(stone), None).unwrap();

        // Four: the near store's three, then one of the far's.
        assert_eq!(take_from([&mut near, &mut far], wood, 4), 4);
        assert_eq!((near.count(wood), far.count(wood)), (0, 4));
        // More than there is: what there is, and the other kind left alone.
        assert_eq!(take_from([&mut near, &mut far], wood, 10), 4);
        assert_eq!((far.count(wood), far.count(stone)), (0, 1));
        assert_eq!(take_from([&mut near, &mut far], wood, 1), 0);
    }

    /// Building pays from the pack first, then from the storage in the builder's claim,
    /// nearest first, never from storage outside it, and is refused, taking nothing, when
    /// all of it together falls short (D141).
    #[test]
    fn paying_takes_the_pack_first_then_the_claims_storage_nearest_first() {
        use crate::catalogue::Builds;
        use crate::crafting::Stations;
        use crate::inventory::{Backpack, Belongings, Broke, Gained, Items, Spilled, starting_kit};
        use bevy::ecs::system::RunSystemOnce;
        use terra_walker::{LocalPlayer, Player};

        let mut world = World::new();
        world.init_resource::<Messages<Gained>>();
        world.init_resource::<Messages<Broke>>();
        world.init_resource::<Messages<Spilled>>();
        let mut items = Items::default();
        let wood = items.kind("Wood");
        let builds = Builds::read(&items, &Stations::read());
        let (mut backpack, gear, bar) = starting_kit(&mut items);
        for _ in 0..2 {
            let made = items.make(wood);
            backpack.0.insert(&items.catalogue, made, None).unwrap();
        }
        let store = |items: &mut Items, count: u32| {
            let mut grid = Grid::new(8, 6);
            for _ in 0..count {
                let made = items.make(wood);
                grid.insert(&items.catalogue, made, None).unwrap();
            }
            Storage(grid)
        };
        let centre = Vec3::new(0.0, 2000.0, 0.0);
        let mut claim = Claim::default();
        claim.stone = Some((Entity::PLACEHOLDER, centre));
        claim.set_step(0, &builds.claim);
        let half = builds.claim.across_m[0] * 0.5;
        let near = world
            .spawn((
                store(&mut items, 3),
                Transform::from_translation(centre + Vec3::X),
            ))
            .id();
        let far = world
            .spawn((
                store(&mut items, 3),
                Transform::from_translation(centre + Vec3::X * (half - 1.0)),
            ))
            .id();
        let outside = world
            .spawn((
                store(&mut items, 40),
                Transform::from_translation(centre + Vec3::X * (half + 5.0)),
            ))
            .id();
        world.insert_resource(items);
        let player = world
            .spawn((Player, LocalPlayer, backpack, gear, bar, claim))
            .id();
        let wood_in = |world: &World, entity: Entity| match world.get::<Storage>(entity) {
            Some(storage) => storage.0.count(wood),
            None => world.get::<Backpack>(entity).unwrap().0.count(wood),
        };
        let pay = move |world: &mut World, count: u32| {
            world
                .run_system_once(
                    move |mut belongings: Belongings, mut stores: Stores, claims: Query<&Claim>| {
                        let claim = claims.get(player).ok();
                        let mut carried = belongings.of(player).unwrap();
                        stores.pay(claim, centre, &mut carried, &[(wood, count)], 1)
                    },
                )
                .unwrap()
        };

        // Three: the pack's two, then one from the nearer chest.
        assert!(pay(&mut world, 3));
        let left = |world: &World| [player, near, far, outside].map(|at| wood_in(world, at));
        assert_eq!(left(&world), [0, 2, 3, 40]);
        // Six is more than the claim's storage holds, though not the chest outside it:
        // refused, and nothing taken.
        assert!(!pay(&mut world, 6));
        assert_eq!(left(&world), [0, 2, 3, 40]);
        // Four: the nearer chest's two, then two of the farther's.
        assert!(pay(&mut world, 4));
        assert_eq!(left(&world), [0, 0, 1, 40]);
    }
}
