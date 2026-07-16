//! Bevy resources, plugin wiring, and queued semantic commands.

use std::any::type_name;
use std::collections::VecDeque;
use std::sync::Arc;

use bevy_app::{App, Plugin, PostUpdate};
use bevy_ecs::prelude::{Commands, ResMut, Resource, World};

use crate::direction::EdgeDirection;
use crate::query::SemanticCommand;
use crate::registry::{canonicalize_name, hash_canonical_name};
use crate::{
    EdgeRegistration, Kind, KindRegistration, SemanticComponents, SemanticError, SemanticRegistry,
    SemanticSnapshot,
};

/// Bevy resource that owns semantic authoring state and the cached read model.
#[derive(Resource, Debug, Clone, Default)]
pub struct Semantics(SemanticRegistry);

impl std::ops::Deref for Semantics {
    type Target = SemanticRegistry;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Semantics {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Semantics {
    /// Start a fluent semantic edit chain.
    pub fn edit(&mut self) -> SemanticEdit<'_> {
        SemanticEdit::new(self)
    }

    /// Register a canonical kind immediately.
    pub fn register_kind(&mut self, name: impl AsRef<str>) -> Result<Kind, SemanticError> {
        self.0.register_kind(name)
    }

    /// Register one compile-time kind and its canonical name immediately.
    pub fn register_const(
        &mut self,
        name: &'static str,
        kind: Kind,
    ) -> Result<Kind, SemanticError> {
        self.0.register_const(name, kind)
    }

    /// Register a group of compile-time kind declarations immediately.
    pub fn register_consts(
        &mut self,
        declarations: &[KindRegistration],
    ) -> Result<(), SemanticError> {
        self.0.register_consts(declarations)
    }

    /// Register the Rust type `T` under its default canonical name.
    pub fn typed_kind<T>(&mut self) -> Result<Kind, SemanticError>
    where
        T: 'static,
    {
        self.0.typed_kind::<T>()
    }

    /// Register the Rust type `T` under an explicit canonical name.
    pub fn typed_kind_named<T>(&mut self, name: impl AsRef<str>) -> Result<Kind, SemanticError>
    where
        T: 'static,
    {
        self.0.typed_kind_named::<T>(name)
    }

    /// Unregister a kind immediately.
    pub fn unregister_kind(&mut self, kind: Kind) -> Result<bool, SemanticError> {
        self.0.unregister_kind(kind)
    }

    /// Tombstone a kind while retaining its stable identity and Rust type binding.
    pub fn tombstone_kind(&mut self, kind: Kind) -> Result<bool, SemanticError> {
        self.0.tombstone_kind(kind)
    }

    /// Reactivate a tombstoned kind.
    pub fn revive_kind(&mut self, kind: Kind) -> Result<bool, SemanticError> {
        self.0.revive_kind(kind)
    }

    pub(crate) fn bind_static_component<T>(
        &mut self,
        name: &'static str,
        kind: Kind,
    ) -> Result<(), SemanticError>
    where
        T: 'static,
    {
        self.0.bind_static_component::<T>(name, kind)
    }

    pub(crate) fn has_static_component_binding<T>(&self, name: &str, kind: Kind) -> bool
    where
        T: 'static,
    {
        self.0.has_static_component_binding::<T>(name, kind)
    }

    /// Return the seeded core handles.
    pub fn core(&self) -> crate::registry::Core {
        self.0.core()
    }

    /// Unregister every kind whose canonical name lives under the given namespace prefix.
    pub fn unregister_namespace(
        &mut self,
        namespace: impl AsRef<str>,
    ) -> Result<bool, SemanticError> {
        self.0.unregister_namespace(namespace)
    }

    /// Resolve a canonical kind name.
    pub fn kind(&self, name: impl AsRef<str>) -> Result<Kind, SemanticError> {
        let canonical = canonicalize_name(name.as_ref())?;
        self.0
            .kind(canonical.as_ref())
            .ok_or_else(|| SemanticError::UnknownKind {
                kind: hash_canonical_name(canonical.as_ref()),
            })
    }

    /// Resolve the kind bound to a Rust type.
    pub fn kind_of<T>(&self) -> Result<Kind, SemanticError>
    where
        T: 'static,
    {
        self.0
            .kind_of::<T>()
            .ok_or_else(|| SemanticError::UnknownKind {
                kind: hash_canonical_name(type_name::<T>()),
            })
    }

    /// Return the cached immutable read model for this semantic state.
    pub fn snapshot(&self) -> Arc<SemanticSnapshot> {
        self.0.snapshot_cached()
    }

    /// Check whether `subject` is-a `target`.
    pub fn is_a(&self, subject: Kind, target: Kind) -> bool {
        self.snapshot().is_a(subject, target)
    }

    /// Check whether an exact edge exists.
    pub fn has_edge(&self, subject: Kind, relation: Kind, target: Kind) -> bool {
        self.0.has_edge(subject, relation, target)
    }

    /// Return all direct targets for a `subject -> relation` pair.
    pub fn targets(&self, subject: Kind, relation: Kind) -> Vec<Kind> {
        self.snapshot().targets(subject, relation).to_vec()
    }

    /// Return all direct subjects for a `relation -> target` pair.
    pub fn subjects(&self, relation: Kind, target: Kind) -> Vec<Kind> {
        self.snapshot().subjects(relation, target).to_vec()
    }

    /// Return direct neighbors for a kind and relation.
    pub fn neighbors(&self, kind: Kind, relation: Kind, direction: EdgeDirection) -> Vec<Kind> {
        self.snapshot().neighbors(kind, relation, direction)
    }

    /// Return kinds reachable within the given depth.
    pub fn reachable(
        &self,
        kind: Kind,
        relation: Kind,
        direction: EdgeDirection,
        depth: usize,
    ) -> Vec<Kind> {
        self.snapshot().reachable(kind, relation, direction, depth)
    }

    /// Check whether a path exists within the given depth.
    pub fn can_reach(&self, start: Kind, relation: Kind, target: Kind, depth: usize) -> bool {
        self.snapshot().can_reach(start, relation, target, depth)
    }

    /// Insert a semantic edge immediately.
    pub fn add_edge(
        &mut self,
        subject: Kind,
        relation: Kind,
        target: Kind,
    ) -> Result<bool, SemanticError> {
        self.0.add_edge(subject, relation, target)
    }

    /// Insert semantic edges transactionally from tuple records.
    pub fn add_edges(
        &mut self,
        edges: impl AsRef<[EdgeRegistration]>,
    ) -> Result<bool, SemanticError> {
        self.0.add_edges(edges)
    }

    /// Remove a semantic edge immediately.
    pub fn remove_edge(
        &mut self,
        subject: Kind,
        relation: Kind,
        target: Kind,
    ) -> Result<bool, SemanticError> {
        self.0.remove_edge(subject, relation, target)
    }
}

/// Fluent semantic edit chain that applies mutations immediately and accumulates the first error.
#[must_use = "finish the edit chain with .expect(...) or .finish()"]
#[derive(Debug)]
pub struct SemanticEdit<'a> {
    semantics: &'a mut Semantics,
    error: Result<(), SemanticError>,
}

