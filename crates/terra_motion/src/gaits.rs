//! A man's walk, jog and sprint (D585), the one place the player's (`terra_walker::movement`,
//! `assets/data/movers.ron`'s man, tested equal) and the townsfolk's (`terra_town`) speeds come
//! from. Each is its clip's pace on the base body (`tools/characters/pace.py`, the paces in
//! `base.glb`, which the walker's and folk's tests hold these to): the walk and jog as made,
//! the sprint as fast as its clip may be shown (played 1.1 times as fast, its stride warped
//! 1.07), until a sprint is made at a sprinter's speed.
//!
//! The clips play at their takes' own frame rate since clip-fps j1: made_clips.py had keyed
//! Kimodo's 30 fps strides one to one on Blender's 24 fps scene, so every pace read 0.8 of the
//! take's and these were a fifth too slow (jog 3.7, sprint 4.6, walk 1.10).

/// Walk_Loop's pace, m/s.
pub const WALK: f32 = 1.375;
/// Jog_Fwd_Loop's pace, m/s.
pub const JOG: f32 = 4.535;
/// Sprint_Loop (pace 5.10 m/s) at the most D585 shows it: 1.1 times as fast, stride x1.07.
pub const SPRINT: f32 = 6.0;
/// The speed over which a body is sprinting, halfway from the jog to the sprint: what its
/// figure shows and what tires it change there.
pub const SPRINT_FROM: f32 = (JOG + SPRINT) / 2.0;
