//! Scenes: bases built whole from shortcut operations, for the game to put up, the kit
//! sheet to lay and the tests and the drawing checker to hold to their rules. None is a
//! system of its own; each is a fixed set of operations in the kinds it is given, Bevy-free
//! so tests and tools build it headless.
//!
//! - [`example`]: the example cabin, the bench village and roofs over plans;
//! - [`sheet`]: the kit sheet's rows and a material's preview;
//! - [`qa_cabin`]: the QA cabin each material is filmed and checked on;
//! - [`castle`] and [`burg`]: a castle of some 140,000 cubes, and one built to burn.

pub mod burg;
pub mod castle;
pub(crate) mod example;
pub mod qa_cabin;
pub mod sheet;
