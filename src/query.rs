//! Semantic query model, builders, and compiled query plans.

use std::any::TypeId;
use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, RwLock};

use crate::direction::EdgeDirection;
use crate::error::SemanticError;
use crate::kind::Kind;
use crate::snapshot::{SemanticSnapshot, TraversalParams};

type EdgeQueryCacheEntry = Option<(u64, Arc<CompiledEdgeQuery>)>;
type TraversalQueryCacheEntry = Option<(u64, Arc<CompiledTraversalQuery>)>;
type EdgeQueryLocalCache = Arc<RwLock<EdgeQueryCacheEntry>>;
type TraversalQueryLocalCache = Arc<RwLock<TraversalQueryCacheEntry>>;

/// Canonical semantic edge record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SemanticEdge {
    pub subject: Kind,
    pub relation: Kind,
    pub target: Kind,
}

/// Tuple form accepted by bulk edge-registration APIs.
pub type EdgeRegistration = (Kind, Kind, Kind);

impl SemanticEdge {
    #[inline]
    /// Construct a semantic edge record.
    pub const fn new(subject: Kind, relation: Kind, target: Kind) -> Self {
        Self {
            subject,
            relation,
            target,
        }
    }
}

impl From<EdgeRegistration> for SemanticEdge {
    fn from((subject, relation, target): EdgeRegistration) -> Self {
        Self::new(subject, relation, target)
    }
}

/// Public semantic commands staged through Bevy or applied directly.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SemanticCommand {
    RegisterKind {
        name: Arc<str>,
    },
    UnregisterNamespace {
        namespace: Arc<str>,
    },
    UnregisterKind {
        kind: Kind,
    },
    TombstoneKind {
        kind: Kind,
    },
    ReviveKind {
        kind: Kind,
    },
    RegisterTypedKind {
        type_id: TypeId,
        name: Option<Arc<str>>,
    },
    AddEdge {
        subject: Kind,
        relation: Kind,
        target: Kind,
    },
    AddEdges {
        edges: Vec<EdgeRegistration>,
    },
    RemoveEdge {
        subject: Kind,
        relation: Kind,
        target: Kind,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
enum EdgeSelection {
    #[default]
    Edges,
    Subjects,
    Targets,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum DepthSpec {
    One,
    Exact(usize),
    To(usize),
    #[default]
    Transitive,
}

/// Declarative edge query definition.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EdgeQuery {
    subjects: Vec<Kind>,
    relations: Vec<Kind>,
    targets: Vec<Kind>,
    direction: EdgeDirection,
    selection: EdgeSelection,
}

impl Default for EdgeQuery {
    fn default() -> Self {
        Self {
            subjects: Vec::new(),
            relations: Vec::new(),
            targets: Vec::new(),
            direction: EdgeDirection::Outgoing,
            selection: EdgeSelection::Edges,
        }
    }
}

impl EdgeQuery {
    /// Start a new edge query builder.
    pub fn builder() -> EdgeQueryBuilder {
        EdgeQueryBuilder::new(None)
    }
}

/// Query execution options shared by traversal builders and compiled plans.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct QueryOptions {
    pub include_start: bool,
    pub dedup: bool,
    pub stop_on_match: bool,
    pub max_results: Option<usize>,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            include_start: false,
            dedup: true,
            stop_on_match: false,
            max_results: None,
        }
    }
}

/// Declarative traversal query definition.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TraversalQuery {
    seeds: Vec<Kind>,
    relations: Vec<Kind>,
    direction: EdgeDirection,
    depth: DepthSpec,
    options: QueryOptions,
}

impl Default for TraversalQuery {
    fn default() -> Self {
        Self {
            seeds: Vec::new(),
            relations: Vec::new(),
            direction: EdgeDirection::Outgoing,
            depth: DepthSpec::Transitive,
            options: QueryOptions::default(),
        }
    }
}

