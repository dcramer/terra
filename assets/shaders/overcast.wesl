// The overcast sky's brightness seen at a height, as a share of the sky's light it sheds on
// open ground (skylight.rs `overcast_seen`, which a test holds this to): the CIE's standard
// overcast sky, three times brighter overhead than at the horizon (Moon and Spencer 1942,
// L = Lz (1 + 2 sin e) / 3), scaled as the painted sky the ground is lit by is. The 2D deck
// is drawn so, and the haze under a deck takes the horizon's (D418).

// The zenith's share: the even clear sky's mean over the overcast law's, 7 / 9, times the
// even clear sky's cosine-weighted mean, 1.15 - 0.35 * 2 / 3.
const OVERCAST_ZENITH: f32 = 1.178571;

fn overcast_seen(sine: f32) -> f32 {
    return OVERCAST_ZENITH * (1.0 + 2.0 * clamp(sine, 0.0, 1.0)) / 3.0;
}
