//! Mutable semantic registry and bulk authoring support.

use std::any::{type_name, TypeId};
use std::sync::Arc;
use std::sync::RwLock;

use bevy_platform::collections::HashMap;

use crate::error::SemanticError;
use crate::kind::Kind;
use crate::query::SemanticCommand;
use crate::snapshot::SemanticSnapshot;
use crate::weight::Weight;

type SnapshotCache = Arc<RwLock<Option<(u64, Arc<SemanticSnapshot>)>>>;

/// Canonical built-in semantic kinds and relations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Builtins {
    pub relation: Kind,
    pub namespace: Kind,
    pub is_a: Kind,
    pub not_a: Kind,
    pub can_be: Kind,
    pub cant_be: Kind,
    pub has_part: Kind,
    pub part_of: Kind,
    pub inverse_of: Kind,
    pub negates: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KindFlags(u8);

impl KindFlags {
    const BUILT_IN: Self = Self(1 << 0);
    const TYPED: Self = Self(1 << 1);
    const USER_DEFINED: Self = Self(1 << 2);

    fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct KindMeta {
    pub(crate) kind: Kind,
    pub(crate) name: Arc<str>,
    pub(crate) type_id: Option<TypeId>,
    pub(crate) flags: KindFlags,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct GraphState {
    pub(crate) edges: HashMap<(Kind, Kind, Kind), Option<Weight>>,
}

/// Mutable source of truth for kinds and edge data.
#[derive(Clone, Debug)]
pub struct SemanticRegistry {
    pub(crate) version: u64,
    pub(crate) kinds: Vec<KindMeta>,
    pub(crate) kind_to_index: HashMap<Kind, usize>,
    pub(crate) name_to_kind: HashMap<Arc<str>, Kind>,
    pub(crate) type_to_kind: HashMap<TypeId, Kind>,
    pub(crate) kind_to_type: HashMap<Kind, TypeId>,
    pub(crate) builtins: Option<Builtins>,
    pub(crate) graph: GraphState,
    snapshot_cache: SnapshotCache,
    bulk_depth: usize,
    bulk_dirty: bool,
}

impl Default for SemanticRegistry {
    fn default() -> Self {
        let mut registry = Self {
            version: 0,
            kinds: Vec::new(),
            kind_to_index: HashMap::new(),
            name_to_kind: HashMap::new(),
            type_to_kind: HashMap::new(),
            kind_to_type: HashMap::new(),
            builtins: None,
            graph: GraphState::default(),
            snapshot_cache: Arc::new(RwLock::new(None)),
            bulk_depth: 0,
            bulk_dirty: false,
        };
        registry.begin_bulk_edit();
        let _ = registry
            .ensure_builtins()
            .expect("built-ins are static and should always seed");
        registry.end_bulk_edit();
        registry
    }
}

impl SemanticRegistry {
    /// Return the current registry version.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Register a new canonical kind name or return the existing `Kind`.
    pub fn register_kind(&mut self, name: impl AsRef<str>) -> Result<Kind, SemanticError> {
        let canonical = canonicalize_name(name.as_ref())?;
        self.register_canonical_kind(canonical, None, KindFlags::USER_DEFINED)
    }

    /// Register the Rust type `T` under its default canonical name.
    pub fn typed_kind<T>(&mut self) -> Result<Kind, SemanticError>
    where
        T: 'static,
    {
        self.typed_kind_named::<T>(type_name::<T>())
    }

    /// Register the Rust type `T` under an explicit canonical name.
    pub fn typed_kind_named<T>(&mut self, name: impl AsRef<str>) -> Result<Kind, SemanticError>
    where
        T: 'static,
    {
        let canonical = canonicalize_name(name.as_ref())?;
        let type_id = TypeId::of::<T>();

        if let Some(existing_kind) = self.type_to_kind.get(&type_id).copied() {
            let existing_name = self.name(existing_kind).unwrap_or("<unknown>").to_string();
            if existing_name == canonical.as_ref() {
                return Ok(existing_kind);
            }
            return Err(SemanticError::KindTypeConflict {
                kind: existing_kind,
                existing_type_name: existing_name,
                requested_type_name: type_name::<T>().to_string(),
            });
        }

        self.register_canonical_kind(canonical, Some(type_id), KindFlags::TYPED)
    }

    fn register_builtin_kind(&mut self, name: impl AsRef<str>) -> Result<Kind, SemanticError> {
        let canonical = canonicalize_name(name.as_ref())?;
        self.register_canonical_kind(canonical, None, KindFlags::BUILT_IN)
    }

    /// Compatibility alias for [`SemanticRegistry::typed_kind`].
    pub fn register_typed_kind<T>(&mut self) -> Result<Kind, SemanticError>
    where
        T: 'static,
    {
        self.typed_kind::<T>()
    }

    /// Compatibility alias for [`SemanticRegistry::typed_kind_named`].
    pub fn register_typed_kind_named<T>(
        &mut self,
        name: impl AsRef<str>,
    ) -> Result<Kind, SemanticError>
    where
        T: 'static,
    {
        self.typed_kind_named::<T>(name)
    }

    /// Look up a `Kind` by canonical name.
    pub fn kind(&self, name: impl AsRef<str>) -> Option<Kind> {
        let canonical = canonicalize_name(name.as_ref()).ok()?;
        self.name_to_kind.get(canonical.as_ref()).copied()
    }

    /// Look up the `Kind` bound to a Rust type.
    pub fn kind_of<T>(&self) -> Option<Kind>
    where
        T: 'static,
    {
        self.type_to_kind.get(&TypeId::of::<T>()).copied()
    }

    /// Resolve the canonical name for a `Kind`.
    pub fn name(&self, kind: Kind) -> Option<&str> {
        self.kind_to_index
            .get(&kind)
            .and_then(|index| self.kinds.get(*index))
            .map(|meta| meta.name.as_ref())
    }

    /// Resolve the Rust `TypeId` bound to a `Kind`, if any.
    pub fn type_id(&self, kind: Kind) -> Option<TypeId> {
        self.kind_to_type.get(&kind).copied()
    }

    /// Return the canonical built-in kinds and relations.
    pub fn builtins(&self) -> Builtins {
        self.builtins
            .expect("built-ins are seeded during registry initialization")
    }

    /// Ensure the built-in semantic kinds and relations are present and return their handles.
    pub fn ensure_builtins(&mut self) -> Result<Builtins, SemanticError> {
        let relation = self.register_builtin_kind("Relation")?;
        let namespace = self.register_builtin_kind("Namespace")?;
        let is_a = self.register_builtin_kind("is_a")?;
        let not_a = self.register_builtin_kind("not_a")?;
        let can_be = self.register_builtin_kind("can_be")?;
        let cant_be = self.register_builtin_kind("cant_be")?;
        let has_part = self.register_builtin_kind("has_part")?;
        let part_of = self.register_builtin_kind("part_of")?;
        let inverse_of = self.register_builtin_kind("inverse_of")?;
        let negates = self.register_builtin_kind("negates")?;

        for kind in [
            relation, is_a, not_a, can_be, cant_be, has_part, part_of, inverse_of, negates,
        ] {
            let _ = self.add_edge(kind, is_a, relation)?;
        }

        let _ = self.add_edge(has_part, inverse_of, part_of)?;
        let _ = self.add_edge(part_of, inverse_of, has_part)?;
        let _ = self.add_edge(is_a, negates, not_a)?;
        let _ = self.add_edge(not_a, negates, is_a)?;
        let _ = self.add_edge(can_be, negates, cant_be)?;
        let _ = self.add_edge(cant_be, negates, can_be)?;

        let builtins = Builtins {
            relation,
            namespace,
            is_a,
            not_a,
            can_be,
            cant_be,
            has_part,
            part_of,
            inverse_of,
            negates,
        };
        self.builtins = Some(builtins);
        Ok(builtins)
    }

    /// Build a fresh immutable snapshot.
    pub fn snapshot(&self) -> SemanticSnapshot {
        SemanticSnapshot::from_registry(self)
    }

    /// Return a cached immutable snapshot when possible.
    #[doc(alias = "snapshot_arc")]
    pub fn snapshot_cached(&self) -> Arc<SemanticSnapshot> {
        if let Some(cached) = self
            .snapshot_cache
            .read()
            .expect("snapshot cache poisoned")
            .as_ref()
            .and_then(|(version, snapshot)| {
                if *version == self.version {
                    Some(Arc::clone(snapshot))
                } else {
                    None
                }
            })
        {
            return cached;
        }

        let snapshot = Arc::new(self.snapshot());
        let mut cache = self
            .snapshot_cache
            .write()
            .expect("snapshot cache poisoned");
        if let Some((version, cached)) = cache.as_ref() {
            if *version == self.version {
                return Arc::clone(cached);
            }
        }

        *cache = Some((self.version, Arc::clone(&snapshot)));
        snapshot
    }

    /// Start a bulk-edit session that defers snapshot invalidation until commit.
    pub fn batch(&mut self) -> SemanticRegistryBatch<'_> {
        self.begin_bulk_edit();
        SemanticRegistryBatch {
            registry: self,
            changed: false,
            committed: false,
        }
    }

    /// Insert or update an unweighted semantic edge.
    pub fn add_edge(
        &mut self,
        subject: Kind,
        relation: Kind,
        target: Kind,
    ) -> Result<bool, SemanticError> {
        self.upsert_edge(subject, relation, target, None)
    }

    /// Insert or update a weighted semantic edge.
    #[doc(alias = "add_weighted_edge")]
    pub fn add_edge_weighted(
        &mut self,
        subject: Kind,
        relation: Kind,
        target: Kind,
        weight: Weight,
    ) -> Result<bool, SemanticError> {
        self.upsert_edge(subject, relation, target, Some(weight))
    }

    /// Compatibility alias for [`SemanticRegistry::add_edge_weighted`].
    #[doc(alias = "add_edge_weighted")]
    pub fn add_weighted_edge(
        &mut self,
        subject: Kind,
        relation: Kind,
        target: Kind,
        weight: Weight,
    ) -> Result<bool, SemanticError> {
        self.add_edge_weighted(subject, relation, target, weight)
    }

    /// Unregister a kind and remove any incident edges and type bindings.
    pub fn unregister_kind(&mut self, kind: Kind) -> Result<bool, SemanticError> {
        let index = self
            .kind_to_index
            .get(&kind)
            .copied()
            .ok_or(SemanticError::UnknownKind { kind })?;

        let meta = self.kinds[index].clone();
        if meta.flags.contains(KindFlags::BUILT_IN) {
            return Err(SemanticError::CannotUnregisterBuiltInKind { kind });
        }

        if let Some(type_id) = meta.type_id {
            self.kind_to_type.remove(&kind);
            self.type_to_kind.remove(&type_id);
        }

        self.name_to_kind.remove(meta.name.as_ref());
        self.kind_to_index.remove(&kind);

        let removed_meta = self.kinds.swap_remove(index);
        if let Some(moved_meta) = self.kinds.get(index) {
            self.kind_to_index.insert(moved_meta.kind, index);
        }
        debug_assert_eq!(removed_meta.kind, kind);

        self.graph.edges.retain(|key, _| {
            let (subject, relation, target) = *key;
            subject != kind && relation != kind && target != kind
        });

        self.bump_version();
        Ok(true)
    }

    /// Remove a semantic edge if it exists.
    pub fn remove_edge(
        &mut self,
        subject: Kind,
        relation: Kind,
        target: Kind,
    ) -> Result<bool, SemanticError> {
        self.ensure_known_kind(subject)?;
        self.ensure_known_kind(relation)?;
        self.ensure_known_kind(target)?;

        let key = (subject, relation, target);
        if self.graph.edges.remove(&key).is_some() {
            self.bump_version();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Check whether an exact semantic edge exists.
    pub fn has_edge(&self, subject: Kind, relation: Kind, target: Kind) -> bool {
        self.graph.edges.contains_key(&(subject, relation, target))
    }

    /// Apply a single semantic command immediately.
    pub fn apply_command(&mut self, command: SemanticCommand) -> Result<bool, SemanticError> {
        match command {
            SemanticCommand::RegisterKind { name } => {
                let before = self.version;
                let _ = self.register_kind(name.as_ref())?;
                Ok(self.version != before)
            }
            SemanticCommand::RegisterTypedKind { type_id, name } => {
                let canonical = name.ok_or_else(|| SemanticError::MalformedCommand {
                    command: "RegisterTypedKind",
                    reason: "missing canonical name".to_string(),
                })?;
                let before = self.version;
                self.register_typed_kind_command(type_id, canonical)?;
                Ok(self.version != before)
            }
            SemanticCommand::AddEdge {
                subject,
                relation,
                target,
                weight,
            } => match weight {
                Some(weight) => self.add_edge_weighted(subject, relation, target, weight),
                None => self.add_edge(subject, relation, target),
            },
            SemanticCommand::UnregisterKind { kind } => self.unregister_kind(kind),
            SemanticCommand::RemoveEdge {
                subject,
                relation,
                target,
            } => self.remove_edge(subject, relation, target),
            SemanticCommand::EnsureBuiltins => {
                let before = self.version;
                let _ = self.ensure_builtins()?;
                Ok(self.version != before)
            }
        }
    }

    /// Apply a batch of semantic commands with one bulk invalidation.
    pub fn apply_commands<I>(&mut self, commands: I) -> Result<bool, SemanticError>
    where
        I: IntoIterator<Item = SemanticCommand>,
    {
        self.begin_bulk_edit();
        let result = (|| {
            let mut changed = false;
            for command in commands {
                changed |= self.apply_command(command)?;
            }
            Ok(changed)
        })();
        self.end_bulk_edit();
        result
    }

    fn register_typed_kind_command(
        &mut self,
        type_id: TypeId,
        name: Arc<str>,
    ) -> Result<Kind, SemanticError> {
        let canonical = canonicalize_name(name.as_ref())?;
        if let Some(existing_kind) = self.type_to_kind.get(&type_id).copied() {
            let existing_name = self.name(existing_kind).unwrap_or("<unknown>").to_string();
            if existing_name == canonical.as_ref() {
                return Ok(existing_kind);
            }
            return Err(SemanticError::KindTypeConflict {
                kind: existing_kind,
                existing_type_name: existing_name,
                requested_type_name: name.as_ref().to_string(),
            });
        }

        self.register_canonical_kind(canonical, Some(type_id), KindFlags::TYPED)
    }

    fn register_canonical_kind(
        &mut self,
        canonical: Arc<str>,
        type_id: Option<TypeId>,
        base_flags: KindFlags,
    ) -> Result<Kind, SemanticError> {
        if let Some(existing_kind) = self.name_to_kind.get(canonical.as_ref()).copied() {
            if let Some(requested_type) = type_id {
                if let Some(existing_type) = self.type_to_kind.get(&requested_type).copied() {
                    if existing_type != existing_kind {
                        return Err(SemanticError::KindTypeConflict {
                            kind: existing_kind,
                            existing_type_name: self
                                .name(existing_kind)
                                .unwrap_or("<unknown>")
                                .to_string(),
                            requested_type_name: canonical.as_ref().to_string(),
                        });
                    }
                }
                if let Some(existing_type) = self.kind_to_type.get(&existing_kind).copied() {
                    if existing_type != requested_type {
                        return Err(SemanticError::KindTypeConflict {
                            kind: existing_kind,
                            existing_type_name: self
                                .name(existing_kind)
                                .unwrap_or("<unknown>")
                                .to_string(),
                            requested_type_name: format!("{requested_type:?}"),
                        });
                    }
                } else {
                    self.kind_to_type.insert(existing_kind, requested_type);
                    self.type_to_kind.insert(requested_type, existing_kind);
                    if let Some(index) = self.kind_to_index.get(&existing_kind).copied() {
                        if let Some(meta) = self.kinds.get_mut(index) {
                            meta.type_id = Some(requested_type);
                            meta.flags = meta.flags.union(KindFlags::TYPED);
                        }
                    }
                    self.bump_version();
                }
            }
            if let Some(index) = self.kind_to_index.get(&existing_kind).copied() {
                if let Some(meta) = self.kinds.get_mut(index) {
                    let updated_flags = meta.flags.union(base_flags);
                    if updated_flags != meta.flags {
                        meta.flags = updated_flags;
                        self.bump_version();
                    }
                }
            }
            return Ok(existing_kind);
        }

        let kind = hash_canonical_name(canonical.as_ref());
        if let Some(existing_index) = self.kind_to_index.get(&kind).copied() {
            let existing_name = self
                .kinds
                .get(existing_index)
                .map(|meta| meta.name.as_ref().to_string())
                .unwrap_or_else(|| "<unknown>".to_string());
            return Err(SemanticError::KindHashCollision {
                kind,
                existing_name,
                requested_name: canonical.as_ref().to_string(),
            });
        }

        let index = self.kinds.len();
        self.kinds.push(KindMeta {
            kind,
            name: canonical.clone(),
            type_id,
            flags: base_flags,
        });
        self.kind_to_index.insert(kind, index);
        self.name_to_kind.insert(canonical, kind);
        if let Some(type_id) = type_id {
            self.kind_to_type.insert(kind, type_id);
            self.type_to_kind.insert(type_id, kind);
        }
        self.bump_version();
        Ok(kind)
    }

    fn upsert_edge(
        &mut self,
        subject: Kind,
        relation: Kind,
        target: Kind,
        weight: Option<Weight>,
    ) -> Result<bool, SemanticError> {
        self.ensure_known_kind(subject)?;
        self.ensure_known_kind(relation)?;
        self.ensure_known_kind(target)?;

        let key = (subject, relation, target);
        match self.graph.edges.get_mut(&key) {
            Some(existing) if *existing == weight => Ok(false),
            Some(existing) => {
                *existing = weight;
                self.bump_version();
                Ok(true)
            }
            None => {
                self.graph.edges.insert(key, weight);
                self.bump_version();
                Ok(true)
            }
        }
    }

    fn ensure_known_kind(&self, kind: Kind) -> Result<(), SemanticError> {
        if self.kind_to_index.contains_key(&kind) {
            Ok(())
        } else {
            Err(SemanticError::UnknownKind { kind })
        }
    }

    fn bump_version(&mut self) {
        self.version = self.version.wrapping_add(1);
        if self.bulk_depth == 0 {
            self.invalidate_snapshot_cache();
        } else {
            self.bulk_dirty = true;
        }
    }

    fn invalidate_snapshot_cache(&self) {
        *self
            .snapshot_cache
            .write()
            .expect("snapshot cache poisoned") = None;
    }

    fn begin_bulk_edit(&mut self) {
        self.bulk_depth += 1;
    }

    fn end_bulk_edit(&mut self) {
        if self.bulk_depth == 0 {
            return;
        }

        self.bulk_depth -= 1;
        if self.bulk_depth == 0 && self.bulk_dirty {
            self.bulk_dirty = false;
            self.invalidate_snapshot_cache();
        }
    }
}

/// Bulk registry editing session that defers snapshot invalidation until commit.
#[must_use]
pub struct SemanticRegistryBatch<'a> {
    registry: &'a mut SemanticRegistry,
    changed: bool,
    committed: bool,
}

impl<'a> SemanticRegistryBatch<'a> {
    /// Return whether the batch made any observable change.
    pub fn changed(&self) -> bool {
        self.changed
    }

    fn finish(&mut self) {
        if self.committed {
            return;
        }
        self.registry.end_bulk_edit();
        self.committed = true;
    }

    pub fn register_kind(&mut self, name: impl AsRef<str>) -> Result<Kind, SemanticError> {
        let before = self.registry.version();
        let kind = self.registry.register_kind(name)?;
        self.changed |= self.registry.version() != before;
        Ok(kind)
    }

    pub fn typed_kind<T>(&mut self) -> Result<Kind, SemanticError>
    where
        T: 'static,
    {
        let before = self.registry.version();
        let kind = self.registry.typed_kind::<T>()?;
        self.changed |= self.registry.version() != before;
        Ok(kind)
    }

    pub fn typed_kind_named<T>(&mut self, name: impl AsRef<str>) -> Result<Kind, SemanticError>
    where
        T: 'static,
    {
        let before = self.registry.version();
        let kind = self.registry.typed_kind_named::<T>(name)?;
        self.changed |= self.registry.version() != before;
        Ok(kind)
    }

    pub fn add_edge(
        &mut self,
        subject: Kind,
        relation: Kind,
        target: Kind,
    ) -> Result<bool, SemanticError> {
        let before = self.registry.version();
        let changed = self.registry.add_edge(subject, relation, target)?;
        self.changed |= changed || self.registry.version() != before;
        Ok(changed)
    }

    #[doc(alias = "add_weighted_edge")]
    pub fn add_edge_weighted(
        &mut self,
        subject: Kind,
        relation: Kind,
        target: Kind,
        weight: Weight,
    ) -> Result<bool, SemanticError> {
        let before = self.registry.version();
        let changed = self
            .registry
            .add_edge_weighted(subject, relation, target, weight)?;
        self.changed |= changed || self.registry.version() != before;
        Ok(changed)
    }

    /// Compatibility alias for [`SemanticRegistryBatch::add_edge_weighted`].
    #[doc(alias = "add_edge_weighted")]
    pub fn add_weighted_edge(
        &mut self,
        subject: Kind,
        relation: Kind,
        target: Kind,
        weight: Weight,
    ) -> Result<bool, SemanticError> {
        self.add_edge_weighted(subject, relation, target, weight)
    }

    /// Unregister a kind and clean up all incident edges.
    pub fn unregister_kind(&mut self, kind: Kind) -> Result<bool, SemanticError> {
        let before = self.registry.version();
        let changed = self.registry.unregister_kind(kind)?;
        self.changed |= changed || self.registry.version() != before;
        Ok(changed)
    }

    pub fn remove_edge(
        &mut self,
        subject: Kind,
        relation: Kind,
        target: Kind,
    ) -> Result<bool, SemanticError> {
        let before = self.registry.version();
        let changed = self.registry.remove_edge(subject, relation, target)?;
        self.changed |= changed || self.registry.version() != before;
        Ok(changed)
    }

    pub fn ensure_builtins(&mut self) -> Result<Builtins, SemanticError> {
        let before = self.registry.version();
        let builtins = self.registry.ensure_builtins()?;
        self.changed |= self.registry.version() != before;
        Ok(builtins)
    }

    pub fn apply_command(&mut self, command: SemanticCommand) -> Result<bool, SemanticError> {
        let before = self.registry.version();
        let changed = self.registry.apply_command(command)?;
        self.changed |= changed || self.registry.version() != before;
        Ok(changed)
    }

    pub fn apply_commands<I>(&mut self, commands: I) -> Result<bool, SemanticError>
    where
        I: IntoIterator<Item = SemanticCommand>,
    {
        let mut changed = false;
        for command in commands {
            changed |= self.apply_command(command)?;
        }
        Ok(changed)
    }

    /// Finalize the batch and build a fresh snapshot.
    pub fn snapshot(self) -> SemanticSnapshot {
        let mut this = self;
        this.finish();
        this.registry.snapshot()
    }
}

impl<'a> std::ops::Deref for SemanticRegistryBatch<'a> {
    type Target = SemanticRegistry;

    fn deref(&self) -> &Self::Target {
        self.registry
    }
}

impl<'a> Drop for SemanticRegistryBatch<'a> {
    fn drop(&mut self) {
        self.finish();
    }
}

pub(crate) fn canonicalize_name(name: &str) -> Result<Arc<str>, SemanticError> {
    let canonical = name.trim();
    if canonical.is_empty() {
        return Err(SemanticError::InvalidName {
            name: name.to_string(),
        });
    }
    Ok(Arc::from(canonical))
}

pub(crate) fn hash_canonical_name(name: &str) -> Kind {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"bevy_semantics.kind.v1:");
    hasher.update(name.as_bytes());
    let digest = hasher.finalize();
    let bytes = digest.as_bytes();
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&bytes[..8]);
    Kind::from(u64::from_le_bytes(raw))
}