impl TraversalQuery {
    /// Start a new traversal query builder.
    pub fn builder() -> TraversalQueryBuilder {
        TraversalQueryBuilder::new(None)
    }
}

/// Kind-set source used to compose query filters.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SetSource {
    Kinds(Vec<Kind>),
    EdgeQuery(Box<EdgeQueryBuilder>),
    TraversalQuery(Box<TraversalQueryBuilder>),
    CompiledEdge(Box<CompiledEdgeQuery>),
    CompiledTraversal(Box<CompiledTraversalQuery>),
}

impl From<Kind> for SetSource {
    fn from(value: Kind) -> Self {
        Self::Kinds(vec![value])
    }
}

impl From<Vec<Kind>> for SetSource {
    fn from(value: Vec<Kind>) -> Self {
        Self::Kinds(value)
    }
}

impl From<&[Kind]> for SetSource {
    fn from(value: &[Kind]) -> Self {
        Self::Kinds(value.to_vec())
    }
}

impl From<EdgeQueryBuilder> for SetSource {
    fn from(value: EdgeQueryBuilder) -> Self {
        Self::EdgeQuery(Box::new(value))
    }
}

impl From<TraversalQueryBuilder> for SetSource {
    fn from(value: TraversalQueryBuilder) -> Self {
        Self::TraversalQuery(Box::new(value))
    }
}

impl From<CompiledEdgeQuery> for SetSource {
    fn from(value: CompiledEdgeQuery) -> Self {
        Self::CompiledEdge(Box::new(value))
    }
}

impl From<CompiledTraversalQuery> for SetSource {
    fn from(value: CompiledTraversalQuery) -> Self {
        Self::CompiledTraversal(Box::new(value))
    }
}

impl SetSource {
    fn evaluate(&self, snapshot: &SemanticSnapshot) -> Result<Vec<Kind>, SemanticError> {
        match self {
            SetSource::Kinds(values) => Ok(values.clone()),
            SetSource::EdgeQuery(query) => match query.query.selection {
                EdgeSelection::Subjects => query.run_subjects(snapshot),
                EdgeSelection::Targets => query.run_targets(snapshot),
                other => Err(SemanticError::InvalidQueryState {
                    query: "edge",
                    reason: format!("cannot use {:?} selection as a set source", other),
                }),
            },
            SetSource::TraversalQuery(query) => query.run_kinds(snapshot),
            SetSource::CompiledEdge(query) => match query.query.selection {
                EdgeSelection::Subjects => query.run_subjects(snapshot),
                EdgeSelection::Targets => query.run_targets(snapshot),
                other => Err(SemanticError::InvalidQueryState {
                    query: "edge",
                    reason: format!("cannot use {:?} selection as a set source", other),
                }),
            },
            SetSource::CompiledTraversal(query) => query.run_kinds(snapshot),
        }
    }
}

#[must_use]
/// Fluent builder for edge queries.
#[derive(Clone, Debug)]
pub struct EdgeQueryBuilder {
    bound_version: Option<u64>,
    query: EdgeQuery,
    subject_sources: Vec<SetSource>,
    target_sources: Vec<SetSource>,
    fingerprint: u64,
    compiled_cache: EdgeQueryLocalCache,
}

impl Default for EdgeQueryBuilder {
    fn default() -> Self {
        Self::new(None)
    }
}

impl EdgeQueryBuilder {
    pub(crate) fn new(bound_version: Option<u64>) -> Self {
        let mut builder = Self {
            bound_version,
            query: EdgeQuery::default(),
            subject_sources: Vec::new(),
            target_sources: Vec::new(),
            fingerprint: 0,
            compiled_cache: Arc::new(RwLock::new(None)),
        };
        builder.refresh_fingerprint();
        builder
    }

    pub fn subject(mut self, subject: Kind) -> Self {
        self.query.subjects.push(subject);
        self.refresh_fingerprint();
        self
    }

