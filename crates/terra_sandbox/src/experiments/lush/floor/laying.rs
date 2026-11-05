//! How the floor lays one thing: where on the ground, whether a place is open to it, the
//! scans and materials it is drawn with, and a plant spawned with its shadow filed.

use super::*;

impl Lay<'_, '_, '_> {
    /// What a place is for a kind that grows in the sun's patches, the verge's bracken: in
    /// the sun where it reaches the fronds' height past the crowns' thick, by the one test of
    /// what a crown hides (`SkyOccluders::hides`).
    pub(super) fn in_sun(&self, kind: Under, at: Vec2, mut here: Here) -> Here {
        if kind == Under::Verge
            && let Some((crowns, sun)) = self.sun
        {
            let ground = self
                .planet
                .ground_at(self.start + self.east * at.x + self.south * at.y);
            let fronds = ground + ground.normalize() * FRONDS_M;
            here.sun = if crowns.sun_reaches(fronds, sun, SUN_LOOKS_M) {
                1.0
            } else {
                0.0
            };
        }
        here
    }

    /// The most a kind could like a place once `in_sun` has asked the sun: the verge's as
    /// much as in full sun, so a place it would pass over even lit is passed over without the
    /// sun's test, each a line through every crown's shadow lumps (0.2 to 1 s of the floor's
    /// laying when every try asked it, instances j10). The same places are kept, by the same
    /// dice: the test only decides, it rolls none.
    pub(super) fn likes_at_most(&self, kind: Under, here: &mut Here) -> f32 {
        if kind != Under::Verge || self.sun.is_none() {
            return kind.likes(here);
        }
        let sun = std::mem::replace(&mut here.sun, 1.0);
        let most = kind.likes(here);
        here.sun = sun;
        most
    }

    /// Whether a clump of a kind is started where it is tried, by the dice's `roll` against
    /// how much it likes the place times its drifts.
    pub(super) fn starts(
        &self,
        kind: Under,
        at: Vec2,
        mut here: Here,
        roll: f32,
        drifts: f32,
    ) -> bool {
        if roll > self.likes_at_most(kind, &mut here) * drifts {
            return false;
        }
        roll <= kind.likes(&self.in_sun(kind, at, here)) * drifts
    }

    /// What a place is for a clump's member, if it is clear of the bark, what lies and the
    /// path by `clear` (`Under::clear_of`) and liked enough to stand there.
    pub(super) fn member(
        &self,
        kind: Under,
        p: Vec2,
        mut here: Here,
        clear: (f32, f32, f32),
    ) -> Option<Here> {
        let (bark_m, lying_m, path_m) = clear;
        if here.bark < bark_m
            || here.lying < lying_m
            || here.path < path_m
            || self.likes_at_most(kind, &mut here) < 0.1
        {
            return None;
        }
        let here = self.in_sun(kind, p, here);
        (kind.likes(&here) >= 0.1).then_some(here)
    }

    /// A place on the ground, `at` east and south of the start, turned at random about its
    /// up, sunk `sink`, `scale` times its size.
    pub(super) fn on(&self, at: Vec2, dice: &mut Rng, sink: f32, scale: f32) -> Transform {
        let ground = self
            .planet
            .ground_at(self.start + self.east * at.x + self.south * at.y);
        let up = ground.normalize();
        let turn = Quat::from_axis_angle(up, dice.range(0.0, TAU));
        Transform {
            translation: ground - up * sink,
            rotation: turn * Quat::from_rotation_arc(Vec3::Y, up),
            scale: Vec3::splat(scale),
        }
    }

    /// Whether a place is on ground dug or filled before the floor was laid (a dungeon's
    /// digs): nothing is laid there, as nothing grown stays where the ground is dug
    /// (`terra_world::flora`'s `follow_digs`).
    pub(super) fn dug(&self, place: &Transform) -> bool {
        self.planet.dug(place.translation)
    }

    /// Whether nothing is laid at a place in an area: on dug ground (`dug`), or where the area
    /// keeps among trees, under a lake's or a pool's water.
    pub(super) fn off(&self, area: &Area, place: &Transform) -> bool {
        let ground = place.translation;
        self.dug(place)
            || (area.among_trees && ground.length() < self.planet.water_near_m(ground) + 0.05)
    }

    /// Where a direction in the world points on the map.
    pub(super) fn on_map(&self, direction: Vec3) -> Vec2 {
        Vec2::new(direction.dot(self.east), direction.dot(self.south)).normalize_or(Vec2::X)
    }

    pub(super) fn cut_as(&self, name: &str, size: f32) -> Option<Cuttable> {
        self.flora.cut_rule(name).map(|rule| rule.at(size))
    }

    /// A rock's scan, its material under moss, `moss` of it.
    pub(super) fn mossy_scan(&mut self, file: &str, moss: f32) -> (Handle<Mesh>, Handle<Mossy>) {
        self.snowed_scan(file, moss, None)
    }

    /// A plant's scan, its colour times `tint`, under the season: it withers and takes snow
    /// as the ground beside it does (`mossy.wesl`), with no moss or lichen on it.
    pub(super) fn plant_scan(&mut self, file: &str, tint: Color) -> (Handle<Mesh>, Handle<Mossy>) {
        self.snowed_scan(file, 0.0, Some(tint))
    }

    /// A scan drawn with the mossy material, a rock's, or a plant's when it has a tint.
    pub(super) fn snowed_scan(
        &mut self,
        file: &str,
        moss: f32,
        plant: Option<Color>,
    ) -> (Handle<Mesh>, Handle<Mossy>) {
        let path = format!("models/nature/forest/{file}.glb");
        let material = self.mossy.add(Mossy {
            base: StandardMaterial::default(),
            extension: MossExtension {
                colour: Vec3::from_array(
                    Color::srgb(0.2, 0.25, 0.1)
                        .to_linear()
                        .to_f32_array_no_alpha(),
                )
                .extend(moss),
                ground_color: None,
                ground_normal: None,
                snow: Vec4::new(0.0, plant.is_some() as u32 as f32, 0.0, 0.0),
            },
        });
        self.mossy_scans.0.push((
            self.assets.load(
                GltfAssetLabel::Material {
                    index: 0,
                    is_scale_inverted: false,
                }
                .from_asset(path.clone()),
            ),
            material.clone(),
            plant.unwrap_or(Color::WHITE),
        ));
        (
            self.assets.load(
                GltfAssetLabel::Primitive {
                    mesh: 0,
                    primitive: 0,
                }
                .from_asset(path),
            ),
            material,
        )
    }

    pub(super) fn end_grain(&mut self) -> Handle<StandardMaterial> {
        end_grain(self.assets, self.plain)
    }

    /// The material a shadow stand-in is drawn in, cast in full to `SHADOW_SHRINK` of `end`
    /// and shrinking away by `end` (D234): the leaves' own picture, cut to their outline, or
    /// for wood plain and solid.
    pub(super) fn shrinking_material(
        &mut self,
        leaves: Option<&Handle<Foliage>>,
        end: f32,
    ) -> Handle<Foliage> {
        let key = (leaves.map(Handle::id), (end * 4.0) as u32);
        if let Some(material) = self.shrinking.get(&key) {
            return material.clone();
        }
        let base = leaves
            .and_then(|handle| self.foliage.get(handle))
            .map(|material| material.base.clone())
            .unwrap_or_default();
        let wind = self
            .foliage
            .get(&self.atlas)
            .expect("the floor's cards are made before it is laid")
            .extension
            .clone();
        let material = self.foliage.add(Foliage {
            base,
            extension: FoliageExtension {
                shrink: Vec4::new(end * SHADOW_SHRINK, end, 0.0, 0.0),
                leaf: Vec4::ZERO,
                ..wind
            },
        });
        self.shrinking.insert(key, material.clone());
        material
    }

    /// Spawns one plant of a kind, standing at `at` on the map: its first part drawn by its
    /// own entity, the rest by its children, and its shadow filed for when the camera is
    /// near (`shade`). Gives back the plant and whether it can be cut, if it has parts.
    pub(super) fn plant(
        &mut self,
        placement: &Placement,
        drawn: &Drawn,
        at: Vec2,
        place: Transform,
    ) -> Option<(Entity, bool)> {
        let cut = match placement.kind {
            Under::Hazel | Under::Scrub | Under::Sapling | Under::Coppice | Under::Holly => {
                self.cut_as("Bush", place.scale.y)
            }
            Under::Bilberry => self.cut_as("Berry bush", place.scale.y),
            _ => None,
        };
        let (seen, near) = (placement.seen_m, placement.near_m);
        let band = |band: Band| match band {
            Band::All => within(seen),
            Band::Near => within(near),
            Band::Far => VisibilityRange {
                start_margin: near * (1.0 - FADE)..near,
                end_margin: seen * (1.0 - FADE)..seen,
                use_aabb: false,
            },
        };
        let drawn_as = |entity: &mut EntityCommands, part: &Part| {
            let range = band(part.band);
            entity.insert((
                Mesh3d(part.mesh.clone()),
                reach::Laid(range.clone()),
                range,
                NotShadowCaster,
                Undergrowth,
            ));
            match &part.look {
                Look::Foliage(material) => entity.insert(MeshMaterial3d(material.clone())),
                Look::Scan(material) => entity.insert(MeshMaterial3d(material.clone())),
            };
            if part.solid {
                entity.insert(Footing);
            }
        };
        let (first, rest) = drawn.parts.split_first()?;
        // It fades as a whole while it is between the camera and the player (D274).
        let mut entity = self.commands.spawn((
            Name::new(placement.name),
            place,
            terra_world::parting::ObstructionFade::default(),
        ));
        drawn_as(&mut entity, first);
        if cut.is_some() {
            entity.insert(Growth::new(cut, None, 0.5));
        }
        if !rest.is_empty() {
            entity.with_children(|plant| {
                for part in rest {
                    drawn_as(&mut plant.spawn_empty(), part);
                }
            });
        }
        // Its shadow from a copy only the sun sees, which shrinks away rather than ending,
        // as Bevy's shadow passes do not dither (D234), so the plant itself never dithers
        // between two of itself.
        let plant = entity.id();
        if let Some(set) = drawn.shadow_set {
            self.shade.add(plant, at, set);
        }
        Some((plant, cut.is_some()))
    }
}
