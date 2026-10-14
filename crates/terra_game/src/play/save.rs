//! Saving and loading the play experiment: what you built, what you carry and wear, your
//! body and skills, your claim, and the people you brought home. The land, its plants and
//! creatures are not saved: they come from the world's rules, and what was cut or killed
//! grows back, as the world heals outside claims (D94).
//!
//! The save is a RON file beside the settings, written every half minute, on F5, and when
//! the game closes, and read when it starts. Scripted runs never touch it unless given
//! `--save <file>`, so a script does the same on every machine; `--fresh` starts over.

use super::catalogue::Builds;
use super::claim::Claim;
use super::controls::Settings;
use super::crafting::{Recipes, Works};
use super::daylight::Clock;
use super::inventory::{self, Backpack, Bar, Gear, Items, SLOTS};
use super::people::{Doing, Person};
use super::props::{self, Models, Prop, Theirs};
use super::skills::{Skill, Skills};
use super::told::Notice;
use super::vitals::Vitals;
use super::{IsLocalPlayer, Player, View};
use crate::build::Site;
use crate::build::act::{BuildRules, Offcuts};
use crate::planet::Planet;
use crate::shell::Options;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use terra_build::Bill;
use terra_build::save::{Change, SavedOp, Snapshot, convert};
use terra_core::{MaterialId, Operation};
use terra_items::{ActionBar, Cell, Equipment, ItemId};

/// Bumped when the format changes in a way old saves cannot be read in.
const VERSION: u32 = 1;
const EVERY_S: f32 = 30.0;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct SaveFile {
    version: u32,
    player: SavedPlayer,
    /// The base, as saves before snapshots held it: the shortcuts that made it, read and
    /// turned into cubes, and never written.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    base: Vec<SavedOp>,
    /// The base: a snapshot of it and the operations since (construction-001, gap 4).
    #[serde(default)]
    snapshot: Option<Snapshot>,
    #[serde(default)]
    since: Vec<Change>,
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
    /// The hour of the day.
    #[serde(default)]
    hour: Option<f32>,
}