    pub fn subjects(mut self, subjects: impl IntoIterator<Item = Kind>) -> Self {
        self.query.subjects.extend(subjects);
        self.refresh_fingerprint();
        self
    }

    pub fn relation(mut self, relation: Kind) -> Self {
        self.query.relations.push(relation);
        self.refresh_fingerprint();
        self
    }

    pub fn relations(mut self, relations: impl IntoIterator<Item = Kind>) -> Self {
        self.query.relations.extend(relations);
        self.refresh_fingerprint();
        self
    }

    pub fn target(mut self, target: Kind) -> Self {
        self.query.targets.push(target);
        self.refresh_fingerprint();
        self
    }

    pub fn targets(mut self, targets: impl IntoIterator<Item = Kind>) -> Self {
        self.query.targets.extend(targets);
        self.refresh_fingerprint();
        self
    }

    pub fn outgoing(mut self) -> Self {
        self.query.direction = EdgeDirection::Outgoing;
        self.refresh_fingerprint();
        self
    }

    pub fn incoming(mut self) -> Self {
        self.query.direction = EdgeDirection::Incoming;
        self.refresh_fingerprint();
        self
    }

    pub fn both(mut self) -> Self {
        self.query.direction = EdgeDirection::Both;
        self.refresh_fingerprint();
        self
    }

    pub fn subjects_from(mut self, query: impl Into<SetSource>) -> Self {
        self.subject_sources.push(query.into());
        self.refresh_fingerprint();
        self
    }

    pub fn targets_from(mut self, query: impl Into<SetSource>) -> Self {
        self.target_sources.push(query.into());
        self.refresh_fingerprint();
        self
    }

    pub fn intersect_subjects_from(self, query: impl Into<SetSource>) -> Self {
        self.subjects_from(query)
    }

    pub fn intersect_targets_from(self, query: impl Into<SetSource>) -> Self {
        self.targets_from(query)
    }

    pub fn select_edges(mut self) -> Self {
        self.query.selection = EdgeSelection::Edges;
        self.refresh_fingerprint();
        self
    }

    pub fn select_subjects(mut self) -> Self {
        self.query.selection = EdgeSelection::Subjects;
        self.refresh_fingerprint();
        self
    }

    pub fn select_targets(mut self) -> Self {
        self.query.selection = EdgeSelection::Targets;
        self.refresh_fingerprint();
        self
    }

    pub fn compile(&self, snapshot: &SemanticSnapshot) -> Result<CompiledEdgeQuery, SemanticError> {
        Ok(self.compile_cached(snapshot)?.as_ref().clone())
    }

    pub fn run_edges(
        &self,
        snapshot: &SemanticSnapshot,
    ) -> Result<Vec<SemanticEdge>, SemanticError> {
        Ok(self.compile_cached(snapshot)?.matches.clone())
    }

    pub fn run_subjects(&self, snapshot: &SemanticSnapshot) -> Result<Vec<Kind>, SemanticError> {
        Ok(self.compile_cached(snapshot)?.subjects.clone())
    }

    pub fn run_targets(&self, snapshot: &SemanticSnapshot) -> Result<Vec<Kind>, SemanticError> {
        Ok(self.compile_cached(snapshot)?.targets.clone())
    }

    pub fn run_exists(&self, snapshot: &SemanticSnapshot) -> Result<bool, SemanticError> {
        Ok(!self.compile_cached(snapshot)?.matches.is_empty())
    }

    pub fn run_count(&self, snapshot: &SemanticSnapshot) -> Result<usize, SemanticError> {
        Ok(self.compile_cached(snapshot)?.matches.len())
    }

    fn ensure_version(&self, snapshot: &SemanticSnapshot) -> Result<(), SemanticError> {
        if let Some(bound_version) = self.bound_version {
            if bound_version != snapshot.version() {
                return Err(SemanticError::StaleCompiledQuery {
                    domain: "edge query",
                    compiled_version: bound_version,
                    current_version: snapshot.version(),
                });
            }
        }
        Ok(())
    }

