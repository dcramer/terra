//! The tasks that only start a tool kept elsewhere (a TypeScript tool, a crate's program, a
//! Blender or shell script), each area's in its own module with its own `TASKS` (D410).

pub(crate) mod art;
pub(crate) mod docs;
pub(crate) mod play;
pub(crate) mod sandbox;
pub(crate) mod world;
