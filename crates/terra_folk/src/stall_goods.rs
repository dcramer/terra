//! The goods on a town's market stalls (D352): what each stall sells and where each good
//! goes is `terra_town::town::market`'s, from `assets/data/market.ron`; here each is
//! placed as the town's catalogue items are, a counter's on the stall's counter top (its
//! model's `counter` socket), the rest on the ground beside it.

use bevy::prelude::*;
use terra_bases::Site;
use terra_survival::catalogue::Builds;
use terra_survival::props::{self, Models, Theirs};
use terra_town::town::{Market, ThingKind, Town};
use terra_world::planet::Planet;

/// The catalogue item a stall is drawn as, whose counter goods are laid on.
const STALL: &str = "MarketStall";

/// Places every good of the town's market. `ground` gives a point of the town's plan on the
/// planet's ground; `site` a point of the plan and a facing as the site's point and turn in
/// degrees that `props::place` takes. How many it placed.
#[allow(clippy::too_many_arguments)]
pub fn lay(
    commands: &mut Commands,
    models: &Models,
    builds: &Builds,
    site: &Site,
    planet: &Planet,
    town: &Town,
    seed: u32,
    ground: &dyn Fn(Vec2) -> Vec3,
    on_site: &dyn Fn(Vec2, f32) -> (Vec3, f32),
) -> usize {
    let market = std::fs::read_to_string(terra_world::data::path("market.ron"))
        .map_err(|why| why.to_string())
        .and_then(|text| Market::parse(&text));
    let market = match market {
        Ok(market) => market,
        Err(why) => {
            error!("Town: {why}");
            return 0;
        }
    };
    let Some(counter) = builds
        .item_from(STALL)
        .and_then(|stall| models.of(&stall)?.socket("counter"))
    else {
        warn!("Town: the market stall's model has no counter socket, so its goods are left off.");
        return 0;
    };
    let size = |file: &str| builds.item_from(file).map(|item| item.dims.xz());
    let goods = market.goods(town, seed, counter.xz(), &size);
    let mut placed = 0;
    for good in &goods {
        let Some(item) = builds.item_from(&good.item) else {
            continue;
        };
        // On the counter, its top's height over the stall's own foot, the ground under the
        // good being a little higher or lower than under the stall's middle.
        let lift = if good.on_counter {
            let stall = ground(town.things[good.stall].at);
            counter.y + (stall - ground(good.at)).dot(stall.normalize())
        } else {
            0.0
        };
        let (point, turn) = on_site(good.at, good.facing);
        if let Some(entity) = props::place(
            commands, models, site, planet, &item, point, lift, turn, false,
        ) {
            commands.entity(entity).insert(Theirs);
            placed += 1;
        }
    }
    // A lantern hung from some stalls' awnings, its foot that far below where it hangs.
    let lantern = builds.item_from(&market.lantern.0);
    let hook = builds
        .item_from(STALL)
        .and_then(|stall| models.of(&stall)?.socket("lantern"));
    if let (Some(lantern), Some(hook)) = (lantern, hook) {
        for index in market.lanterns(town, seed) {
            let stall = &town.things[index];
            let (point, turn) = on_site(stall.place(hook.xz()), stall.facing);
            let lift = hook.y - lantern.dims.y
                + (ground(stall.at) - ground(stall.place(hook.xz())))
                    .dot(ground(stall.at).normalize());
            if let Some(entity) = props::place(
                commands, models, site, planet, &lantern, point, lift, turn, false,
            ) {
                commands.entity(entity).insert(Theirs);
                placed += 1;
            }
        }
    }
    let stalls = town
        .things
        .iter()
        .filter(|t| t.kind == ThingKind::Stall)
        .count();
    info!("Town: {placed} goods and lanterns on and by its {stalls} stalls.");
    placed
}
