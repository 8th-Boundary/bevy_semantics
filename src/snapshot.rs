//! Immutable read-only semantic snapshots and query execution helpers.

use std::any::TypeId;
use std::cell::RefCell;
use std::sync::{Arc, RwLock};

use bevy_platform::collections::{HashMap, HashSet};

use crate::direction::EdgeDirection;
use crate::kind::Kind;
use crate::query::{
    CompiledEdgeQuery, CompiledTraversalQuery, EdgeQueryBuilder, TraversalQueryBuilder,
};
use crate::registry::{Core, SemanticRegistry};
use crate::{EdgeRegistration, SemanticEdge};

const EMPTY_KINDS: [Kind; 0] = [];
/// Immutable snapshot of the semantic registry state.
#[derive(Clone, Debug)]
pub struct SemanticSnapshot {
    version: u64,
    kinds: Vec<Kind>,
    kind_dense_index: HashMap<Kind, usize>,
    kind_by_name: HashMap<ArcStrKey, Kind>,
    name_by_kind: HashMap<Kind, ArcStrKey>,
    type_by_kind: HashMap<Kind, TypeId>,
    core: Core,
    is_a_cache: Arc<RwLock<HashMap<Kind, Vec<Kind>>>>,
    edge_query_cache: Arc<RwLock<HashMap<EdgeQueryBuilder, Arc<CompiledEdgeQuery>>>>,
    traversal_query_cache: Arc<RwLock<HashMap<TraversalQueryBuilder, Arc<CompiledTraversalQuery>>>>,
    edges: Vec<SemanticEdge>,
    edge_triples: HashSet<EdgeRegistration>,
    outgoing_all: HashMap<Kind, Vec<Kind>>,
    incoming_all: HashMap<Kind, Vec<Kind>>,
    outgoing_by_relation: HashMap<(Kind, Kind), Vec<Kind>>,
    incoming_by_relation: HashMap<(Kind, Kind), Vec<Kind>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ArcStrKey(Arc<str>);

impl ArcStrKey {
    fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

impl std::borrow::Borrow<str> for ArcStrKey {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl From<Arc<str>> for ArcStrKey {
    fn from(value: Arc<str>) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TraversalParams<'a> {
    pub seeds: &'a [Kind],
    pub relation_filter: Option<&'a [Kind]>,
    pub direction: EdgeDirection,
    pub max_depth: usize,
    pub exact_depth: Option<usize>,
    pub include_start: bool,
    pub dedup: bool,
    pub stop_on_match: bool,
    pub max_results: Option<usize>,
}

impl SemanticSnapshot {
    /// Build a snapshot from the current mutable registry state.
    pub(crate) fn from_registry(registry: &SemanticRegistry) -> Self {
        let kinds = if registry.tombstoned_count == 0 {
            registry
                .kinds
                .iter()
                .map(|meta| meta.kind)
                .collect::<Vec<_>>()
        } else {
            registry
                .kinds
                .iter()
                .filter(|meta| !meta.is_tombstoned())
                .map(|meta| meta.kind)
                .collect::<Vec<_>>()
        };

        let mut kind_dense_index = HashMap::with_capacity(kinds.len());
        for (index, kind) in kinds.iter().copied().enumerate() {
            kind_dense_index.insert(kind, index);
        }

        let mut kind_by_name = HashMap::with_capacity(registry.kinds.len());
        let mut name_by_kind = HashMap::with_capacity(registry.kinds.len());
        let mut type_by_kind = HashMap::with_capacity(registry.kinds.len());
        if registry.tombstoned_count == 0 {
            for meta in &registry.kinds {
                kind_by_name.insert(meta.name.clone().into(), meta.kind);
                name_by_kind.insert(meta.kind, meta.name.clone().into());
                if let Some(type_id) = meta.type_id {
                    type_by_kind.insert(meta.kind, type_id);
                }
            }
        } else {
            for meta in &registry.kinds {
                if meta.is_tombstoned() {
                    continue;
                }
                kind_by_name.insert(meta.name.clone().into(), meta.kind);
                name_by_kind.insert(meta.kind, meta.name.clone().into());
                if let Some(type_id) = meta.type_id {
                    type_by_kind.insert(meta.kind, type_id);
                }
            }
        }
        let core = registry.core();

        let mut edges = registry
            .graph
            .edges
            .iter()
            .copied()
            .map(SemanticEdge::from)
            .collect::<Vec<_>>();
        edges.sort_unstable_by_key(|edge| (edge.subject, edge.relation, edge.target));

        let mut edge_triples = HashSet::with_capacity(edges.len());
        let mut outgoing_all: HashMap<Kind, Vec<Kind>> = HashMap::with_capacity(kinds.len());
        let mut incoming_all: HashMap<Kind, Vec<Kind>> = HashMap::with_capacity(kinds.len());
        let mut outgoing_by_relation: HashMap<(Kind, Kind), Vec<Kind>> =
            HashMap::with_capacity(edges.len());
        let mut incoming_by_relation: HashMap<(Kind, Kind), Vec<Kind>> =
            HashMap::with_capacity(edges.len());

        for edge in &edges {
            edge_triples.insert((edge.subject, edge.relation, edge.target));
            outgoing_all
                .entry(edge.subject)
                .or_default()
                .push(edge.target);
            incoming_all
                .entry(edge.target)
                .or_default()
                .push(edge.subject);
            outgoing_by_relation
                .entry((edge.subject, edge.relation))
                .or_default()
                .push(edge.target);
            incoming_by_relation
                .entry((edge.relation, edge.target))
                .or_default()
                .push(edge.subject);
        }

        let mut snapshot = Self {
            version: registry.version,
            kinds,
            kind_dense_index,
            kind_by_name,
            name_by_kind,
            type_by_kind,
            core,
            is_a_cache: Arc::new(RwLock::new(HashMap::new())),
            edge_query_cache: Arc::new(RwLock::new(HashMap::new())),
            traversal_query_cache: Arc::new(RwLock::new(HashMap::new())),
            edges,
            edge_triples,
            outgoing_all,
            incoming_all,
            outgoing_by_relation,
            incoming_by_relation,
        };
        snapshot.normalize_rows();
        snapshot
    }

