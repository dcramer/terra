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
    args: ["--experiment", "walk", "--radius", "30000", "--site", "lake", "--date", "06-10"],
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
    // Round the shore to the right, the first spots of the walk `cargo xtask fields --site
    // lake` prints; the bench's lake-walk follows the same.
    route: { at: "0,0", face: "0,-200", through: "84,32;123,16;173,16;216,22;248,-3;275,-35;302,-67;340,-84;378,-100" },
  },
};

/// The place a tool's `--place` names, `lush` by default.
export function place(name: string | undefined): Place {
  const found = PLACES[name ?? "lush"];
  if (!found) throw new Error(`no place called ${name}: ${Object.keys(PLACES).join(", ")}`);
  return found;
}
