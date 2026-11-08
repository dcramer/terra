//! Every word the layers speak across the network, by its first byte (D644): a host's
//! `asked` and a link's `events` are one stream each, and each layer takes the words whose
//! first byte is its own and leaves the rest. Two layers with one byte would each take the
//! other's words and make nothing of them, silently; this is the one place that sees every
//! layer, so the one place that can hold them apart.

#[cfg(test)]
mod tests {
    /// Every first byte in use, with whose it is. A new word's byte is added here.
    #[test]
    fn no_two_layers_words_begin_with_the_same_byte() {
        let words: [(u8, &str); 14] = [
            (terra_walker::net::ground::GROUND, "the ground's operations"),
            (terra_walker::net::worn::WEAR, "what a joined player wears"),
            (terra_walker::net::looks::LOOK, "how a joined player looks"),
            (terra_walker::net::clock::CLOCK, "the host's clock"),
            (terra_walker::net::woods::FELLED, "a felled tree"),
            (crate::play::portal::shared::GATE, "the gate"),
            (crate::play::save::guest::CHARACTER, "a guest's character"),
            (terra_creatures::net::STRIKE, "a joined player's blow"),
            (terra_creatures::net::HURT, "a creature's blow on a guest"),
            (crate::play::marks::MARKED, "a mark earned"),
            (
                terra_survival::hands_net::HANDS,
                "what a joined player holds",
            ),
            (crate::play::base_net::BASE, "the base"),
            (crate::play::props_net::PROP, "a prop"),
            (terra_survival::storage_net::STORED, "what a storage holds"),
        ];
        for (at, (byte, whose)) in words.iter().enumerate() {
            for (other, theirs) in &words[..at] {
                assert_ne!(byte, other, "{whose} and {theirs} both begin with {byte}");
            }
        }
    }
}
