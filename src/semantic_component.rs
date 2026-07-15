//! Static component identities and world-local Bevy component bindings.

use core::any::type_name;

use bevy_app::App;
use bevy_ecs::component::{Component, ComponentId};
use bevy_ecs::prelude::{Resource, World};
use bevy_platform::collections::HashMap;

use crate::plugin::Semantics;
use crate::registry::hash_canonical_name;
use crate::{Kind, SemanticError};

/// A Bevy component with an explicit, stable semantic identity.
///
/// `KIND_NAME` is the canonical semantic name and `KIND` must equal
/// `kind!(KIND_NAME)`. Implementations should normally be generated with
/// `#[derive(SemanticComponent)]` or [`crate::semantic_component!`].
pub trait SemanticComponent: Component {
    const KIND: Kind;
    const KIND_NAME: &'static str;
}

/// World-local bindings between stable semantic kinds and Bevy component IDs.
///
/// A binding remains for the lifetime of the world because Bevy component
/// registrations themselves cannot be unregistered.
#[derive(Resource, Debug, Default)]
pub struct SemanticComponents {
    by_kind: HashMap<Kind, ComponentId>,
    by_component: Vec<Option<Kind>>,
}

impl SemanticComponents {
    #[inline]
    pub fn component_id(&self, kind: Kind) -> Option<ComponentId> {
        self.by_kind.get(&kind).copied()
    }

    #[inline]
    pub fn kind(&self, component_id: ComponentId) -> Option<Kind> {
        self.by_component
            .get(component_id.index())
            .copied()
            .flatten()
    }

    #[inline]
    pub fn component_id_of<T>(&self) -> Option<ComponentId>
    where
        T: SemanticComponent,
    {
        self.component_id(T::KIND)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.by_kind.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.by_kind.is_empty()
    }

    fn validate(&self, kind: Kind, component_id: ComponentId) -> Result<(), SemanticError> {
        if let Some(existing) = self.component_id(kind) {
            if existing != component_id {
                return Err(SemanticError::KindComponentConflict {
                    kind,
                    existing_component: existing.index(),
                    requested_component: component_id.index(),
                });
            }
        }
        if let Some(existing) = self.kind(component_id) {
            if existing != kind {
                return Err(SemanticError::ComponentKindConflict {
                    component: component_id.index(),
                    existing_kind: existing,
                    requested_kind: kind,
                });
            }
        }
        Ok(())
    }

    fn bind(&mut self, kind: Kind, component_id: ComponentId) {
        if self.component_id(kind) == Some(component_id) {
            return;
        }
        self.by_kind.insert(kind, component_id);
        if self.by_component.len() <= component_id.index() {
            self.by_component.resize(component_id.index() + 1, None);
        }
        self.by_component[component_id.index()] = Some(kind);
    }
}

/// Fallible semantic component registration directly on a Bevy world.
pub trait SemanticWorldExt {
    fn try_register_semantic_component<T>(&mut self) -> Result<ComponentId, SemanticError>
    where
        T: SemanticComponent;
}

impl SemanticWorldExt for World {
    fn try_register_semantic_component<T>(&mut self) -> Result<ComponentId, SemanticError>
    where
        T: SemanticComponent,
    {
        let generated = hash_canonical_name(T::KIND_NAME.trim());
        if T::KIND_NAME.trim().is_empty() || generated != T::KIND {
            return Err(SemanticError::StaticKindMismatch {
                type_name: type_name::<T>(),
                name: T::KIND_NAME,
                declared: T::KIND,
                generated,
            });
        }

        self.init_resource::<Semantics>();
        self.init_resource::<SemanticComponents>();
        let component_id = self.register_component::<T>();

        {
            let mappings = self.resource::<SemanticComponents>();
            mappings.validate(T::KIND, component_id)?;
            if mappings.component_id(T::KIND) == Some(component_id) {
                let semantics = self.resource::<Semantics>();
                if semantics.has_static_component_binding::<T>(T::KIND_NAME, T::KIND) {
                    return Ok(component_id);
                }
            }
        }

        self.resource_mut::<Semantics>()
            .bind_static_component::<T>(T::KIND_NAME, T::KIND)?;
        self.resource_mut::<SemanticComponents>()
            .bind(T::KIND, component_id);
        Ok(component_id)
    }
}

/// Infallible application-builder registration for prewarming component bindings.
pub trait SemanticAppExt {
    fn register_semantic_component<T>(&mut self) -> &mut Self
    where
        T: SemanticComponent;
}

impl SemanticAppExt for App {
    fn register_semantic_component<T>(&mut self) -> &mut Self
    where
        T: SemanticComponent,
    {
        self.world_mut()
            .try_register_semantic_component::<T>()
            .unwrap_or_else(|error| {
                panic!(
                    "failed to register semantic component {}: {error}",
                    type_name::<T>()
                )
            });
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Component)]
    struct First;

    #[derive(Component)]
    struct Second;

    #[test]
    fn binding_is_strict_and_idempotent() {
        let mut mappings = SemanticComponents::default();
        let first = ComponentId::new(3);
        let second = ComponentId::new(7);
        let first_kind = crate::kind!("First");
        let second_kind = crate::kind!("Second");

        mappings.validate(first_kind, first).unwrap();
        mappings.bind(first_kind, first);
        mappings.validate(first_kind, first).unwrap();
        mappings.bind(first_kind, first);
        assert_eq!(mappings.len(), 1);

        assert!(matches!(
            mappings.validate(first_kind, second),
            Err(SemanticError::KindComponentConflict { .. })
        ));
        assert!(matches!(
            mappings.validate(second_kind, first),
            Err(SemanticError::ComponentKindConflict { .. })
        ));

        let _ = (First, Second);
    }
}
