//! Saving and loading the play experiment: what you built, what you carry and wear, your
//! body and skills, your claim, the people you brought home, the ground dug and the woods'
//! grown trees felled, which stay felled as they do while their woods stream (D622). The
//! rest of the land, its plants and creatures, are not saved: they come from the world's
//! rules, and what was cut or killed grows back, as the world heals outside claims (D94).
//!
//! The save is a RON file in its world's folder (`worlds`, D579), written every half minute,
//! on F5, and when the game closes, and read when the world is chosen. It holds the
//! character's record and one for each world they have been in (`record`, D471): a world
//! left through a gate is kept as it is left and given back when it is entered again, and a
//! game closed in another world is started again there. Scripted runs never
//! touch one unless given `--save <file>` or `--worlds=<dir>`, so a script does the same on
//! every machine; `--fresh` starts a new world, or that file over.

use super::catalogue::Builds;
use super::claim::Claim;
use super::crafting::{Recipes, Worker, Works};
use super::daylight::Clock;
use super::inventory::{self, Backpack, Bar, Gear, Items, SLOTS};
use super::people::{Doing, Person};
use super::props::{self, Models, Prop, Theirs};
use super::skills::{Skill, Skills};
use super::storage::Storage;
use super::told::Notice;
use super::vitals::Vitals;
use super::{IsLocalPlayer, Player, Stamina, View};
use crate::build::Site;
use crate::build::act::{BuildRules, Offcuts};
use crate::planet::Planet;
use crate::shell::Options;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use terra_build::Bill;
use terra_build::ops::Op;
use terra_build::save::Snapshot;
use terra_items::{ActionBar, Cell, Equipment, ItemId};
use terra_world::woods::{FellAgain, FelledWoods};

mod ground;
pub(super) mod guest;
mod record;
use ground::SavedEdit;
use record::{Kept, SaveFile};
use terra_world::instance::{InstanceApp, WorldSpec};

/// Bumped when the format changes; a save of any other version is refused, not converted
/// (D353).
const VERSION: u32 = 3;
const EVERY_S: f32 = 30.0;

