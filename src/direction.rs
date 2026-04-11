//! Edge traversal direction controls.

/// Direction for neighborhood and traversal queries.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum EdgeDirection {
    #[default]
    Outgoing,
    Incoming,
    Both,
}
