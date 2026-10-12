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
use crate::planet::Planet;
use crate::shell::Options;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use terra_build::BuildOp;
use terra_core::{MaterialId, Operation};
use terra_items::{ActionBar, Cell, Equipment, ItemId};

/// Bumped when the format changes in a way old saves cannot be read in.
const VERSION: u32 = 1;
const EVERY_S: f32 = 30.0;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct SaveFile {
    version: u32,
    player: SavedPlayer,
    base: Vec<BuildOp>,
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
    file: String,
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
    (builds, recipes): (Res<Builds>, Res<Recipes>),
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
    site.replace_with(&save.base, &mut commands, &planet, &mut meshes);
    for entity in &placed {
        commands.entity(entity).despawn();
    }
    for prop in &save.props {
        if let Some(item) = builds.item_from(&prop.file)
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
    site: Res<Site>,
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
        &site,
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
    site: Res<Site>,
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
            &site,
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
    site: &Site,
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
        base: site.operations.clone(),
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
                    file: prop.item.file.to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_save_reads_back_as_it_was_written() {
        let save = SaveFile {
            version: VERSION,
            player: SavedPlayer {
                at: [1.0, 2000.0, 3.0],
                facing: [0.0, 0.0, 1.0],
                health: 80.0,
                stamina: 50.0,
                fed: 60.0,
                skills: vec![("Chopping".to_string(), 2.5)],
                worn: vec![Some("Small pack".to_string()), None, None, None],
                worn_wear: vec![0, 0, 3, 0],
                worn_held: vec![0, 0, 0, 0],
                backpack: vec![SavedItem {
                    kind: "Wood".to_string(),
                    x: 1,
                    y: 2,
                    held: 0,
                    wear: 12,
                }],
                bar: vec![Some(0), None],
                picked: 0,
            },
            base: vec![BuildOp::AddRoom {
                min: [0, 0, 0],
                max: [2, 0, 2],
            }],
            frame: Some(([1.0, 2000.0, 2.0], [0.0, 0.0, 0.0, 1.0])),
            props: vec![SavedProp {
                file: "Table_Small".to_string(),
                at: [1.0, 0.0, 2.0],
                lift: 0.0,
                turn: 20.0,
                indoors: false,
                queue: vec!["Iron".to_string()],
                progress: 2.5,
                done: vec!["Iron".to_string()],
            }],
            claim_step: 1,
            hour: Some(21.5),
            ground: vec![
                SavedEdit {
                    centre: [1.0, 2000.0, 2.0],
                    radius: 0.7,
                    fill: None,
                    cut: None,
                },
                SavedEdit {
                    centre: [1.0, 2000.0, 2.0],
                    radius: 0.0,
                    fill: None,
                    cut: Some(SavedBox {
                        half: [1.0, 1.5, 2.0],
                        rotation: [0.0, 0.0, 0.0, 1.0],
                    }),
                },
            ],
            people: vec![SavedPerson {
                name: "Bren".to_string(),
                doing: "working".to_string(),
                at: [0.0, 2000.0, 0.0],
                spot: Some([1.0, 2000.0, 1.0]),
            }],
        };
        let text = ron::ser::to_string_pretty(&save, ron::ser::PrettyConfig::default()).unwrap();
        let back: SaveFile = ron::from_str(&text).unwrap();
        assert_eq!(back, save);
    }
}