    /// Build a snapshot from a default registry.
    pub fn default_snapshot() -> Self {
        SemanticRegistry::default().snapshot()
    }

    /// Return the snapshot version.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Return all kinds in dense order.
    pub fn kinds(&self) -> &[Kind] {
        self.kinds.as_slice()
    }

    /// Return all canonical edges in deterministic order.
    pub fn edges(&self) -> &[SemanticEdge] {
        self.edges.as_slice()
    }

    /// Resolve a kind by canonical name.
    pub fn kind(&self, name: impl AsRef<str>) -> Option<Kind> {
        let canonical = name.as_ref().trim();
        if canonical.is_empty() {
            return None;
        }
        self.kind_by_name.get(canonical).copied()
    }

    /// Resolve the canonical name for a kind.
    pub fn name(&self, kind: Kind) -> Option<&str> {
        self.name_by_kind.get(&kind).map(ArcStrKey::as_str)
    }

    /// Resolve the Rust `TypeId` bound to a kind, if any.
    pub fn type_id(&self, kind: Kind) -> Option<TypeId> {
        self.type_by_kind.get(&kind).copied()
    }

    /// Return the canonical core kinds and relations.
    pub fn core(&self) -> Core {
        self.core
    }

    /// Check whether an exact edge exists.
    pub fn has_edge(&self, subject: Kind, relation: Kind, target: Kind) -> bool {
        self.edge_triples.contains(&(subject, relation, target))
    }

    /// Return all targets for a `subject -> relation` pair.
    pub fn targets(&self, subject: Kind, relation: Kind) -> &[Kind] {
        self.outgoing_by_relation
            .get(&(subject, relation))
            .map(Vec::as_slice)
            .unwrap_or(&EMPTY_KINDS)
    }

    /// Return all subjects for a `relation -> target` pair.
    pub fn subjects(&self, relation: Kind, target: Kind) -> &[Kind] {
        self.incoming_by_relation
            .get(&(relation, target))
            .map(Vec::as_slice)
            .unwrap_or(&EMPTY_KINDS)
    }

    /// Return direct neighbors in deterministic order.
    pub fn neighbors(&self, kind: Kind, relation: Kind, direction: EdgeDirection) -> Vec<Kind> {
        let relation_filter = [relation];
        self.collect_neighbors_for_traversal(kind, Some(&relation_filter[..]), direction)
    }

