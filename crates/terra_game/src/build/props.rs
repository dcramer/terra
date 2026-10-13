//! Doors and windows (construction-001, rule 7 and gap 15; D192): props a base holds in a
//! hole of built cubes, fitted by `terra_build`'s operations. None is fitted or drawn for
//! now (the developers, 2026-10-13: the models were not good enough), so openings stand as
//! framed holes; the kinds and their prices stay, for when better props come.

use terra_build::PropKind;

/// The kinds of prop, as the operations number them.
pub const DOOR: PropKind = PropKind(0);
pub const WINDOW: PropKind = PropKind(1);

/// The name of a kind of prop.
pub fn name(kind: PropKind) -> &'static str {
    if kind == WINDOW { "window" } else { "door" }
}