/// A world's own record: what of it is not its rules' (D47).
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Instance {
    /// The base: a snapshot of it and the operations since (construction-001, gap 4).
    #[serde(default)]
    snapshot: Option<Snapshot>,
    #[serde(default)]
    since: Vec<Op>,
    /// What is left of items cut into for building (`build::act::Offcuts`).
    #[serde(default)]
    offcuts: Bill,
    /// Where the base stands: the place and turn of its grid, begun at its claim stone.
    #[serde(default)]
    frame: Option<([f32; 3], [f32; 4])>,
    props: Vec<SavedProp>,
    claim_step: usize,
    people: Vec<SavedPerson>,
    /// How the ground has been dug and filled, in order (D21).
    #[serde(default)]
    ground: Vec<SavedEdit>,
    /// Where the woods' grown trees felled stood (D622).
    #[serde(default)]
    felled: Vec<[f32; 3]>,
    /// The stumps those felled for their logs left.
    #[serde(default)]
    stumps: Vec<ground::SavedStump>,
    /// The hour of the day.
    #[serde(default)]
    hour: Option<f32>,
    /// The day of the world's clock, which the sun's path and the climate follow.
    #[serde(default)]
    day: Option<i64>,
    /// Whether the world's gate has been opened (D590): for everyone, and for good.
    #[serde(default)]
    gate_open: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct SavedPlayer {
    at: [f32; 3],
    facing: [f32; 3],
    health: f32,
    stamina: f32,
    fed: f32,
    skills: Vec<(String, f32)>,
    /// What is worn, in the order of the slots, and how many a worn container holds.
    worn: Vec<Option<String>>,
    #[serde(default)]
    worn_held: Vec<u32>,
    /// How worn what is worn is, in the order of the slots (D99).
    #[serde(default)]
    worn_wear: Vec<u16>,
    backpack: Vec<SavedItem>,
    /// For each slot of the bar, which of the backpack's items it points at.
    bar: Vec<Option<usize>>,
    picked: usize,
    /// The marks borne, by key (D590).
    #[serde(default)]
    marks: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct SavedItem {
    kind: String,
    x: u8,
    y: u8,
    /// How many a container holds, such as a quiver's arrows.
    #[serde(default)]
    held: u32,
    /// How worn it is (D99).
    #[serde(default)]
    wear: u16,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct SavedProp {
    /// The item, by name.
    item: String,
    at: [f32; 3],
    lift: f32,
    turn: f32,
    indoors: bool,
    /// A station's queue and what it has made, by what each makes, and how far into the
    /// first it is.
    #[serde(default)]
    queue: Vec<String>,
    #[serde(default)]
    progress: f32,
    #[serde(default)]
    done: Vec<String>,
    /// What storage holds (D84), as a backpack's items are kept.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    stored: Vec<SavedItem>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct SavedPerson {
    name: String,
    /// "bound", "following" or "working".
    doing: String,
    at: [f32; 3],
    spot: Option<[f32; 3]>,
}

/// Made afresh in each world entered: a save chosen is read into the world as it is entered.
#[derive(Resource, Default)]
struct Entered;

/// Where the save goes, if this run saves at all.
#[derive(Resource)]
pub(super) struct SavePath(pub(super) PathBuf);

/// What is waiting for people to arrive before it can be put back.
#[derive(Resource, Default)]
struct Waiting {
    people: Vec<SavedPerson>,
    claim_step: Option<usize>,
}

/// Which save is read and written is chosen by `worlds`, at the start or on the title
/// screen; it is read once chosen.
pub fn plugin(app: &mut App) {
    let choice = terra_world::choice::Choice::from_args(std::env::args());
    app.init_resource::<Waiting>()
        .init_resource::<Kept>()
        .init_world_resource::<Entered>()
        .insert_resource(choice)
        .add_plugins(guest::plugin)
        .add_systems(
            Startup,
            super::worlds::choose.in_set(terra_world::choice::Choosing),
        )
        .add_systems(
            Update,
            (
                // Once in the world for each save put in, at start-up or by the title (D623).
                load.run_if(resource_exists::<SavePath>),
                (
                    restore_people,
                    restore_claim,
                    (keep, autosave).chain().run_if(due),
                )
                    .run_if(resource_exists::<SavePath>),
                super::worlds::keep_the_start.run_if(resource_exists::<super::worlds::Playing>),
            )
                .chain()
                // A save is of the world: loaded into it once it is made (D623).
                .run_if(terra_world::instance::in_world),
        )
        .add_systems(
            Last,
            (keep, save_on_exit)
                .chain()
                .run_if(closing)
                .run_if(resource_exists::<SavePath>)
                .run_if(terra_world::instance::in_world),
        )
        // A world left through a gate is kept as it is left (D471).
        .add_systems(
            OnExit(terra_world::instance::Travel::Playing),
            keep.run_if(resource_exists::<SavePath>),
        );
}

#[allow(clippy::too_many_arguments)]
fn load(
    mut commands: Commands,
    path: Option<Res<SavePath>>,
    mut clock: ResMut<Clock>,
    options: Res<Options>,
    mut planet: ResMut<Planet>,
    models: Res<Models>,
    (builds, recipes): (Res<Builds>, Res<Recipes>),
    (rules, mut offcuts): (Res<BuildRules>, ResMut<Offcuts>),
    mut site: ResMut<Site>,
    mut items: ResMut<Items>,
    mut view: Single<&mut View, IsLocalPlayer>,
    (mut waiting, mut kept, entered, spec, mut returns, mut gate): (
        ResMut<Waiting>,
        ResMut<Kept>,
        Res<Entered>,
        Res<WorldSpec>,
        ResMut<super::portal::Returns>,
        Option<ResMut<super::portal::gates::Gate>>,
    ),
    (mut meshes, mut fell): (ResMut<Assets<Mesh>>, MessageWriter<FellAgain>),
    placed: Query<Entity, (With<Prop>, Without<Theirs>)>,
    player: Single<
        (
            &mut Transform,
            (&mut Vitals, &mut Stamina),
            &mut Skills,
            &mut Backpack,
            &mut Gear,
            &mut Bar,
            &mut super::marks::Marks,
        ),
        IsLocalPlayer,
    >,
) {
    let Some(path) = path else { return };
    // Once for each save chosen, and once for each world entered while one is.
    if !(path.is_added() || entered.is_added()) {
        return;
    }
    if path.is_added() || kept.file.is_none() {
        *kept = Kept {
            file: record::read(&path.0),
            placed: false,
        };
        returns.0 = kept.returns();
    }
    let place = !kept.placed;
    let Some(file) = kept.file.as_ref() else {
        // No save to put anyone by: the game begins where it stands.
        kept.placed = true;
        return;
    };
    // Saved in another world than the one the save was begun in: there first, as a crossing.
    if place && file.here.key != spec.key.0 {
        info!(
            "The saved game was left in another world ({} {}): crossing there.",
            file.here.kind, file.here.key
        );
        terra_world::loading::cross_in_the_dark(&mut commands, file.here.spec());
        return;
    }
    let instance = file.instances.get(&spec.key.0);
    let (transform, (vitals, stamina), skills, backpack, gear, bar, marks) = player.into_inner();

    if let Some(save) = instance {
        // The time of day and the date, unless a scripted run set them.
        if let (Some(hour), None) = (save.hour, options.time) {
            clock.hour = hour;
        }
        if let (Some(day), None) = (save.day, options.date) {
            clock.day = day;
        }

        // The ground as it was dug and filled (D21), and the trees felled (D622).
        ground::replay(&mut planet, &save.ground);
        ground::fell_again(&save.felled, &mut fell);
        ground::stumps_again(&mut commands, &save.stumps);

        // The base and what stands in and around it, where it was begun.
        if let Some((at, turn)) = save.frame {
            let frame =
                Transform::from_translation(Vec3::from(at)).with_rotation(Quat::from_array(turn));
            site.move_to(frame, &mut commands, &planet, &mut meshes);
        }
        if let Some(snapshot) = &save.snapshot {
            site.restore(snapshot, &save.since, &rules.rules);
        }
        offcuts.0 = save.offcuts.clone();
        for entity in &placed {
            commands.entity(entity).despawn();
        }
        for prop in &save.props {
            let item = builds.item_named(&prop.item);
            if item.is_none() {
                warn!(
                    "The save has a {}, which is not in the catalogue.",
                    prop.item
                );
            }
            let Some(item) = item else { continue };
            let Some(entity) = props::place(
                &mut commands,
                &models,
                &site,
                &planet,
                &item,
                Vec3::from(prop.at),
                prop.lift,
                prop.turn,
                prop.indoors,
            ) else {
                continue;
            };
            // Storage takes back what it held.
            if let Some((w, h)) = item.holds {
                let mut grid = terra_items::Grid::new(w, h);
                restore_items(&mut items, &mut grid, &prop.stored);
                commands.entity(entity).insert(Storage(grid));
            }
            if let Some(station) = item.station {
                // A station takes up its queue where it left off.
                let named = |names: &[String]| -> Vec<usize> {
                    names
                        .iter()
                        .filter_map(|name| recipes.named(station, name, &items))
                        .collect()
                };
                commands.entity(entity).insert(Works::resumed(
                    named(&prop.queue),
                    prop.progress,
                    named(&prop.done),
                ));
            }
        }

        // The claim grows back to its size once its stone is in place, and people take up
        // what they were doing once they arrive.
        // A gate once opened is open (D590).
        if let (true, Some(gate)) = (save.gate_open, gate.as_mut()) {
            gate.open = true;
        }
        waiting.claim_step = Some(save.claim_step);
        waiting.people = save.people.clone();
    }

    if place {
        put_player(
            &file.player,
            &options,
            &mut items,
            &mut view,
            (
                transform, vitals, stamina, skills, backpack, gear, bar, marks,
            ),
        );
        info!("Loaded the saved game from {}.", path.0.display());
    }
    kept.placed = true;
}

/// Puts the saved player back: where they stood and faced, their vitals and skills, what
/// they wear and carry, their bar and their marks.
#[allow(clippy::type_complexity)]
fn put_player(
    player: &SavedPlayer,
    options: &Options,
    items: &mut Items,
    view: &mut View,
    (
        mut transform,
        mut vitals,
        mut stamina,
        mut skills,
        mut backpack,
        mut gear,
        mut bar,
        mut marks,
    ): (
        Mut<Transform>,
        Mut<Vitals>,
        Mut<Stamina>,
        Mut<Skills>,
        Mut<Backpack>,
        Mut<Gear>,
        Mut<Bar>,
        Mut<super::marks::Marks>,
    ),
) {
    // The body, its skills, and where it stands.
    let at = Vec3::from(player.at);
    let facing = Vec3::from(player.facing);
    *transform = Transform::from_translation(at).looking_to(facing, at.normalize());
    view.forward = facing;
    vitals.health = player.health;
    stamina.left = player.stamina;
    vitals.fed = player.fed;
    *skills = Skills::default();
    for (name, progress) in &player.skills {
        if let Some(skill) = Skill::ALL.into_iter().find(|skill| skill.name() == name) {
            skills.restore(skill, *progress);
        }
    }

    // What is worn first, since the pack decides the backpack's size; then what is in it.
    let mut grid = inventory::bare_backpack();
    let mut equipment = Equipment::new(&SLOTS.map(|(slot, _)| slot));
    for name in player.worn.iter().flatten() {
        if let Some(item) = items.make_named(name)
            && grid.insert(&items.catalogue, item, None).is_ok()
        {
            let _ = inventory::wear(items, &mut grid, &mut equipment, item.id);
        }
    }
    for (&(slot, _), &held) in SLOTS.iter().zip(&player.worn_held) {
        if held > 0 {
            let _ = equipment.set_held(slot, held);
        }
    }
    for (&(slot, _), &wear) in SLOTS.iter().zip(&player.worn_wear) {
        if wear > 0 {
            let _ = equipment.set_wear(slot, wear);
        }
    }
    let ids = restore_items(items, &mut grid, &player.backpack);
    let mut action_bar = ActionBar::new(player.bar.len().max(inventory::BAR_SLOTS));
    for (slot, index) in player.bar.iter().enumerate() {
        action_bar.set(
            slot,
            index.and_then(|index| ids.get(index).copied().flatten()),
        );
    }
    action_bar.pick(player.picked);
    // For testing, --give adds to what was saved, so scripted runs can follow on.
    inventory::give_for_testing(
        items,
        &mut grid,
        &mut equipment,
        &mut action_bar,
        &options.gives,
    );
    backpack.0 = grid;
    gear.0 = equipment;
    bar.0 = action_bar;
    marks.0 = player.marks.iter().cloned().collect();
}

fn restore_claim(
    builds: Res<Builds>,
    mut waiting: ResMut<Waiting>,
    mut claim: Single<&mut Claim, IsLocalPlayer>,
) {
    if let Some(step) = waiting.claim_step
        && claim.stone.is_some()
    {
        claim.set_step(step, &builds.claim);
        waiting.claim_step = None;
    }
}

fn restore_people(
    mut commands: Commands,
    mut waiting: ResMut<Waiting>,
    mut people: Query<(Entity, &mut Person, &mut Worker, &mut Transform)>,
    player: Single<Entity, IsLocalPlayer>,
) {
    if waiting.people.is_empty() {
        return;
    }
    let mut still = Vec::new();
    for saved in waiting.people.drain(..) {
        let Some((entity, mut person, mut worker, mut transform)) = people
            .iter_mut()
            .find(|(_, person, ..)| person.name == saved.name)
        else {
            still.push(saved);
            continue;
        };
        let doing = match (saved.doing.as_str(), saved.spot) {
            ("working", Some(spot)) => Doing::Working(Vec3::from(spot)),
            // A saved game is this machine's player's, so whoever followed, follows them.
            ("following", _) => Doing::Following(*player),
            _ => continue,
        };
        person.set_doing(doing, &mut worker);
        transform.translation = Vec3::from(saved.at);
        commands
            .entity(entity)
            .remove::<super::interact::CanInteract>();
    }
    waiting.people = still;
}

/// Whether a save is due: every half minute, and on F5, which is said once written.
fn due(time: Res<Time>, keys: Res<ButtonInput<KeyCode>>, mut since: Local<f32>) -> bool {
    *since += time.delta_secs();
    if *since < EVERY_S && !keys.just_pressed(KeyCode::F5) {
        return false;
    }
    *since = 0.0;
    true
}

/// Whether the game is closing, or its world is being left for the title screen.
fn closing(mut exits: MessageReader<AppExit>, leaving: Option<Res<super::title::Leaving>>) -> bool {
    exits.read().next().is_some() || leaving.is_some()
}

/// Takes the player and the world they are in into the kept save (`record::Kept`), that
/// world's record replaced: before each writing, and as a world is left through a gate, so
/// it is as it was left when it is entered again.
#[allow(clippy::too_many_arguments)]
fn keep(
    clock: Res<Clock>,
    (planet, felled): (Res<Planet>, FelledWoods),
    (site, offcuts): (Res<Site>, Res<Offcuts>),
    claim: Single<&Claim, IsLocalPlayer>,
    items: Res<Items>,
    recipes: Res<Recipes>,
    view: Single<&View, IsLocalPlayer>,
    props: Query<(&Prop, Option<&Works>, Option<&Storage>), Without<Theirs>>,
    people: Query<(&Person, &Transform), Without<Player>>,
    player: Single<PlayerParts, IsLocalPlayer>,
    (mut kept, spec, returns, gate): (
        ResMut<Kept>,
        Res<WorldSpec>,
        Res<super::portal::Returns>,
        Option<Res<super::portal::gates::Gate>>,
    ),
) {
    let gathered = gather(
        &clock,
        (&planet, &felled),
        (&site, &offcuts),
        &claim,
        (&items, &recipes),
        &view,
        &props,
        &people,
        (&player.into_inner(), gate.is_some_and(|gate| gate.open)),
    );
    kept.keep(&spec, &returns, gathered);
}

/// Writes the kept save down: every half minute and on F5 (`due`).
fn autosave(
    path: Res<SavePath>,
    kept: Res<Kept>,
    keys: Res<ButtonInput<KeyCode>>,
    mut notice: ResMut<Notice>,
) {
    // Asked for, it is said once written.
    if kept.file.as_ref().is_some_and(|file| write(&path.0, file)) && keys.just_pressed(KeyCode::F5)
    {
        notice.say("Saved");
    }
}

/// Writes the kept save down as the game closes, or as its world is left for the title
/// screen (`closing`).
pub(super) fn save_on_exit(path: Res<SavePath>, kept: Res<Kept>) {
    if let Some(file) = &kept.file {
        write(&path.0, file);
    }
}

type PlayerParts<'a> = (
    &'a Transform,
    (&'a Vitals, &'a Stamina),
    &'a Skills,
    &'a Backpack,
    &'a Gear,
    &'a Bar,
    &'a super::marks::Marks,
);

/// Puts saved items into a grid, each at its cell or, if that is taken, wherever it fits,
/// with what it holds and how worn it is. Returns each one's identity, or none where it
/// could not be put back.
fn restore_items(
    items: &mut Items,
    grid: &mut terra_items::Grid,
    saved: &[SavedItem],
) -> Vec<Option<ItemId>> {
    let mut ids = Vec::new();
    for saved in saved {
        let id = items.make_named(&saved.kind).and_then(|item| {
            let cell = Cell {
                x: saved.x,
                y: saved.y,
            };
            grid.insert(&items.catalogue, item, Some(cell))
                .or_else(|_| grid.insert(&items.catalogue, item, None))
                .ok()
                .map(|_| item.id)
        });
        if let Some(id) = id
            && saved.held > 0
        {
            let _ = grid.set_held(id, saved.held);
        }
        if let Some(id) = id
            && saved.wear > 0
        {
            let _ = grid.set_wear(id, saved.wear);
        }
        ids.push(id);
    }
    ids
}

/// What is in a grid, to save.
fn kept_items(items: &Items, grid: &terra_items::Grid) -> Vec<SavedItem> {
    grid.iter()
        .map(|placed| SavedItem {
            kind: items.name(placed.item.kind).to_string(),
            x: placed.at.x,
            y: placed.at.y,
            held: placed.stack.map_or(0, |stack| stack.held),
            wear: placed.item.wear,
        })
        .collect()
}

/// The player as a record: where they stand, how they fare, what they wear, carry and bear.
fn saved_player(items: &Items, view: &View, player: &PlayerParts) -> SavedPlayer {
    let (at, (vitals, stamina), skills, backpack, gear, bar, marks) = *player;
    let placed: Vec<_> = backpack.0.iter().collect();
    let kept = kept_items(items, &backpack.0);
    let bar_slots = (0..bar.0.len())
        .map(|slot| {
            let id = bar.0.get(slot)?;
            placed.iter().position(|placed| placed.item.id == id)
        })
        .collect();
    SavedPlayer {
        at: at.translation.into(),
        facing: view.forward.into(),
        health: vitals.health,
        stamina: stamina.left,
        fed: vitals.fed,
        skills: skills
            .known()
            .into_iter()
            .map(|(skill, progress)| (skill.name().to_string(), progress))
            .collect(),
        worn: gear
            .0
            .iter()
            .map(|(_, item)| item.map(|item| items.name(item.kind).to_string()))
            .collect(),
        worn_held: SLOTS
            .iter()
            .map(|(slot, _)| gear.0.stack(*slot).map_or(0, |stack| stack.held))
            .collect(),
        worn_wear: SLOTS
            .iter()
            .map(|(slot, _)| gear.0.worn(*slot).map_or(0, |item| item.wear))
            .collect(),
        backpack: kept,
        bar: bar_slots,
        picked: bar.0.picked(),
        marks: marks.0.iter().cloned().collect(),
    }
}

fn gather(
    clock: &Clock,
    (planet, felled): (&Planet, &FelledWoods),
    (site, offcuts): (&Site, &Offcuts),
    claim: &Claim,
    (items, recipes): (&Items, &Recipes),
    view: &View,
    props: &Query<(&Prop, Option<&Works>, Option<&Storage>), Without<Theirs>>,
    people: &Query<(&Person, &Transform), Without<Player>>,
    (player, gate_open): (&PlayerParts, bool),
) -> (SavedPlayer, Instance) {
    let saved = saved_player(items, view, player);
    let instance = Instance {
        snapshot: Some(
            site.snapshot
                .clone()
                .unwrap_or_else(|| site.base.snapshot()),
        ),
        since: if site.snapshot.is_some() {
            site.since.clone()
        } else {
            Vec::new()
        },
        offcuts: offcuts.0.clone(),
        frame: Some((
            site.frame.translation.into(),
            site.frame.rotation.to_array(),
        )),
        props: props
            .iter()
            .map(|(prop, works, storage)| {
                let names = |indices: &mut dyn Iterator<Item = &usize>| {
                    indices
                        .map(|&index| recipes[index].makes.name(items))
                        .collect()
                };
                SavedProp {
                    item: prop.item.name.to_string(),
                    at: prop.at.into(),
                    lift: prop.lift,
                    turn: prop.turn,
                    indoors: prop.indoors,
                    queue: works.map_or(Vec::new(), |works| names(&mut works.queue.iter())),
                    progress: works.map_or(0.0, |works| works.progress),
                    done: works.map_or(Vec::new(), |works| names(&mut works.done.iter())),
                    stored: storage.map_or(Vec::new(), |storage| kept_items(items, &storage.0)),
                }
            })
            .collect(),
        claim_step: claim.step(),
        gate_open,
        hour: Some(clock.hour),
        day: Some(clock.day),
        ground: ground::kept(planet),
        felled: felled.feet().iter().map(Vec3::to_array).collect(),
        stumps: ground::stumps_kept(&felled.stumps()),
        people: people
            .iter()
            .filter_map(|(person, at)| {
                let (doing, spot) = match person.doing() {
                    Doing::Following(_) => ("following", None),
                    Doing::Working(spot) => ("working", Some(spot.into())),
                    _ => return None,
                };
                Some(SavedPerson {
                    name: person.name.to_string(),
                    doing: doing.to_string(),
                    at: at.translation.into(),
                    spot,
                })
            })
            .collect(),
    };
    (saved, instance)
}

fn write(path: &PathBuf, save: &SaveFile) -> bool {
    let text = match ron::ser::to_string_pretty(save, ron::ser::PrettyConfig::default()) {
        Ok(text) => text,
        Err(problem) => {
            warn!("Could not write the game down: {problem}");
            return false;
        }
    };
    if let Some(folder) = path.parent() {
        let _ = std::fs::create_dir_all(folder);
    }
    // Write beside it and swap, so a crash mid-write never leaves half a save.
    let fresh = path.with_extension("ron.new");
    match std::fs::write(&fresh, text).and_then(|()| std::fs::rename(&fresh, path)) {
        Ok(()) => true,
        Err(problem) => {
            warn!("Could not save to {}: {problem}", path.display());
            false
        }
    }
}

#[cfg(test)]
mod tests;