    /// Return kinds reachable within the given depth.
    pub fn reachable(
        &self,
        kind: Kind,
        relation: Kind,
        direction: EdgeDirection,
        depth: usize,
    ) -> Vec<Kind> {
        let relation_filter = [relation];
        self.traverse_kinds(TraversalParams {
            seeds: &[kind],
            relation_filter: Some(&relation_filter[..]),
            direction,
            max_depth: depth,
            exact_depth: None,
            include_start: false,
            dedup: true,
            stop_on_match: false,
            max_results: None,
        })
    }

    /// Check whether a path exists within the given depth.
    pub fn can_reach(&self, start: Kind, relation: Kind, target: Kind, depth: usize) -> bool {
        if start == target {
            return true;
        }
        if depth == 0 {
            return false;
        }
        with_traversal_scratch(|scratch| {
            scratch.begin(self.kinds.len());
            let _ = scratch.mark_seen(self, start);
            scratch.frontier.push(start);

            let mut current_depth = 0usize;
            while current_depth < depth && !scratch.frontier.is_empty() {
                scratch.next_frontier.clear();
                let frontier_len = scratch.frontier.len();
                for index in 0..frontier_len {
                    let node = scratch.frontier[index];
                    if let Some(neighbors) = self.outgoing_by_relation.get(&(node, relation)) {
                        for neighbor in neighbors.iter().copied() {
                            if neighbor == target {
                                return true;
                            }
                            if scratch.mark_seen(self, neighbor) {
                                scratch.next_frontier.push(neighbor);
                            }
                        }
                    }
                }

                if scratch.next_frontier.is_empty() {
                    break;
                }
                std::mem::swap(&mut scratch.frontier, &mut scratch.next_frontier);
                current_depth += 1;
            }

            false
        })
    }

    /// Check whether `subject` is-a `target`.
    pub fn is_a(&self, subject: Kind, target: Kind) -> bool {
        if subject == target {
            return true;
        }

        let is_a = self.core.is_a;

        if let Some(cached) = self
            .is_a_cache
            .read()
            .expect("is_a cache poisoned")
            .get(&subject)
        {
            return cached.binary_search(&target).is_ok();
        }

        let mut ancestors = self.reachable(subject, is_a, EdgeDirection::Outgoing, usize::MAX);
        ancestors.sort_unstable();
        ancestors.dedup();

        let mut cache = self.is_a_cache.write().expect("is_a cache poisoned");
        let cached = cache.entry(subject).or_insert(ancestors);
        cached.binary_search(&target).is_ok()
    }

    /// Start an edge query builder bound to this snapshot version.
    pub fn edge_query(&self) -> EdgeQueryBuilder {
        EdgeQueryBuilder::new(Some(self.version))
    }

    /// Start a traversal query builder bound to this snapshot version.
    pub fn traversal_query(&self) -> TraversalQueryBuilder {
        TraversalQueryBuilder::new(Some(self.version))
    }

    pub(crate) fn edge_query_cache(
        &self,
    ) -> &RwLock<HashMap<EdgeQueryBuilder, Arc<CompiledEdgeQuery>>> {
        self.edge_query_cache.as_ref()
    }

    pub(crate) fn traversal_query_cache(
        &self,
    ) -> &RwLock<HashMap<TraversalQueryBuilder, Arc<CompiledTraversalQuery>>> {
        self.traversal_query_cache.as_ref()
    }

    pub(crate) fn sort_kinds_by_dense(&self, kinds: &mut [Kind]) {
        kinds.sort_unstable_by_key(|kind| self.kind_dense_index(*kind).unwrap_or(usize::MAX));
    }

    pub(crate) fn kind_dense_index(&self, kind: Kind) -> Option<usize> {
        self.kind_dense_index.get(&kind).copied()
    }

    pub(crate) fn dense_sort_key(&self, kind: Kind) -> (usize, Kind) {
        (self.kind_dense_index(kind).unwrap_or(usize::MAX), kind)
    }

