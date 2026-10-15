// The places the demos make (docs/demos.md, D326): each named once here with the sandbox
// options that put the camera there, its fixed views, the look it is held to, and its route,
// so `vantage.ts`, `film.ts` and the others take `--place <name>` rather than each keeping its
// own copy for one forest. Views are a name and where the camera stands and what it looks at,
// in metres east, up and south of the start (the sandbox's --vantage); with `ground` their
// heights are over the ground under each point (--vantage-ground). Node 22.18 or later; no
// dependencies.

export type Place = {
  /// What it is, for the tools' output.
  about: string;
  /// The sandbox's options that make it.
  args: string[];
  /// Its fixed views.
  views: [string, string][];
  /// The views held to its look's reference, and the reference (`looks/<name>.json`).
  measured: string[];
  look: string;
  /// Heights over the ground under each view's points, not over the start.
  ground: boolean;
  /// Whether the canopy and undergrowth are measured, as in a forest.
  forest: boolean;
  /// The film's route: spots to walk to in turn, metres east and south of the start, and
  /// where the walker starts and first faces.
  route?: { at: string; face: string; through: string };
};

export const PLACES: Record<string, Place> = {
  lush: {
    about: "the lush forest: a tall conifer stand with oaks, on the yard's rolling ground (D301)",
    args: ["--experiment", "walk", "--experiment", "lush", "--level", "yard"],
    views: [
      ["floor", "-56,1.7,-43,-72,1.0,-55"],
      ["edge", "0,1.8,250,0,5,200"],
      ["clearing", "7,1.8,9,-8,1.2,-7"],
      ["overlook", "0,45,150,0,0,0"],
      // Close along a path at eye height, where the floor, undergrowth and trunks are judged.
      ["path", "-54,1.6,-41,-60,0.5,-48"],
      // Into a spruce grove.
      ["grove", "-21.9,1.7,-100,-21.9,4,-114"],
    ],
    measured: ["floor", "clearing", "path", "grove"],
    look: "sotf-forest-day",
    ground: false,
    forest: true,
  },
  lake: {
    about:
      "the mountain lake (docs/demos.md §1): the 30 km world's mountains, the lake its query finds (--site lake), in early June with the peaks' snow",
    args: ["--experiment", "play", "--radius", "30000", "--site", "lake", "--date", "06-10"],
    // The start is the lake's shore nearest its deepest water, facing it: -south is over the
    // water, east to the right.
    views: [
      // The shore at eye height, across the water to the far slope.
      ["shore", "0,1.7,0,0,30,-900"],
      // Up on the shore's slope, across the lake to the forest, the tree line and the peaks.
      ["across", "40,60,120,0,200,-1500"],
      // Along the shore, the forest's edge on the water.
      ["along", "0,1.7,0,-600,10,-80"],
      // The far slopes and the peaks over them.
      ["peaks", "0,1.7,0,300,500,-2500"],
      // The valley from high on its side, the lake below.
      ["valley", "-500,350,600,0,0,-300"],
    ],
    measured: ["shore", "across", "along", "peaks", "valley"],
    look: "mountain-lake-day",
    ground: true,
    forest: false,
    // Up the east shore, on the edge of the water and the forest with the peaks ahead across the
    // lake, as the developers asked (2026-10-15); spots of the shore walk `cargo xtask fields
    // --site lake` prints.
    route: { at: "216,22", face: "248,-3", through: "248,-3;275,-35;302,-67;340,-84;378,-100;410,-124;431,-165;445,-213" },
  },
  tundra: {
    about:
      "the tundra (docs/demos.md §2): the fell plateau its query finds 10.5 km north of the start (--site tundra), in early October, the heath's autumn colours and the first snow lying in the lees",
    args: ["--experiment", "play", "--radius", "30000", "--site", "tundra", "--date", "10-08"],
    // The start faces the closed forest's edge, 1.9 km off: -south is toward it, the open
    // plateau and the fells with their snow behind.
    views: [
      // The heath at eye height, its communities and the polygons' pools ahead.
      ["heath", "0,1.7,0,0,0.8,-25"],
      // Out over the plateau to the forest's edge and its last spruce.
      ["edge", "0,1.7,0,0,25,-1900"],
      // The other way, the open plateau to the fells, snow in their lees.
      ["open", "0,1.7,0,0,2,600"],
      // Down over the patterned ground and its pools from a rise's height.
      ["pools", "0,8,0,40,0,-40"],
      // The plateau from 40 m up.
      ["overview", "0,40,60,0,0,-200"],
    ],
    measured: ["heath", "edge", "open", "overview"],
    look: "tundra-autumn-day",
    ground: true,
    forest: false,
    // Across the heath into the prevailing west wind and back with it.
    route: { at: "0,0", face: "40,0", through: "40,4;80,-6;120,4;160,-8;200,0;170,20;120,26;70,18;20,10" },
  },
  town: {
    about:
      "the market town (docs/demos.md §3): where the land's rules make room for one on the 30 km world (--site town), a valley floor at the foot of the range, the forest past its fields, on market morning with its people and animals",
    args: ["--experiment", "play", "--experiment", "town", "--radius", "30000", "--site", "town", "--mute"],
    // The views the town logs as `Town vantage` (`play::town`), in the start's terms: the start
    // stands 330 m beside the town, out by its fields.
    views: [
      ["street", "63.00,0.22,-374.58,40.70,0.96,-354.76"],
      ["square", "20.94,1.11,-347.32,-11.74,0.90,-342.65"],
      ["market", "-14.89,0.99,-346.09,3.92,0.76,-345.86"],
      ["forge", "25.68,0.88,-351.56,29.82,0.01,-346.28"],
      ["yard", "5.63,0.35,-382.04,10.47,-0.47,-375.88"],
      ["above", "43.63,27.33,-323.67,0.00,-0.61,-345.04"],
    ],
    measured: ["street", "square", "market", "forge", "yard"],
    look: "town-market-day",
    ground: false,
    forest: false,
    // In along the main street from halfway out, across the square past the market and up the
    // other main street a way (the town's log gives it as `Town route`).
    route: {
      at: "63.8,-373.7",
      face: "0.0,-345.1",
      through: "54.2,-366.5;45.5,-358.3;35.2,-352.3;23.5,-349.9;-6.6,-341.6;-29.3,-339.0;-40.7,-335.4;-51.9,-331.0",
    },
  },
};

/// The place a tool's `--place` names, `lush` by default.
export function place(name: string | undefined): Place {
  const found = PLACES[name ?? "lush"];
  if (!found) throw new Error(`no place called ${name}: ${Object.keys(PLACES).join(", ")}`);
  return found;
}