    fn compile_cached(
        &self,
        snapshot: &SemanticSnapshot,
    ) -> Result<Arc<CompiledEdgeQuery>, SemanticError> {
        self.ensure_version(snapshot)?;

        if let Some(cached) = self.local_cache(snapshot.version()) {
            return Ok(cached);
        }

        if let Some(cached) = snapshot
            .edge_query_cache()
            .read()
            .expect("edge query cache poisoned")
            .get(self)
            .cloned()
        {
            self.store_local_cache(snapshot.version(), Arc::clone(&cached));
            return Ok(cached);
        }

        let compiled = Arc::new(self.compile_uncached(snapshot)?);

        let mut cache = snapshot
            .edge_query_cache()
            .write()
            .expect("edge query cache poisoned");
        if let Some(cached) = cache.get(self).cloned() {
            self.store_local_cache(snapshot.version(), Arc::clone(&cached));
            return Ok(cached);
        }

        cache.insert(self.clone(), Arc::clone(&compiled));
        self.store_local_cache(snapshot.version(), Arc::clone(&compiled));
        Ok(compiled)
    }

    fn compile_uncached(
        &self,
        snapshot: &SemanticSnapshot,
    ) -> Result<CompiledEdgeQuery, SemanticError> {
        let query = normalize_edge_query(self.query.clone());
        let subject_filter = resolve_kind_set(snapshot, &query.subjects, &self.subject_sources)?;
        let target_filter = resolve_kind_set(snapshot, &query.targets, &self.target_sources)?;
        let matches = run_edge_query(
            snapshot,
            &query,
            subject_filter.as_deref(),
            target_filter.as_deref(),
        )?;
        let mut subjects = Vec::with_capacity(matches.len());
        let mut targets = Vec::with_capacity(matches.len());
        for edge in &matches {
            subjects.push(edge.subject);
            targets.push(edge.target);
        }
        Ok(CompiledEdgeQuery {
            version: snapshot.version(),
            query,
            matches,
            subjects: dedup_sorted_kinds(subjects, snapshot),
            targets: dedup_sorted_kinds(targets, snapshot),
        })
    }
}

impl EdgeQueryBuilder {
    fn refresh_fingerprint(&mut self) {
        self.fingerprint = hash_state(&(
            self.bound_version,
            &self.query,
            &self.subject_sources,
            &self.target_sources,
        ));
        self.clear_cache();
    }

    fn clear_cache(&mut self) {
        *self
            .compiled_cache
            .write()
            .expect("edge query cache poisoned") = None;
    }

    fn local_cache(&self, snapshot_version: u64) -> Option<Arc<CompiledEdgeQuery>> {
        self.compiled_cache
            .read()
            .expect("edge query cache poisoned")
            .as_ref()
            .and_then(|(version, compiled)| {
                if *version == snapshot_version {
                    Some(Arc::clone(compiled))
                } else {
                    None
                }
            })
    }

    fn store_local_cache(&self, snapshot_version: u64, compiled: Arc<CompiledEdgeQuery>) {
        let mut cache = self
            .compiled_cache
            .write()
            .expect("edge query cache poisoned");
        if cache
            .as_ref()
            .is_some_and(|(version, _)| *version == snapshot_version)
        {
            return;
        }
        *cache = Some((snapshot_version, compiled));
    }
}

impl PartialEq for EdgeQueryBuilder {
    fn eq(&self, other: &Self) -> bool {
        self.fingerprint == other.fingerprint
            && self.bound_version == other.bound_version
            && self.query == other.query
            && self.subject_sources == other.subject_sources
            && self.target_sources == other.target_sources
    }
}

impl Eq for EdgeQueryBuilder {}

impl Hash for EdgeQueryBuilder {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.fingerprint);
    }
}

