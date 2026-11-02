//! A man's walk, jog and sprint (D585), the one place the player's (`terra_walker::movement`,
//! `assets/data/movers.ron`'s man, tested equal) and the townsfolk's (`terra_town`) speeds come
//! from. The walk and the townsfolk's jog are their clips' paces on the base body
//! (`tools/characters/pace.py`, the paces in `base.glb`, which the folk's tests hold to a
//! hundredth); the player's jog and sprint are within a tenth of theirs (the walker's tests),
//! the sprint as fast as its clip may be shown until a sprint is made at a sprinter's speed.
//!
//! The clips play at their takes' own frame rate since clip-fps j1: made_clips.py had keyed
//! Kimodo's 30 fps strides one to one on Blender's 24 fps scene, so every pace read 0.8 of the
//! take's and these were a fifth too slow (jog 3.7, sprint 4.6, walk 1.10). D593's longer legs
//! lengthened every stride at the same cadence (walk 1.375 to 1.44, jog 4.535 to 4.654, sprint
//! 5.10 to 5.365): a body re-made re-measures its paces, and the clips' paces here are set to
//! them.

/// Walk_Loop's pace, m/s: the townsfolk's walk.
pub const WALK: f32 = 1.44;
/// Jog_Fwd_Loop's pace, m/s: the townsfolk's jog.
pub const JOG_LOOP: f32 = 4.654;
/// The player's jog, m/s: Jog_Fwd_Loop's pace before D593, its clip now shown at 0.97, within
/// D585's tenth. It moves to `JOG_LOOP` once the controller's stairs hold a descent at that
/// speed: at 4.654 `stair.down` went from 0.001 to 0.091 of the jog (bar 0.03) and `wade.step`
/// from 0.766 to 0.839 m/s.
pub const JOG: f32 = 4.535;
/// Sprint_Loop (pace 5.365 m/s) shown at 6.0, within D585: 1.1 times as fast, stride x1.017.
pub const SPRINT: f32 = 6.0;
/// The speed over which a body is sprinting, halfway from the jog to the sprint: what its
/// figure shows and what tires it change there.
pub const SPRINT_FROM: f32 = (JOG + SPRINT) / 2.0;