    pub(crate) fn traverse_kinds(&self, params: TraversalParams<'_>) -> Vec<Kind> {
        if params.seeds.is_empty() {
            return Vec::new();
        }

        with_traversal_scratch(|scratch| {
            scratch.begin(self.kinds.len());

            for seed in params.seeds {
                if scratch.mark_seen(self, *seed) {
                    scratch.frontier.push(*seed);
                }
            }

            let mut results = Vec::new();

            if params.include_start {
                for seed in params.seeds {
                    push_traversal_result(&mut results, scratch, self, *seed, params.dedup);
                    if should_stop_after_result(
                        params.stop_on_match,
                        params.max_results,
                        results.len(),
                    ) {
                        return results;
                    }
                }
            }

            if params.max_depth == 0 {
                return results;
            }

            let mut depth = 0usize;
            while !scratch.frontier.is_empty() && depth < params.max_depth {
                depth += 1;
                scratch.next_frontier.clear();
                let frontier_len = scratch.frontier.len();

                for index in 0..frontier_len {
                    let node = scratch.frontier[index];
                    scratch.neighbor_buffer.clear();
                    self.collect_neighbors_for_traversal_into(
                        node,
                        params.relation_filter,
                        params.direction,
                        &mut scratch.neighbor_buffer,
                    );
                    self.sort_kinds_by_dense(&mut scratch.neighbor_buffer);
                    scratch.neighbor_buffer.dedup();

                    let neighbor_len = scratch.neighbor_buffer.len();
                    for neighbor_index in 0..neighbor_len {
                        let neighbor = scratch.neighbor_buffer[neighbor_index];
                        if scratch.mark_seen(self, neighbor) {
                            scratch.next_frontier.push(neighbor);
                        }

                        let depth_matches = match params.exact_depth {
                            Some(target_depth) => depth == target_depth,
                            None => true,
                        };
                        if depth_matches {
                            push_traversal_result(
                                &mut results,
                                scratch,
                                self,
                                neighbor,
                                params.dedup,
                            );
                            if should_stop_after_result(
                                params.stop_on_match,
                                params.max_results,
                                results.len(),
                            ) {
                                return results;
                            }
                        }
                    }
                }

                if scratch.next_frontier.is_empty() {
                    break;
                }
                std::mem::swap(&mut scratch.frontier, &mut scratch.next_frontier);
            }

            if let Some(max_results) = params.max_results {
                if results.len() > max_results {
                    results.truncate(max_results);
                }
            }

            results
        })
    }

    pub(crate) fn collect_neighbors_for_traversal_into(
        &self,
        kind: Kind,
        relation_filter: Option<&[Kind]>,
        direction: EdgeDirection,
        out: &mut Vec<Kind>,
    ) {
        match relation_filter {
            None => match direction {
                EdgeDirection::Outgoing => {
                    if let Some(values) = self.outgoing_all.get(&kind) {
                        out.extend_from_slice(values);
                    }
                }
                EdgeDirection::Incoming => {
                    if let Some(values) = self.incoming_all.get(&kind) {
                        out.extend_from_slice(values);
                    }
                }
                EdgeDirection::Both => {
                    if let Some(values) = self.outgoing_all.get(&kind) {
                        out.extend_from_slice(values);
                    }
                    if let Some(values) = self.incoming_all.get(&kind) {
                        out.extend_from_slice(values);
                    }
                }
            },
            Some(relations) => match direction {
                EdgeDirection::Outgoing => {
                    for relation in relations {
                        if let Some(values) = self.outgoing_by_relation.get(&(kind, *relation)) {
                            out.extend_from_slice(values);
                        }
                    }
                }
                EdgeDirection::Incoming => {
                    for relation in relations {
                        if let Some(values) = self.incoming_by_relation.get(&(*relation, kind)) {
                            out.extend_from_slice(values);
                        }
                    }
                }
                EdgeDirection::Both => {
                    for relation in relations {
                        if let Some(values) = self.outgoing_by_relation.get(&(kind, *relation)) {
                            out.extend_from_slice(values);
                        }
                        if let Some(values) = self.incoming_by_relation.get(&(*relation, kind)) {
                            out.extend_from_slice(values);
                        }
                    }
                }
            },
        }
    }

    pub(crate) fn collect_neighbors_for_traversal(
        &self,
        kind: Kind,
        relation_filter: Option<&[Kind]>,
        direction: EdgeDirection,
    ) -> Vec<Kind> {
        let mut out = Vec::new();
        self.collect_neighbors_for_traversal_into(kind, relation_filter, direction, &mut out);
        self.sort_kinds_by_dense(&mut out);
        out.dedup();
        out
    }