/// Compiled edge query plan.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CompiledEdgeQuery {
    version: u64,
    query: EdgeQuery,
    matches: Vec<SemanticEdge>,
    subjects: Vec<Kind>,
    targets: Vec<Kind>,
}

impl CompiledEdgeQuery {
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn run_edges(
        &self,
        snapshot: &SemanticSnapshot,
    ) -> Result<Vec<SemanticEdge>, SemanticError> {
        self.ensure_version(snapshot)?;
        Ok(self.matches.clone())
    }

    pub fn run_subjects(&self, snapshot: &SemanticSnapshot) -> Result<Vec<Kind>, SemanticError> {
        self.ensure_version(snapshot)?;
        Ok(self.subjects.clone())
    }

    pub fn run_targets(&self, snapshot: &SemanticSnapshot) -> Result<Vec<Kind>, SemanticError> {
        self.ensure_version(snapshot)?;
        Ok(self.targets.clone())
    }

    pub fn run_exists(&self, snapshot: &SemanticSnapshot) -> Result<bool, SemanticError> {
        self.ensure_version(snapshot)?;
        Ok(!self.matches.is_empty())
    }

    pub fn run_count(&self, snapshot: &SemanticSnapshot) -> Result<usize, SemanticError> {
        self.ensure_version(snapshot)?;
        Ok(self.matches.len())
    }

    fn ensure_version(&self, snapshot: &SemanticSnapshot) -> Result<(), SemanticError> {
        if self.version != snapshot.version() {
            return Err(SemanticError::StaleCompiledQuery {
                domain: "edge query",
                compiled_version: self.version,
                current_version: snapshot.version(),
            });
        }
        Ok(())
    }
}

#[must_use]
/// Fluent builder for traversal queries.
#[derive(Clone, Debug)]
pub struct TraversalQueryBuilder {
    bound_version: Option<u64>,
    query: TraversalQuery,
    fingerprint: u64,
    compiled_cache: TraversalQueryLocalCache,
}

impl Default for TraversalQueryBuilder {
    fn default() -> Self {
        Self::new(None)
    }
}

impl TraversalQueryBuilder {
    pub(crate) fn new(bound_version: Option<u64>) -> Self {
        let mut builder = Self {
            bound_version,
            query: TraversalQuery::default(),
            fingerprint: 0,
            compiled_cache: Arc::new(RwLock::new(None)),
        };
        builder.refresh_fingerprint();
        builder
    }

    pub fn seed(mut self, seed: Kind) -> Self {
        self.query.seeds.push(seed);
        self.refresh_fingerprint();
        self
    }

    pub fn seeds(mut self, seeds: impl IntoIterator<Item = Kind>) -> Self {
        self.query.seeds.extend(seeds);
        self.refresh_fingerprint();
        self
    }

    pub fn relation(mut self, relation: Kind) -> Self {
        self.query.relations.push(relation);
        self.refresh_fingerprint();
        self
    }

    pub fn relations(mut self, relations: impl IntoIterator<Item = Kind>) -> Self {
        self.query.relations.extend(relations);
        self.refresh_fingerprint();
        self
    }

    pub fn outgoing(mut self) -> Self {
        self.query.direction = EdgeDirection::Outgoing;
        self.refresh_fingerprint();
        self
    }

    pub fn incoming(mut self) -> Self {
        self.query.direction = EdgeDirection::Incoming;
        self.refresh_fingerprint();
        self
    }

    pub fn both(mut self) -> Self {
        self.query.direction = EdgeDirection::Both;
        self.refresh_fingerprint();
        self
    }

    pub fn depth_1(mut self) -> Self {
        self.query.depth = DepthSpec::One;
        self.refresh_fingerprint();
        self
    }

    pub fn depth_exact(mut self, depth: usize) -> Self {
        self.query.depth = DepthSpec::Exact(depth);
        self.refresh_fingerprint();
        self
    }

    pub fn depth_to(mut self, depth: usize) -> Self {
        self.query.depth = DepthSpec::To(depth);
        self.refresh_fingerprint();
        self
    }