impl<'a> SemanticEdit<'a> {
    fn new(semantics: &'a mut Semantics) -> Self {
        Self {
            semantics,
            error: Ok(()),
        }
    }

    /// Register a canonical kind immediately.
    pub fn register_kind(mut self, name: impl Into<Arc<str>>) -> Self {
        if self.error.is_ok() {
            let name = name.into();
            self.error = self.semantics.0.register_kind(name.as_ref()).map(|_| ());
        }
        self
    }

    /// Register one compile-time kind and its canonical name immediately.
    pub fn register_const(mut self, name: &'static str, kind: Kind) -> Self {
        if self.error.is_ok() {
            self.error = self.semantics.0.register_const(name, kind).map(|_| ());
        }
        self
    }

    /// Register a group of compile-time kind declarations immediately.
    pub fn register_consts(mut self, declarations: &[KindRegistration]) -> Self {
        if self.error.is_ok() {
            self.error = self.semantics.0.register_consts(declarations);
        }
        self
    }

    /// Unregister a kind immediately.
    pub fn unregister_kind(mut self, kind: Kind) -> Self {
        if self.error.is_ok() {
            self.error = self.semantics.0.unregister_kind(kind).map(|_| ());
        }
        self
    }

    /// Tombstone a kind without removing its stable identity binding.
    pub fn tombstone_kind(mut self, kind: Kind) -> Self {
        if self.error.is_ok() {
            self.error = self.semantics.0.tombstone_kind(kind).map(|_| ());
        }
        self
    }

    /// Reactivate a tombstoned kind.
    pub fn revive_kind(mut self, kind: Kind) -> Self {
        if self.error.is_ok() {
            self.error = self.semantics.0.revive_kind(kind).map(|_| ());
        }
        self
    }

    /// Unregister every kind under the given namespace prefix immediately.
    pub fn unregister_namespace(mut self, namespace: impl Into<Arc<str>>) -> Self {
        if self.error.is_ok() {
            let namespace = namespace.into();
            self.error = self
                .semantics
                .0
                .unregister_namespace(namespace.as_ref())
                .map(|_| ());
        }
        self
    }

    /// Register a typed kind immediately.
    pub fn typed_kind<T>(mut self) -> Self
    where
        T: 'static,
    {
        if self.error.is_ok() {
            self.error = self.semantics.0.typed_kind::<T>().map(|_| ());
        }
        self
    }

