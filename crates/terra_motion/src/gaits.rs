//! A man's walk, jog and sprint (D585), the one place the player's (`terra_walker::movement`,
//! `assets/data/movers.ron`'s man, tested equal) and the townsfolk's (`terra_town`) speeds come
//! from. Each is its clip's pace on the base body (`tools/characters/pace.py`, the paces in
//! `base.glb`, which the walker's and folk's tests hold these to): the walk and the town's jog
//! as made; the player's jog and sprint faster than their clips were made, as fast as D585
//! shows them (played up to 1.1 times as fast, the stride warped up to 1.1), until each is
//! made at the game's speed.
//!
//! The clips play at their takes' own frame rate since clip-fps j1: made_clips.py had keyed
//! Kimodo's 30 fps strides one to one on Blender's 24 fps scene, so every pace read 0.8 of the
//! take's and these were a fifth too slow (jog 3.7, sprint 4.6, walk 1.10).

/// Walk_Loop's pace, m/s.
pub const WALK: f32 = 1.375;
/// Jog_Fwd_Loop's pace, m/s, the town's jog: Uthana's locomotion take, asked for 4.3 m/s
/// (2026-10-30). Uthana's locomotion makes no faster clean jog on our body: asked for 5.67, its
/// hips went 4.27 m/s and its feet skated 0.6-1.2 m/s in contact (jog-arms j3).
pub const JOG_PACE: f32 = 3.442;
/// The player's jog, m/s: Jog_Fwd_Loop played 1.1 times as fast, its stride warped 1.06 (D585),
/// short of the old Kimodo jog's 4.535, which no clean Uthana take reaches.
pub const JOG: f32 = 4.0;
/// Sprint_Loop (pace 5.10 m/s) at the most D585 shows it: 1.1 times as fast, stride x1.07.
pub const SPRINT: f32 = 6.0;
/// The speed over which a body is sprinting, halfway from the jog to the sprint: what its
/// figure shows and what tires it change there.
pub const SPRINT_FROM: f32 = (JOG + SPRINT) / 2.0;