    pub fn transitive(mut self) -> Self {
        self.query.depth = DepthSpec::Transitive;
        self.refresh_fingerprint();
        self
    }

    pub fn include_start(mut self, include_start: bool) -> Self {
        self.query.options.include_start = include_start;
        self.refresh_fingerprint();
        self
    }

    pub fn dedup(mut self, dedup: bool) -> Self {
        self.query.options.dedup = dedup;
        self.refresh_fingerprint();
        self
    }

    pub fn stop_on_match(mut self, stop_on_match: bool) -> Self {
        self.query.options.stop_on_match = stop_on_match;
        self.refresh_fingerprint();
        self
    }

    pub fn max_results(mut self, max_results: Option<usize>) -> Self {
        self.query.options.max_results = max_results;
        self.refresh_fingerprint();
        self
    }

    pub fn compile(
        &self,
        snapshot: &SemanticSnapshot,
    ) -> Result<CompiledTraversalQuery, SemanticError> {
        Ok(self.compile_cached(snapshot)?.as_ref().clone())
    }

    pub fn run_kinds(&self, snapshot: &SemanticSnapshot) -> Result<Vec<Kind>, SemanticError> {
        Ok(self.compile_cached(snapshot)?.results.clone())
    }

    pub fn run_exists(&self, snapshot: &SemanticSnapshot) -> Result<bool, SemanticError> {
        Ok(!self.compile_cached(snapshot)?.results.is_empty())
    }

    pub fn run_count(&self, snapshot: &SemanticSnapshot) -> Result<usize, SemanticError> {
        Ok(self.compile_cached(snapshot)?.results.len())
    }

    fn ensure_version(&self, snapshot: &SemanticSnapshot) -> Result<(), SemanticError> {
        if let Some(bound_version) = self.bound_version {
            if bound_version != snapshot.version() {
                return Err(SemanticError::StaleCompiledQuery {
                    domain: "traversal query",
                    compiled_version: bound_version,
                    current_version: snapshot.version(),
                });
            }
        }
        Ok(())
    }

    fn compile_cached(
        &self,
        snapshot: &SemanticSnapshot,
    ) -> Result<Arc<CompiledTraversalQuery>, SemanticError> {
        self.ensure_version(snapshot)?;

        if let Some(cached) = self.local_cache(snapshot.version()) {
            return Ok(cached);
        }

        if let Some(cached) = snapshot
            .traversal_query_cache()
            .read()
            .expect("traversal query cache poisoned")
            .get(self)
            .cloned()
        {
            self.store_local_cache(snapshot.version(), Arc::clone(&cached));
            return Ok(cached);
        }

        let compiled = Arc::new(self.compile_uncached(snapshot)?);

        let mut cache = snapshot
            .traversal_query_cache()
            .write()
            .expect("traversal query cache poisoned");
        if let Some(cached) = cache.get(self).cloned() {
            self.store_local_cache(snapshot.version(), Arc::clone(&cached));
            return Ok(cached);
        }

        cache.insert(self.clone(), Arc::clone(&compiled));
        self.store_local_cache(snapshot.version(), Arc::clone(&compiled));
        Ok(compiled)
    }

    fn compile_uncached(
        &self,
        snapshot: &SemanticSnapshot,
    ) -> Result<CompiledTraversalQuery, SemanticError> {
        let query = normalize_traversal_query(self.query.clone());
        validate_traversal_query(&query)?;
        let results = evaluate_traversal_query(snapshot, &query)?;
        Ok(CompiledTraversalQuery {
            version: snapshot.version(),
            query,
            results,
        })
    }
}

impl TraversalQueryBuilder {
    fn refresh_fingerprint(&mut self) {
        self.fingerprint = hash_state(&(self.bound_version, &self.query));
        self.clear_cache();
    }