    fn normalize_rows(&mut self) {
        let kind_dense_index = &self.kind_dense_index;
        for row in self.outgoing_all.values_mut() {
            normalize_kind_vec(row, kind_dense_index);
        }
        for row in self.incoming_all.values_mut() {
            normalize_kind_vec(row, kind_dense_index);
        }
        for row in self.outgoing_by_relation.values_mut() {
            normalize_kind_vec(row, kind_dense_index);
        }
        for row in self.incoming_by_relation.values_mut() {
            normalize_kind_vec(row, kind_dense_index);
        }
    }
}

fn normalize_kind_vec(values: &mut Vec<Kind>, kind_dense_index: &HashMap<Kind, usize>) {
    values.sort_unstable_by_key(|kind| kind_dense_index.get(kind).copied().unwrap_or(usize::MAX));
    values.dedup();
}

#[derive(Default)]
struct TraversalScratch {
    seen_generation: u32,
    result_generation: u32,
    seen_gen: Vec<u32>,
    result_seen_gen: Vec<u32>,
    unknown_seen: HashSet<Kind>,
    unknown_result_seen: HashSet<Kind>,
    frontier: Vec<Kind>,
    next_frontier: Vec<Kind>,
    neighbor_buffer: Vec<Kind>,
}

impl TraversalScratch {
    fn begin(&mut self, kind_count: usize) {
        self.seen_generation = advance_generation(self.seen_generation, &mut self.seen_gen);
        self.result_generation =
            advance_generation(self.result_generation, &mut self.result_seen_gen);
        if self.seen_gen.len() < kind_count {
            self.seen_gen.resize(kind_count, 0);
        }
        if self.result_seen_gen.len() < kind_count {
            self.result_seen_gen.resize(kind_count, 0);
        }
        self.unknown_seen.clear();
        self.unknown_result_seen.clear();
        self.frontier.clear();
        self.next_frontier.clear();
        self.neighbor_buffer.clear();
    }

    fn mark_seen(&mut self, snapshot: &SemanticSnapshot, kind: Kind) -> bool {
        match snapshot.kind_dense_index(kind) {
            Some(index) => self.mark_seen_index(index),
            None => self.unknown_seen.insert(kind),
        }
    }

    fn mark_seen_index(&mut self, index: usize) -> bool {
        if self.seen_gen[index] == self.seen_generation {
            return false;
        }
        self.seen_gen[index] = self.seen_generation;
        true
    }

    fn mark_result_seen(&mut self, snapshot: &SemanticSnapshot, kind: Kind) -> bool {
        match snapshot.kind_dense_index(kind) {
            Some(index) => self.mark_result_seen_index(index),
            None => self.unknown_result_seen.insert(kind),
        }
    }

    fn mark_result_seen_index(&mut self, index: usize) -> bool {
        if self.result_seen_gen[index] == self.result_generation {
            return false;
        }
        self.result_seen_gen[index] = self.result_generation;
        true
    }
}

thread_local! {
    static TRAVERSAL_SCRATCH: RefCell<TraversalScratch> = RefCell::new(TraversalScratch::default());
}

fn with_traversal_scratch<T>(f: impl FnOnce(&mut TraversalScratch) -> T) -> T {
    TRAVERSAL_SCRATCH.with(|cell| {
        if let Ok(mut scratch) = cell.try_borrow_mut() {
            f(&mut scratch)
        } else {
            let mut scratch = TraversalScratch::default();
            f(&mut scratch)
        }
    })
}

fn push_traversal_result(
    results: &mut Vec<Kind>,
    scratch: &mut TraversalScratch,
    snapshot: &SemanticSnapshot,
    kind: Kind,
    dedup: bool,
) {
    if dedup {
        if scratch.mark_result_seen(snapshot, kind) {
            results.push(kind);
        }
    } else {
        results.push(kind);
    }
}

fn should_stop_after_result(
    stop_on_match: bool,
    max_results: Option<usize>,
    result_len: usize,
) -> bool {
    if stop_on_match && result_len > 0 {
        return true;
    }
    if let Some(max_results) = max_results {
        return result_len >= max_results;
    }
    false
}

fn advance_generation(generation: u32, seen: &mut [u32]) -> u32 {
    let next = generation.wrapping_add(1);
    if next == 0 {
        seen.fill(0);
        1
    } else {
        next
    }
}