/// One change to the ground: a sphere dug out, or filled with a material, or a box dug
/// out.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct SavedEdit {
    centre: [f32; 3],
    radius: f32,
    /// The material it was filled with, for a fill; nothing for a dig.
    #[serde(default)]
    fill: Option<u16>,
    /// For a box, half its size each way and how it is turned; the radius is then unused.
    #[serde(default)]
    cut: Option<SavedBox>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct SavedBox {
    half: [f32; 3],
    rotation: [f32; 4],
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
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
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct SavedProp {
    /// The item, by name. Saves before names were kept give its model's file instead, which
    /// finds it while the model is the same one.
    #[serde(alias = "file")]
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
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct SavedPerson {
    name: String,
    /// "bound", "following" or "working".
    doing: String,
    at: [f32; 3],
    spot: Option<[f32; 3]>,
}

/// Where the save goes, if this run saves at all.
#[derive(Resource)]
struct SavePath(PathBuf);

/// What is waiting for people to arrive before it can be put back.
#[derive(Resource, Default)]
struct Waiting {
    people: Vec<SavedPerson>,
    claim_step: Option<usize>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Waiting>()
        .add_systems(Startup, choose_path)
        .add_systems(
            Update,
            (
                load.run_if(run_once),
                (restore_people, restore_claim, autosave).run_if(resource_exists::<SavePath>),
            )
                .chain(),
        )
        .add_systems(Last, save_on_exit.run_if(resource_exists::<SavePath>));
}

fn choose_path(mut commands: Commands, options: Res<Options>) {
    let path = match &options.save {
        // The yard, the flat test level, is never saved, and never touches a save.
        _ if options.yard() => None,
        Some(path) => Some(path.clone()),
        None if options.scripted() => None,
        None => Settings::path().map(|settings| settings.with_file_name("save.ron")),
    };
    if let Some(path) = path {
        if options.fresh {
            let _ = std::fs::remove_file(&path);
        }
        commands.insert_resource(SavePath(path));
    }
}

#[allow(clippy::too_many_arguments)]
fn load(
    mut commands: Commands,
    path: Option<Res<SavePath>>,
    mut clock: ResMut<Clock>,
    options: Res<crate::shell::Options>,
    mut planet: ResMut<Planet>,
    models: Res<Models>,
    (builds, recipes, look): (Res<Builds>, Res<Recipes>, Res<crate::build::BuildLook>),
    (rules, mut offcuts): (Res<BuildRules>, ResMut<Offcuts>),
    mut site: ResMut<Site>,
    mut items: ResMut<Items>,
    mut view: Single<&mut View, IsLocalPlayer>,
    mut waiting: ResMut<Waiting>,
    mut meshes: ResMut<Assets<Mesh>>,
    placed: Query<Entity, (With<Prop>, Without<Theirs>)>,
    player: Single<
        (
            &mut Transform,
            &mut Vitals,
            &mut Skills,
            &mut Backpack,
            &mut Gear,
            &mut Bar,
        ),
        IsLocalPlayer,
    >,
) {
    let Some(path) = path else { return };
    let Ok(text) = std::fs::read_to_string(&path.0) else {
        info!("No saved game at {}; starting afresh.", path.0.display());
        return;
    };
    let save: SaveFile = match ron::from_str(&text) {
        Ok(save) => save,
        Err(problem) => {
            warn!(
                "Could not read the saved game at {}: {problem}",
                path.0.display()
            );
            return;
        }
    };
    if save.version != VERSION {
        warn!("The saved game is from an older version; starting afresh.");
        return;
    }
    let (mut transform, mut vitals, mut skills, mut backpack, mut gear, mut bar) =
        player.into_inner();

    // The time of day, unless a scripted run set it.
    if let (Some(hour), None) = (save.hour, options.time) {
        clock.hour = hour;
    }

    // The ground as it was dug and filled, replayed in order (D21).
    if planet.operations().is_empty() && !save.ground.is_empty() {
        info!("Replayed {} changes to the ground.", save.ground.len());
        for edit in &save.ground {
            let centre = Vec3::from(edit.centre);
            planet.apply(&match (&edit.cut, edit.fill) {
                (Some(cut), _) => Operation::DigBox {
                    centre,
                    half: Vec3::from(cut.half),
                    rotation: Quat::from_array(cut.rotation),
                },
                (None, None) => Operation::Dig {
                    centre,
                    radius: edit.radius,
                },
                (None, Some(material)) => Operation::Fill {
                    centre,
                    radius: edit.radius,
                    material: MaterialId(material),
                },
            });
        }
    }

    // The base and what stands in and around it, where it was begun.
    if let Some((at, turn)) = save.frame {
        let frame =
            Transform::from_translation(Vec3::from(at)).with_rotation(Quat::from_array(turn));
        site.move_to(frame, &mut commands, &planet, &mut meshes);
    }
    match &save.snapshot {
        Some(snapshot) => site.restore(snapshot, &save.since, &rules.rules, &|walls| {
            roof_for(&look, walls)
        }),
        None => {
            // A save from before snapshots: the shortcuts that made it, laid as built, its
            // roofs from before roofs were cubes laid in the roof its walls take (D241).
            let converted = convert(&save.base, look.laying);
            if converted.left_out > 0 {
                warn!(
                    "{} of the saved base's rooms had a wall pushed or pulled; those changes are left out.",
                    converted.left_out
                );
            }
            let base = converted.base(&|walls| roof_for(&look, walls));
            site.restore(&base.snapshot(), &[], &rules.rules, &|walls| {
                roof_for(&look, walls)
            });
        }
    }
    offcuts.0 = save.offcuts.clone();
    for entity in &placed {
        commands.entity(entity).despawn();
    }
    for prop in &save.props {
        let item = builds
            .item_named(&prop.item)
            .or_else(|| builds.item_from(&prop.item));
        if item.is_none() {
            warn!(
                "The save has a {}, which is not in the catalogue.",
                prop.item
            );
        }
        if let Some(item) = item
            && let Some(entity) = props::place(
                &mut commands,
                &models,
                &site,
                &planet,
                &item,
                Vec3::from(prop.at),
                prop.lift,
                prop.turn,
                prop.indoors,
            )
            && let Some(station) = item.station
        {
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

    // The body, its skills, and where it stands.
    let player = &save.player;
    let at = Vec3::from(player.at);
    let facing = Vec3::from(player.facing);
    *transform = Transform::from_translation(at).looking_to(facing, at.normalize());
    view.forward = facing;
    vitals.health = player.health;
    vitals.stamina = player.stamina;
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
            let _ = inventory::wear(&items, &mut grid, &mut equipment, item.id);
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
    let mut ids: Vec<Option<ItemId>> = Vec::new();
    for saved in &player.backpack {
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
        &mut items,
        &mut grid,
        &mut equipment,
        &mut action_bar,
        &options.gives,
    );
    backpack.0 = grid;
    gear.0 = equipment;
    bar.0 = action_bar;

    // The claim grows back to its size once its stone is in place, and people take up
    // what they were doing once they arrive.
    waiting.claim_step = Some(save.claim_step);
    waiting.people = save.people;
    info!("Loaded the saved game from {}.", path.0.display());
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
    mut people: Query<(Entity, &mut Person, &mut Transform)>,
    player: Single<Entity, IsLocalPlayer>,
) {
    if waiting.people.is_empty() {
        return;
    }
    let mut still = Vec::new();
    for saved in waiting.people.drain(..) {
        let Some((entity, mut person, mut transform)) = people
            .iter_mut()
            .find(|(_, person, _)| person.name == saved.name)
        else {
            still.push(saved);
            continue;
        };
        person.doing = match (saved.doing.as_str(), saved.spot) {
            ("working", Some(spot)) => Doing::Working(Vec3::from(spot)),
            // A saved game is this machine's player's, so whoever followed, follows them.
            ("following", _) => Doing::Following(*player),
            _ => continue,
        };
        transform.translation = Vec3::from(saved.at);
        commands
            .entity(entity)
            .remove::<super::interact::CanInteract>();
    }
    waiting.people = still;
}

/// Writes the game down every half minute and on F5.
#[allow(clippy::too_many_arguments)]
fn autosave(
    clock: Res<Clock>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    path: Res<SavePath>,
    planet: Res<Planet>,
    (site, offcuts): (Res<Site>, Res<Offcuts>),
    claim: Single<&Claim, IsLocalPlayer>,
    items: Res<Items>,
    recipes: Res<Recipes>,
    view: Single<&View, IsLocalPlayer>,
    props: Query<(&Prop, Option<&Works>), Without<Theirs>>,
    people: Query<(&Person, &Transform), Without<Player>>,
    player: Single<(&Transform, &Vitals, &Skills, &Backpack, &Gear, &Bar), IsLocalPlayer>,
    mut since: Local<f32>,
    mut notice: ResMut<Notice>,
) {
    *since += time.delta_secs();
    let asked = keys.just_pressed(KeyCode::F5);
    if *since < EVERY_S && !asked {
        return;
    }
    *since = 0.0;
    let save = gather(
        &clock,
        &planet,
        (&site, &offcuts),
        &claim,
        (&items, &recipes),
        &view,
        &props,
        &people,
        &player.into_inner(),
    );
    if write(&path.0, &save) && asked {
        notice.say("Saved");
    }
}

/// Writes the game down as it closes.
#[allow(clippy::too_many_arguments)]
fn save_on_exit(
    clock: Res<Clock>,
    mut exits: MessageReader<AppExit>,
    path: Res<SavePath>,
    planet: Res<Planet>,
    (site, offcuts): (Res<Site>, Res<Offcuts>),
    claim: Single<&Claim, IsLocalPlayer>,
    items: Res<Items>,
    recipes: Res<Recipes>,
    view: Single<&View, IsLocalPlayer>,
    props: Query<(&Prop, Option<&Works>), Without<Theirs>>,
    people: Query<(&Person, &Transform), Without<Player>>,
    player: Single<(&Transform, &Vitals, &Skills, &Backpack, &Gear, &Bar), IsLocalPlayer>,
) {
    if exits.read().next().is_some() {
        let save = gather(
            &clock,
            &planet,
            (&site, &offcuts),
            &claim,
            (&items, &recipes),
            &view,
            &props,
            &people,
            &player.into_inner(),
        );
        write(&path.0, &save);
    }
}

type PlayerParts<'a> = (
    &'a Transform,
    &'a Vitals,
    &'a Skills,
    &'a Backpack,
    &'a Gear,
    &'a Bar,
);

fn gather(
    clock: &Clock,
    planet: &Planet,
    (site, offcuts): (&Site, &Offcuts),
    claim: &Claim,
    (items, recipes): (&Items, &Recipes),
    view: &View,
    props: &Query<(&Prop, Option<&Works>), Without<Theirs>>,
    people: &Query<(&Person, &Transform), Without<Player>>,
    player: &PlayerParts,
) -> SaveFile {
    let (at, vitals, skills, backpack, gear, bar) = *player;
    let placed: Vec<_> = backpack.0.iter().collect();
    let kept: Vec<SavedItem> = placed
        .iter()
        .map(|placed| SavedItem {
            kind: items.name(placed.item.kind).to_string(),
            x: placed.at.x,
            y: placed.at.y,
            held: placed.stack.map_or(0, |stack| stack.held),
            wear: placed.item.wear,
        })
        .collect();
    let bar_slots = (0..bar.0.len())
        .map(|slot| {
            let id = bar.0.get(slot)?;
            placed.iter().position(|placed| placed.item.id == id)
        })
        .collect();
    SaveFile {
        version: VERSION,
        player: SavedPlayer {
            at: at.translation.into(),
            facing: view.forward.into(),
            health: vitals.health,
            stamina: vitals.stamina,
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
        },
        base: Vec::new(),
        snapshot: Some(
            site.snapshot
                .clone()
                .unwrap_or_else(|| site.base.snapshot()),
        ),
        since: if site.snapshot.is_some() {
            site.since.iter().cloned().map(Change::Now).collect()
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
            .map(|(prop, works)| {
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
                }
            })
            .collect(),
        claim_step: claim.step(),
        hour: Some(clock.hour),
        ground: planet
            .operations()
            .into_iter()
            .filter_map(|operation| match operation {
                Operation::Dig { centre, radius } => Some(SavedEdit {
                    centre: centre.into(),
                    radius,
                    fill: None,
                    cut: None,
                }),
                Operation::DigBox {
                    centre,
                    half,
                    rotation,
                } => Some(SavedEdit {
                    centre: centre.into(),
                    radius: 0.0,
                    fill: None,
                    cut: Some(SavedBox {
                        half: half.into(),
                        rotation: rotation.to_array(),
                    }),
                }),
                Operation::Fill {
                    centre,
                    radius,
                    material,
                } => Some(SavedEdit {
                    centre: centre.into(),
                    radius,
                    fill: Some(material.0),
                    cut: None,
                }),
                _ => None,
            })
            .collect(),
        people: people
            .iter()
            .filter_map(|(person, at)| {
                let (doing, spot) = match person.doing {
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
    }
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

/// The roof a base's walls of a kind take, as its kit says (D241): an old save's roofs are
/// laid in it.
fn roof_for(look: &crate::build::BuildLook, walls: terra_build::Kind) -> terra_build::Kind {
    look.kits
        .as_ref()
        .and_then(|kits| kits.roof_for(walls))
        .unwrap_or(terra_build::Kind::FIRST_ROOF)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::DrivenByPlayer;
    use crate::play::{Flora, LocalPlayer, registries};
    use crate::start::Start;
    use bevy::ecs::system::RunSystemOnce;
    use terra_build::BuildOp;
    use terra_core::{PlanetSpec, Seed};

    /// A world with what saving and loading need: the planet, the registries, the base
    /// and the props' models, set up as the game sets them up, with no one in it yet.
    fn game() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<Image>();
        let planet = Planet::new(PlanetSpec {
            seed: Seed(1),
            radius_m: 2000.0,
        });
        app.insert_resource(Options::from_args(std::iter::empty()).unwrap())
            .insert_resource(Start(Transform::from_translation(planet.start())))
            .insert_resource(planet)
            .insert_resource(DrivenByPlayer)
            .insert_resource(Flora::read())
            .init_resource::<Clock>()
            .init_resource::<Waiting>();
        registries::plugin(&mut app);
        crate::build::plugin(&mut app);
        props::plugin(&mut app);
        app.world_mut().run_schedule(Startup);
        app
    }

    /// This machine's player as they arrive: what they start with, and no claim yet.
    fn arrive(world: &mut World) -> Entity {
        let at = world.resource::<Planet>().start();
        let kit = inventory::starting_kit(&mut world.resource_mut::<Items>());
        world
            .spawn((
                Player,
                LocalPlayer,
                Transform::from_translation(at),
                View {
                    forward: Vec3::X,
                    pitch: 0.0,
                },
                Vitals::default(),
                Skills::default(),
                kit,
                Claim::default(),
            ))
            .id()
    }

    /// A while of play: walked off and turned, hurt and tired, skilled, wearing a
    /// container half full and something worn down, a tool used and a spare container
    /// carried with a few in it, the bar rearranged, a base begun
    /// and built, a station at work, the claim grown, the ground dug and filled, someone
    /// at work and someone following, and the evening come.
    fn play_a_while(world: &mut World, player: Entity) {
        let start = world.resource::<Planet>().start();
        let up = start.normalize();
        let east = up.any_orthonormal_vector();
        let north = up.cross(east);

        world.resource_mut::<Clock>().hour = 21.5;
        let mut planet = world.resource_mut::<Planet>();
        planet.apply(&Operation::Dig {
            centre: start + east * 4.0,
            radius: 0.7,
        });
        planet.apply(&Operation::Fill {
            centre: start + east * 6.0,
            radius: 0.5,
            material: MaterialId(2),
        });
        planet.apply(&Operation::DigBox {
            centre: start + north * 5.0,
            half: Vec3::new(1.0, 1.5, 2.0),
            rotation: Quat::from_rotation_arc(Vec3::Y, up),
        });

        // The body, where it is and what it knows.
        let mut entity = world.entity_mut(player);
        *entity.get_mut::<Transform>().unwrap() =
            Transform::from_translation(start + east * 3.0 + north * 2.0);
        entity.get_mut::<View>().unwrap().forward = (east + north).normalize();
        let mut vitals = entity.get_mut::<Vitals>().unwrap();
        vitals.health = 61.5;
        vitals.stamina = 12.25;
        vitals.fed = 40.0;
        let mut skills = entity.get_mut::<Skills>().unwrap();
        for _ in 0..9 {
            skills.did(Skill::Chopping, 1.0);
        }
        skills.did(Skill::Mining, 1.0);

        // A worn container, such as a quiver, put on and half filled, and something worn
        // that wears, worn down.
        world.resource_scope(|world, mut items: Mut<Items>| {
            let (container, (holds, most)) = items
                .kinds()
                .find_map(|kind| Some((kind, items.worn(kind)?.holds.clone()?)))
                .expect("something worn holds things");
            let wearing = items
                .kinds()
                .find(|&kind| items.worn(kind).is_some() && items.def(kind).lasts.is_some())
                .expect("something worn wears");
            let holds = items.kind(&holds);
            let mut entity = world.entity_mut(player);
            let (mut backpack, mut gear, mut bar) = (
                entity.get::<Backpack>().unwrap().0.clone(),
                entity.get::<Gear>().unwrap().0.clone(),
                entity.get::<Bar>().unwrap().0.clone(),
            );
            for kind in [container, wearing] {
                let made = items.make(kind);
                backpack.insert(&items.catalogue, made, None).unwrap();
                inventory::wear(&items, &mut backpack, &mut gear, made.id).unwrap();
            }
            let (slot, _) = gear
                .iter()
                .find(|(_, worn)| worn.is_some_and(|worn| worn.kind == wearing))
                .unwrap();
            gear.wear(&items.catalogue, slot, 3).unwrap();
            assert_eq!(gear.stow(holds, most / 2), most / 2);
            // A spare one, carried in the backpack with a few in it.
            let spare = items.make(container);
            backpack.insert(&items.catalogue, spare, None).unwrap();
            assert_eq!(backpack.stow(holds, 2), 2);
            // A tool worn down, the first slot of the bar emptied, and the last picked.
            let tool = bar.get(1).unwrap();
            backpack.wear(&items.catalogue, tool, 7).unwrap();
            bar.set(0, None);
            bar.pick(bar.len() - 1);
            entity.insert((Backpack(backpack), Gear(gear), Bar(bar)));
        });

        // The base, begun away from where it was first laid, with a room in it.
        let frame = Transform::from_translation(start + north * 12.0)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, up));
        world
            .run_system_once(
                move |mut commands: Commands,
                      mut site: ResMut<Site>,
                      look: Res<crate::build::BuildLook>,
                      planet: Res<Planet>,
                      mut meshes: ResMut<Assets<Mesh>>| {
                    site.move_to(frame, &mut commands, &planet, &mut meshes);
                    let room = BuildOp::AddRoom {
                        min: [0, 0, 0],
                        max: [2, 0, 2],
                        kind: look.laying,
                    };
                    assert!(site.apply(room, &mut commands, &planet, &mut meshes));
                },
            )
            .unwrap();

        // A station outside it, with things queued, one under way and one made.
        world
            .run_system_once(
                |mut commands: Commands,
                 builds: Res<Builds>,
                 recipes: Res<Recipes>,
                 models: Res<Models>,
                 site: Res<Site>,
                 planet: Res<Planet>| {
                    let item = builds
                        .items()
                        .find(|item| item.station.is_some())
                        .expect("something placed is a station");
                    let makes: Vec<usize> = recipes.at(item.station.unwrap()).collect();
                    assert!(!makes.is_empty(), "{} makes nothing", item.name);
                    let at = Vec3::new(-3.0, 0.0, 1.5);
                    let station = props::place(
                        &mut commands,
                        &models,
                        &site,
                        &planet,
                        item,
                        at,
                        0.25,
                        30.0,
                        false,
                    )
                    .expect("the station's model is there");
                    let queue = vec![makes[makes.len() - 1], makes[0]];
                    commands
                        .entity(station)
                        .insert(Works::resumed(queue, 1.5, vec![makes[0]]));
                },
            )
            .unwrap();

        // The claim, its stone set and grown to its largest.
        world.resource_scope(|world, builds: Mut<Builds>| {
            let mut claim = world.get_mut::<Claim>(player).unwrap();
            claim.stone = Some((Entity::PLACEHOLDER, frame.translation));
            claim.set_step(builds.claim.across_m.len() - 1, &builds.claim);
            assert!(claim.step() > 0, "a claim that cannot grow tells nothing");
        });

        // People: one at work, one following, and one still bound, who is not kept.
        world.spawn((
            Person::new("Bren", Doing::Working(start + east * 9.0), None),
            Transform::from_translation(start + east * 8.0),
        ));
        world.spawn((
            Person::new("Tam", Doing::Following(player), None),
            Transform::from_translation(start + north * 4.0),
        ));
        world.spawn((
            Person::new("Ash", Doing::Bound, None),
            Transform::from_translation(start + north * 30.0),
        ));
    }

    /// Everyone the save keeps, as they might be before it is loaded: elsewhere, and
    /// still bound.
    fn waiting_to_be_found(world: &mut World) {
        let start = world.resource::<Planet>().start();
        for name in ["Bren", "Tam"] {
            world.spawn((
                Person::new(name, Doing::Bound, None),
                Transform::from_translation(start + Vec3::X * 50.0),
            ));
        }
    }

    /// Everything a save should keep, read from a world's own components and
    /// resources, never through the save.
    #[derive(Debug, PartialEq)]
    struct Kept {
        at: Vec3,
        facing: Vec3,
        body: [f32; 3],
        skills: Skills,
        /// Each slot's kind, its wear, and how many it holds.
        worn: Vec<Option<(String, u16, Option<u32>)>>,
        /// Each item carried: its kind, where, its wear, and how many it holds.
        carried: Vec<(String, (u8, u8), u16, Option<u32>)>,
        /// What each slot of the bar points at, by where it is carried.
        bar: Vec<Option<(u8, u8)>>,
        picked: usize,
        base: Snapshot,
        frame: Transform,
        /// Each station or thing placed, with its queue, progress and what it has made.
        props: Vec<(
            String,
            Vec3,
            f32,
            f32,
            bool,
            Option<(Vec<usize>, f32, Vec<usize>)>,
        )>,
        claim: usize,
        hour: f32,
        ground: Vec<Operation>,
        people: Vec<(&'static str, String, Vec3)>,
    }

    fn kept(world: &mut World) -> Kept {
        let mut players = world.query_filtered::<Entity, IsLocalPlayer>();
        let player = players.single(world).unwrap();
        let entity = world.entity(player);
        let items = world.resource::<Items>();
        let backpack = &entity.get::<Backpack>().unwrap().0;
        let gear = &entity.get::<Gear>().unwrap().0;
        let bar = &entity.get::<Bar>().unwrap().0;
        let vitals = entity.get::<Vitals>().unwrap();
        let site = world.resource::<Site>();
        let mut kept = Kept {
            at: entity.get::<Transform>().unwrap().translation,
            facing: entity.get::<View>().unwrap().forward,
            body: [vitals.health, vitals.stamina, vitals.fed],
            skills: entity.get::<Skills>().unwrap().clone(),
            worn: gear
                .iter()
                .map(|(slot, worn)| {
                    let held = gear.stack(slot).map(|stack| stack.held);
                    worn.map(|worn| (items.name(worn.kind).to_string(), worn.wear, held))
                })
                .collect(),
            carried: backpack
                .iter()
                .map(|placed| {
                    (
                        items.name(placed.item.kind).to_string(),
                        (placed.at.x, placed.at.y),
                        placed.item.wear,
                        placed.stack.map(|stack| stack.held),
                    )
                })
                .collect(),
            bar: (0..bar.len())
                .map(|slot| {
                    let placed = backpack.get(bar.get(slot)?)?;
                    Some((placed.at.x, placed.at.y))
                })
                .collect(),
            picked: bar.picked(),
            base: site.base.snapshot(),
            frame: site.frame,
            props: Vec::new(),
            claim: entity.get::<Claim>().unwrap().step(),
            hour: world.resource::<Clock>().hour,
            ground: world.resource::<Planet>().operations(),
            people: Vec::new(),
        };
        let mut props = world.query::<(&Prop, Option<&Works>)>();
        kept.props = props
            .iter(world)
            .map(|(prop, works)| {
                (
                    prop.item.name.clone(),
                    prop.at,
                    prop.lift,
                    prop.turn,
                    prop.indoors,
                    works.map(|works| {
                        let queue = works.queue.iter().copied().collect();
                        (queue, works.progress, works.done.clone())
                    }),
                )
            })
            .collect();
        kept.props.sort_by(|a, b| a.0.cmp(&b.0));
        let mut people = world.query::<(&Person, &Transform)>();
        kept.people = people
            .iter(world)
            .map(|(person, at)| {
                let doing = match person.doing {
                    Doing::Following(whom) if whom == player => "following the player".into(),
                    Doing::Working(spot) => format!("working at {spot}"),
                    ref other => format!("{other:?}"),
                };
                (person.name, doing, at.translation)
            })
            .collect();
        kept.people.sort_by(|a, b| a.0.cmp(b.0));
        kept
    }

    /// Saves a world the way the game does, to a file of its own.
    fn save(world: &mut World, name: &str) -> PathBuf {
        let save = world
            .run_system_once(
                |clock: Res<Clock>,
                 planet: Res<Planet>,
                 site: Res<Site>,
                 offcuts: Res<Offcuts>,
                 claim: Single<&Claim, IsLocalPlayer>,
                 items: Res<Items>,
                 recipes: Res<Recipes>,
                 view: Single<&View, IsLocalPlayer>,
                 props: Query<(&Prop, Option<&Works>), Without<Theirs>>,
                 people: Query<(&Person, &Transform), Without<Player>>,
                 player: Single<
                    (&Transform, &Vitals, &Skills, &Backpack, &Gear, &Bar),
                    IsLocalPlayer,
                >| {
                    gather(
                        &clock,
                        &planet,
                        (&site, &offcuts),
                        &claim,
                        (&items, &recipes),
                        &view,
                        &props,
                        &people,
                        &player.into_inner(),
                    )
                },
            )
            .unwrap();
        let path = std::env::temp_dir().join(format!(
            "terra-{name}-{}-{:?}.ron",
            std::process::id(),
            std::thread::current().id()
        ));
        assert!(write(&path, &save));
        path
    }

    /// Loads a save the way the game does as it starts, and lets the claim and people
    /// take up where they were once their stone and they are there.
    fn load_from(world: &mut World, path: &std::path::Path) {
        world.insert_resource(SavePath(path.to_path_buf()));
        world.run_system_once(load).unwrap();
        world.run_system_once(restore_claim).unwrap();
        world.run_system_once(restore_people).unwrap();
    }

    #[test]
    fn a_world_saved_and_loaded_into_another_is_the_same_world() {
        let mut played = game();
        let world = played.world_mut();
        let player = arrive(world);
        play_a_while(world, player);
        let before = kept(world);
        // What the world was given to keep is there to keep.
        assert!(before.worn.iter().flatten().any(|(_, wear, _)| *wear > 0));
        assert!(
            before
                .worn
                .iter()
                .flatten()
                .any(|(_, _, held)| *held > Some(0))
        );
        assert!(before.carried.iter().any(|(_, _, wear, _)| *wear > 0));
        assert!(before.carried.iter().any(|(_, _, _, held)| *held > Some(0)));
        assert_eq!(before.bar[0], None);
        assert!(before.props.iter().any(|(.., works)| works.is_some()));
        assert_eq!(before.ground.len(), 3);
        assert_eq!(before.people.len(), 3);
        let path = save(world, "round-trip");

        let mut fresh = game();
        let world = fresh.world_mut();
        let player = arrive(world);
        waiting_to_be_found(world);
        load_from(world, &path);
        // The claim waits for its stone before it grows back.
        assert_eq!(world.get::<Claim>(player).unwrap().step(), 0);
        let stone = before.frame.translation;
        world.get_mut::<Claim>(player).unwrap().stone = Some((Entity::PLACEHOLDER, stone));
        world.run_system_once(restore_claim).unwrap();
        let _ = std::fs::remove_file(&path);

        // Whoever was still bound is not kept: they are found again where they are.
        let mut before = before;
        before.people.retain(|(name, ..)| *name != "Ash");
        assert_eq!(kept(world), before);
    }

    #[test]
    fn a_save_from_another_version_is_left_alone() {
        let mut played = game();
        let world = played.world_mut();
        let player = arrive(world);
        play_a_while(world, player);
        let path = save(world, "other-version");
        let text = std::fs::read_to_string(&path).unwrap();
        let mut save: SaveFile = ron::from_str(&text).unwrap();
        save.version = VERSION + 1;
        assert!(write(&path, &save));

        let mut fresh = game();
        let world = fresh.world_mut();
        arrive(world);
        let before = kept(world);
        load_from(world, &path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(kept(world), before);
    }

    #[test]
    fn a_save_from_before_snapshots_holds_the_shortcuts_that_made_its_base() {
        let text = "(version: 1, player: (at: (0, 0, 0), facing: (0, 0, 1), health: 100, \
            stamina: 100, fed: 100, skills: [], worn: [], backpack: [], bar: [], picked: 0), \
            base: [Op(AddRoom(min: (0, 0, 0), max: (3, 0, 3), kind: (1)))], props: [], \
            claim_step: 0, people: [])";
        let save: SaveFile = ron::from_str(text).unwrap();
        assert_eq!(save.snapshot, None);
        let converted = convert(&save.base, terra_build::Kind(1));
        assert_eq!(
            terra_build::Base::from_shortcuts(&converted.ops).room_cubes(),
            4 * 5
        );
    }
}