    fn clear_cache(&mut self) {
        *self
            .compiled_cache
            .write()
            .expect("traversal query cache poisoned") = None;
    }

    fn local_cache(&self, snapshot_version: u64) -> Option<Arc<CompiledTraversalQuery>> {
        self.compiled_cache
            .read()
            .expect("traversal query cache poisoned")
            .as_ref()
            .and_then(|(version, compiled)| {
                if *version == snapshot_version {
                    Some(Arc::clone(compiled))
                } else {
                    None
                }
            })
    }

    fn store_local_cache(&self, snapshot_version: u64, compiled: Arc<CompiledTraversalQuery>) {
        let mut cache = self
            .compiled_cache
            .write()
            .expect("traversal query cache poisoned");
        if cache
            .as_ref()
            .is_some_and(|(version, _)| *version == snapshot_version)
        {
            return;
        }
        *cache = Some((snapshot_version, compiled));
    }
}

impl PartialEq for TraversalQueryBuilder {
    fn eq(&self, other: &Self) -> bool {
        self.fingerprint == other.fingerprint
            && self.bound_version == other.bound_version
            && self.query == other.query
    }
}

impl Eq for TraversalQueryBuilder {}

impl Hash for TraversalQueryBuilder {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.fingerprint);
    }
}

/// Compiled traversal query plan.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CompiledTraversalQuery {
    version: u64,
    query: TraversalQuery,
    results: Vec<Kind>,
}

impl CompiledTraversalQuery {
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn run_kinds(&self, snapshot: &SemanticSnapshot) -> Result<Vec<Kind>, SemanticError> {
        self.ensure_version(snapshot)?;
        Ok(self.results.clone())
    }

    pub fn run_exists(&self, snapshot: &SemanticSnapshot) -> Result<bool, SemanticError> {
        self.ensure_version(snapshot)?;
        Ok(!self.results.is_empty())
    }

    pub fn run_count(&self, snapshot: &SemanticSnapshot) -> Result<usize, SemanticError> {
        self.ensure_version(snapshot)?;
        Ok(self.results.len())
    }

    fn ensure_version(&self, snapshot: &SemanticSnapshot) -> Result<(), SemanticError> {
        if self.version != snapshot.version() {
            return Err(SemanticError::StaleCompiledQuery {
                domain: "traversal query",
                compiled_version: self.version,
                current_version: snapshot.version(),
            });
        }
        Ok(())
    }
}

fn normalize_edge_query(mut query: EdgeQuery) -> EdgeQuery {
    normalize_kind_list(&mut query.subjects);
    normalize_kind_list(&mut query.relations);
    normalize_kind_list(&mut query.targets);
    query
}

fn normalize_traversal_query(mut query: TraversalQuery) -> TraversalQuery {
    normalize_kind_list(&mut query.seeds);
    normalize_kind_list(&mut query.relations);
    query
}

fn normalize_kind_list(values: &mut Vec<Kind>) {
    values.sort_unstable();
    values.dedup();
}

fn hash_state<T: Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn resolve_kind_set(
    snapshot: &SemanticSnapshot,
    direct: &[Kind],
    sources: &[SetSource],
) -> Result<Option<Vec<Kind>>, SemanticError> {
    let mut current = if direct.is_empty() {
        None
    } else {
        Some(dedup_sorted_kinds(direct.to_vec(), snapshot))
    };

    for source in sources {
        let mut source_values = source.evaluate(snapshot)?;
        source_values = dedup_sorted_kinds(source_values, snapshot);
        current = Some(match current {
            Some(existing) => intersect_kind_sets(snapshot, &existing, &source_values),
            None => source_values,
        });

        if current.as_ref().is_some_and(Vec::is_empty) {
            break;
        }
    }

    Ok(current)
}