    /// Register a typed kind with an explicit name immediately.
    pub fn typed_kind_named<T>(mut self, name: impl Into<Arc<str>>) -> Self
    where
        T: 'static,
    {
        if self.error.is_ok() {
            let name = name.into();
            self.error = self
                .semantics
                .0
                .typed_kind_named::<T>(name.as_ref())
                .map(|_| ());
        }
        self
    }

    /// Add an edge to the current semantic state.
    pub fn add_edge(mut self, subject: Kind, relation: Kind, target: Kind) -> Self {
        if self.error.is_ok() {
            self.error = self
                .semantics
                .0
                .add_edge(subject, relation, target)
                .map(|_| ());
        }
        self
    }

    /// Add tuple-form edges transactionally to the current semantic state.
    pub fn add_edges(mut self, edges: impl AsRef<[EdgeRegistration]>) -> Self {
        if self.error.is_ok() {
            self.error = self.semantics.0.add_edges(edges).map(|_| ());
        }
        self
    }

    /// Remove an edge from the current semantic state.
    pub fn remove_edge(mut self, subject: Kind, relation: Kind, target: Kind) -> Self {
        if self.error.is_ok() {
            self.error = self
                .semantics
                .0
                .remove_edge(subject, relation, target)
                .map(|_| ());
        }
        self
    }

    /// Panic on error and return the underlying semantic resource.
    pub fn expect(self, message: &str) -> &'a mut Semantics {
        if let Err(error) = self.error {
            panic!("{message}: {error}");
        }
        self.semantics
    }

    /// Finish the edit chain and return the semantic resource or the first error.
    pub fn finish(self) -> Result<&'a mut Semantics, SemanticError> {
        self.error.map(|()| self.semantics)
    }
}

/// Batches of semantic commands produced off-thread and played back later on the main thread.
#[derive(Resource, Debug, Default)]
pub struct SemanticPlaybackQueue {
    batches: VecDeque<Vec<SemanticCommand>>,
}

impl SemanticPlaybackQueue {
    /// Enqueue one batch of semantic commands for later playback.
    pub fn enqueue(&mut self, commands: impl IntoIterator<Item = SemanticCommand>) {
        self.batches.push_back(commands.into_iter().collect());
    }

    /// Return `true` when no command batches are waiting.
    pub fn is_empty(&self) -> bool {
        self.batches.is_empty()
    }

    /// Drain all queued batches into the provided semantic state.
    pub fn playback(&mut self, semantics: &mut SemanticRegistry) -> Result<bool, SemanticError> {
        let mut changed = false;
        while let Some(batch) = self.batches.pop_front() {
            changed |= semantics.apply_commands(batch)?;
        }
        Ok(changed)
    }
}

/// Buffered semantic commands staged through Bevy.
#[derive(Resource, Debug, Default)]
struct SemanticCommandQueue(pub Vec<SemanticCommand>);

impl SemanticCommandQueue {
    /// Queue a semantic command for later application.
    pub fn push(&mut self, command: SemanticCommand) {
        self.0.push(command);
    }

    /// Drain all queued semantic commands.
    pub fn drain(&mut self) -> Vec<SemanticCommand> {
        std::mem::take(&mut self.0)
    }
}

/// Bevy plugin that installs semantic state, component mappings, playback, and command buffers.
pub struct SemanticsPlugin;

impl Plugin for SemanticsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Semantics>()
            .init_resource::<SemanticComponents>()
            .init_resource::<SemanticPlaybackQueue>()
            .init_resource::<SemanticCommandQueue>()
            .add_systems(PostUpdate, apply_semantic_commands);
    }
}

/// Command extensions that stage semantic mutations for later application.
#[doc(hidden)]
pub trait SemanticCommandsExt<'w, 's> {
    fn register_kind(&mut self, name: impl Into<Arc<str>>) -> &mut Self;

    fn unregister_kind(&mut self, kind: Kind) -> &mut Self;

    fn tombstone_kind(&mut self, kind: Kind) -> &mut Self;

    fn revive_kind(&mut self, kind: Kind) -> &mut Self;

    fn unregister_namespace(&mut self, namespace: impl Into<Arc<str>>) -> &mut Self;

    fn typed_kind<T>(&mut self) -> &mut Self
    where
        T: 'static;

    fn typed_kind_named<T>(&mut self, name: impl Into<Arc<str>>) -> &mut Self
    where
        T: 'static;

    fn add_edge(&mut self, subject: Kind, relation: Kind, target: Kind) -> &mut Self;

    fn add_edges(&mut self, edges: impl AsRef<[EdgeRegistration]>) -> &mut Self;

    fn remove_edge(&mut self, subject: Kind, relation: Kind, target: Kind) -> &mut Self;
}

