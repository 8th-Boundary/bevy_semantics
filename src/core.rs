//! Compile-time identities for the ontology kinds seeded by every registry.

use crate::Kind;

/// The core `Relation` classification kind.
pub const RELATION: Kind = crate::kind!("Relation");
/// The reserved core `Namespace` kind.
pub const NAMESPACE: Kind = crate::kind!("Namespace");
/// The core `is_a` taxonomy relation.
pub const IS_A: Kind = crate::kind!("is_a");
/// The core `not_a` exclusion relation.
pub const NOT_A: Kind = crate::kind!("not_a");
/// The core `can_be` capability relation.
pub const CAN_BE: Kind = crate::kind!("can_be");
/// The core `cant_be` negative capability relation.
pub const CANT_BE: Kind = crate::kind!("cant_be");
/// The core `has_part` composition relation.
pub const HAS_PART: Kind = crate::kind!("has_part");
/// The core `part_of` composition relation.
pub const PART_OF: Kind = crate::kind!("part_of");
/// The core `inverse_of` relation.
pub const INVERSE_OF: Kind = crate::kind!("inverse_of");
/// The core `negates` relation.
pub const NEGATES: Kind = crate::kind!("negates");