fn intersect_kind_sets(snapshot: &SemanticSnapshot, left: &[Kind], right: &[Kind]) -> Vec<Kind> {
    let mut out = Vec::with_capacity(left.len().min(right.len()));
    let mut left_index = 0usize;
    let mut right_index = 0usize;

    while left_index < left.len() && right_index < right.len() {
        match snapshot
            .dense_sort_key(left[left_index])
            .cmp(&snapshot.dense_sort_key(right[right_index]))
        {
            Ordering::Less => left_index += 1,
            Ordering::Greater => right_index += 1,
            Ordering::Equal => {
                out.push(left[left_index]);
                left_index += 1;
                right_index += 1;
            }
        }
    }

    out
}

fn dedup_sorted_kinds(mut values: Vec<Kind>, snapshot: &SemanticSnapshot) -> Vec<Kind> {
    sort_kind_list_by_dense(snapshot, &mut values);
    values.dedup();
    values
}

fn sort_kind_list_by_dense(snapshot: &SemanticSnapshot, values: &mut [Kind]) {
    values.sort_unstable_by_key(|kind| snapshot.dense_sort_key(*kind));
}

fn validate_traversal_query(query: &TraversalQuery) -> Result<(), SemanticError> {
    if query.seeds.is_empty() {
        return Err(SemanticError::EmptySeedSet);
    }
    Ok(())
}

fn evaluate_traversal_query(
    snapshot: &SemanticSnapshot,
    query: &TraversalQuery,
) -> Result<Vec<Kind>, SemanticError> {
    validate_traversal_query(query)?;

    let seeds = dedup_sorted_kinds(query.seeds.clone(), snapshot);
    let relation_filter = if query.relations.is_empty() {
        None
    } else {
        Some(query.relations.as_slice())
    };

    let max_depth = match query.depth {
        DepthSpec::One => 1,
        DepthSpec::Exact(depth) => depth,
        DepthSpec::To(depth) => depth,
        DepthSpec::Transitive => usize::MAX,
    };

    let exact_depth = match query.depth {
        DepthSpec::One => Some(1),
        DepthSpec::Exact(depth) => Some(depth),
        DepthSpec::To(_) | DepthSpec::Transitive => None,
    };

    Ok(snapshot.traverse_kinds(TraversalParams {
        seeds: &seeds,
        relation_filter,
        direction: query.direction,
        max_depth,
        exact_depth,
        include_start: query.options.include_start,
        dedup: query.options.dedup,
        stop_on_match: query.options.stop_on_match,
        max_results: query.options.max_results,
    }))
}

fn run_edge_query(
    snapshot: &SemanticSnapshot,
    query: &EdgeQuery,
    subject_filter: Option<&[Kind]>,
    target_filter: Option<&[Kind]>,
) -> Result<Vec<SemanticEdge>, SemanticError> {
    let mut out = Vec::new();

    for edge in snapshot.edges() {
        if edge_matches(query, *edge, subject_filter, target_filter) {
            out.push(*edge);
        }
    }

    Ok(out)
}

fn edge_matches(
    query: &EdgeQuery,
    edge: SemanticEdge,
    subject_filter: Option<&[Kind]>,
    target_filter: Option<&[Kind]>,
) -> bool {
    let relation_match =
        query.relations.is_empty() || query.relations.binary_search(&edge.relation).is_ok();
    if !relation_match {
        return false;
    }

    match query.direction {
        EdgeDirection::Outgoing => {
            kind_matches(subject_filter, edge.subject) && kind_matches(target_filter, edge.target)
        }
        EdgeDirection::Incoming => {
            kind_matches(subject_filter, edge.target) && kind_matches(target_filter, edge.subject)
        }
        EdgeDirection::Both => {
            (kind_matches(subject_filter, edge.subject) && kind_matches(target_filter, edge.target))
                || (kind_matches(subject_filter, edge.target)
                    && kind_matches(target_filter, edge.subject))
        }
    }
}

fn kind_matches(filter: Option<&[Kind]>, value: Kind) -> bool {
    match filter {
        None => true,
        Some(values) => values.contains(&value),
    }
}