impl<'w, 's> SemanticCommandsExt<'w, 's> for Commands<'w, 's> {
    fn register_kind(&mut self, name: impl Into<Arc<str>>) -> &mut Self {
        let name = name.into();
        self.queue(move |world: &mut World| {
            let mut queue = world
                .get_resource_mut::<SemanticCommandQueue>()
                .expect("semantic command buffer is missing; add SemanticsPlugin first");
            queue.push(SemanticCommand::RegisterKind { name });
        });
        self
    }

    fn unregister_kind(&mut self, kind: Kind) -> &mut Self {
        self.queue(move |world: &mut World| {
            let mut queue = world
                .get_resource_mut::<SemanticCommandQueue>()
                .expect("semantic command buffer is missing; add SemanticsPlugin first");
            queue.push(SemanticCommand::UnregisterKind { kind });
        });
        self
    }

    fn tombstone_kind(&mut self, kind: Kind) -> &mut Self {
        self.queue(move |world: &mut World| {
            let mut queue = world
                .get_resource_mut::<SemanticCommandQueue>()
                .expect("semantic command buffer is missing; add SemanticsPlugin first");
            queue.push(SemanticCommand::TombstoneKind { kind });
        });
        self
    }

    fn revive_kind(&mut self, kind: Kind) -> &mut Self {
        self.queue(move |world: &mut World| {
            let mut queue = world
                .get_resource_mut::<SemanticCommandQueue>()
                .expect("semantic command buffer is missing; add SemanticsPlugin first");
            queue.push(SemanticCommand::ReviveKind { kind });
        });
        self
    }

    fn unregister_namespace(&mut self, namespace: impl Into<Arc<str>>) -> &mut Self {
        let namespace = namespace.into();
        self.queue(move |world: &mut World| {
            let mut queue = world
                .get_resource_mut::<SemanticCommandQueue>()
                .expect("semantic command buffer is missing; add SemanticsPlugin first");
            queue.push(SemanticCommand::UnregisterNamespace { namespace });
        });
        self
    }

    fn typed_kind<T>(&mut self) -> &mut Self
    where
        T: 'static,
    {
        let name: Arc<str> = Arc::from(type_name::<T>());
        self.queue(move |world: &mut World| {
            let mut queue = world
                .get_resource_mut::<SemanticCommandQueue>()
                .expect("semantic command buffer is missing; add SemanticsPlugin first");
            queue.push(SemanticCommand::RegisterTypedKind {
                type_id: std::any::TypeId::of::<T>(),
                name: Some(name),
            });
        });
        self
    }

    fn typed_kind_named<T>(&mut self, name: impl Into<Arc<str>>) -> &mut Self
    where
        T: 'static,
    {
        let name = name.into();
        self.queue(move |world: &mut World| {
            let mut queue = world
                .get_resource_mut::<SemanticCommandQueue>()
                .expect("semantic command buffer is missing; add SemanticsPlugin first");
            queue.push(SemanticCommand::RegisterTypedKind {
                type_id: std::any::TypeId::of::<T>(),
                name: Some(name),
            });
        });
        self
    }

    fn add_edge(&mut self, subject: Kind, relation: Kind, target: Kind) -> &mut Self {
        self.queue(move |world: &mut World| {
            let mut queue = world
                .get_resource_mut::<SemanticCommandQueue>()
                .expect("semantic command buffer is missing; add SemanticsPlugin first");
            queue.push(SemanticCommand::AddEdge {
                subject,
                relation,
                target,
            });
        });
        self
    }

    fn add_edges(&mut self, edges: impl AsRef<[EdgeRegistration]>) -> &mut Self {
        let edges = edges.as_ref().to_vec();
        self.queue(move |world: &mut World| {
            let mut queue = world
                .get_resource_mut::<SemanticCommandQueue>()
                .expect("semantic command buffer is missing; add SemanticsPlugin first");
            queue.push(SemanticCommand::AddEdges { edges });
        });
        self
    }

    fn remove_edge(&mut self, subject: Kind, relation: Kind, target: Kind) -> &mut Self {
        self.queue(move |world: &mut World| {
            let mut queue = world
                .get_resource_mut::<SemanticCommandQueue>()
                .expect("semantic command buffer is missing; add SemanticsPlugin first");
            queue.push(SemanticCommand::RemoveEdge {
                subject,
                relation,
                target,
            });
        });
        self
    }
}

pub use SemanticCommandsExt as CommandsExt;

fn apply_semantic_commands(
    mut semantics: ResMut<'_, Semantics>,
    mut queue: ResMut<'_, SemanticCommandQueue>,
) {
    let commands = queue.drain();
    semantics
        .apply_commands(commands)
        .unwrap_or_else(|error| panic!("failed to apply semantic command: {error}"));
}
