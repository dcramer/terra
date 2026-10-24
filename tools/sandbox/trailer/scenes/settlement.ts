// The settlement rising (D381): the time-lapse of a market town going up in its clearing by
// the lake, from its first house into dusk, as the settlement's own film shows it
// (timelapse/settlement-dusk.json, whose camera keys the run file carries). No remote calls:
// the time-lapse builds and keeps the clock, the keys move the camera. The pinned settlement
// site replaces the town place's (the sandbox keeps the last --site it is given).

export default {
  about: "a settlement rising from its clearing into a market town, on into dusk",
  sandbox: ["--site", "settlement@2.41,2.51", "--quality", "ultra", "--timelapse", "settlement-dusk.ron@120"],
  rolls: 120,
  script: () => {},
};
