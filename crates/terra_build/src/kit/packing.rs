//! How a packed kind's surface is shaped (D236): earth's lumps and lifts, rock's facets.

/// How a block kind's earth was packed, in its surface's shape (D236): lumps where it was
/// pressed by hand, and each lift rounded out between the seams where the next was rammed
/// on it, the seams following its masks' `lift` and wandering as its strata do.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Packing {
    /// How far its lumps stand out or sink, either way, in metres, and how far across one
    /// is.
    pub lumps: f32,
    pub lump: f32,
    /// How far each lift's middle stands out from the seams either side, in metres.
    pub ridge: f32,
    /// How far a whole lift stands out or back from the next, either way, in metres: each
    /// rammed a little proud or shy of the one under it, so a wall's arrises and its outline
    /// against the sky step from lift to lift.
    #[serde(default)]
    pub step: f32,
    /// How far its broken faces stand out or sink, either way, in metres, and how far
    /// across one is: hard rock split along its joints in flat facets meeting at creases,
    /// not pressed in lumps (the mine's walls, round 8's judges: "smooth plaster").
    #[serde(default)]
    pub facets: f32,
    #[serde(default)]
    pub facet: f32,
}
