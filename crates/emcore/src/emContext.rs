use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

/// Key for the model registry: (concrete type, name).
///
/// In C++ Eagle Mode, models are identified by `(typeid(FinalClass), name)`.
/// We mirror this with `(TypeId, String)`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ModelKey {
    type_id: TypeId,
    name: String,
}

/// Entry stored in the model registry for a registered (common) model.
struct ModelEntry {
    /// The model itself, type-erased. Downcast via `Rc::downcast::<RefCell<T>>`.
    model: Rc<dyn Any>,
    /// Minimum seconds the context keeps this model alive after external refs drop.
    /// `u32::MAX` means infinite (lives as long as the context).
    min_common_lifetime: u32,
}

/// A tree node for service/singleton lookup.
///
/// Models can be *registered* (common) so they are discoverable by
/// `(TypeId, name)`. Unregistered (private) models are not stored here.
///
/// Typed singletons (e.g. `emCoreConfig`) are added as
/// `RefCell<Option<Rc<T>>>` fields with getter methods that walk the parent
/// chain (inherited lookup). Dynamic resources use `ResourceCache<V>` stored
/// as typed singletons.
///
/// Children are stored as `Weak` references to avoid memory leaks.
/// The child `Rc` is owned by whoever created it (typically a emView or Panel).
///
/// DIVERGED (Phase-3 Task-2): the C++ `emRef<emClipboard>
/// emClipboard::LookupInherited(emContext&)` mirror used to live here.
/// It has been relocated to the framework per spec §3.4 / §3.6(a) so
/// winit text-event callbacks can access it without `&mut framework`
/// reach. Access via `EngineCtx`/`SchedCtx` methods.
pub struct emContext {
    parent: Option<Weak<emContext>>,
    children: RefCell<Vec<Weak<emContext>>>,
    /// Registry of common (named) models, keyed by `(TypeId, name)`.
    registry: RefCell<HashMap<ModelKey, ModelEntry>>,
}

impl emContext {
    pub fn NewRoot() -> Rc<Self> {
        Rc::new(Self {
            parent: None,
            children: RefCell::new(Vec::new()),
            registry: RefCell::new(HashMap::new()),
        })
    }

    pub fn NewChild(parent: &Rc<emContext>) -> Rc<Self> {
        let child = Rc::new(Self {
            parent: Some(Rc::downgrade(parent)),
            children: RefCell::new(Vec::new()),
            registry: RefCell::new(HashMap::new()),
        });
        parent.children.borrow_mut().push(Rc::downgrade(&child));
        child
    }

    pub fn GetParentContext(&self) -> Option<Rc<emContext>> {
        self.parent.as_ref().and_then(|w| w.upgrade())
    }

    /// Walk the parent chain and return the root context.
    ///
    /// A root context (no parent) returns a clone of its own `Rc`.
    /// Port of C++ `emContext::GetRootContext`.
    pub fn GetRootContext(self: &Rc<emContext>) -> Rc<emContext> {
        let mut cur = Rc::clone(self);
        while let Some(parent) = cur.GetParentContext() {
            cur = parent;
        }
        cur
    }

    /// Borrow the raw children list (weak refs to live + dead child contexts).
    /// Callers iterate and `Weak::upgrade` to filter live entries.
    ///
    /// Used by the tree-dump cascade (`emTreeDump::dump_context_with_cascade`)
    /// which needs to walk the full child set, not just count.
    pub(crate) fn children(&self) -> std::cell::Ref<'_, Vec<Weak<emContext>>> {
        self.children.borrow()
    }

    /// Number of live children (expired weak references are not counted).
    pub fn child_count(&self) -> usize {
        self.children
            .borrow()
            .iter()
            .filter(|w| w.strong_count() > 0)
            .count()
    }

    /// Purge expired weak references from the children list.
    pub fn purge_dead_children(&self) {
        self.children.borrow_mut().retain(|w| w.strong_count() > 0);
    }

    // ------------------------------------------------------------------
    // Named model registry — port of C++ emContext::RegisterModel et al.
    // ------------------------------------------------------------------

    /// Register a common model under `(TypeId::of::<T>(), name)`.
    ///
    /// Port of C++ `emContext::RegisterModel`. The model is stored type-erased
    /// as `Rc<RefCell<T>>` behind `Rc<dyn Any>`, so callers can later
    /// downcast it back.
    ///
    /// # Panics
    ///
    /// Panics if a model with the same type and name is already registered
    /// (mirrors the C++ `emFatalError` on duplicate identity).
    pub fn register_model<T: 'static>(&self, name: &str, model: Rc<RefCell<T>>) {
        let key = ModelKey {
            type_id: TypeId::of::<T>(),
            name: name.to_string(),
        };
        let mut reg = self.registry.borrow_mut();
        if reg.contains_key(&key) {
            panic!(
                "Context::register_model: duplicate common model: type={}, name=\"{}\"",
                std::any::type_name::<T>(),
                name,
            );
        }
        reg.insert(
            key,
            ModelEntry {
                model: model as Rc<dyn Any>,
                min_common_lifetime: 0,
            },
        );
    }

    /// Unregister a common model. No-op if the model is not registered.
    ///
    /// Port of C++ `emContext::UnregisterModel`.
    pub fn unregister_model<T: 'static>(&self, name: &str) {
        let key = ModelKey {
            type_id: TypeId::of::<T>(),
            name: name.to_string(),
        };
        self.registry.borrow_mut().remove(&key);
    }

    /// Check whether a model with the given type and name is registered.
    ///
    /// Port of C++ `emModel::IsRegistered` / `IsCommon`.
    pub fn is_registered<T: 'static>(&self, name: &str) -> bool {
        let key = ModelKey {
            type_id: TypeId::of::<T>(),
            name: name.to_string(),
        };
        self.registry.borrow().contains_key(&key)
    }

    /// emLook up a registered model by type and name in this context only.
    ///
    /// Port of C++ `emContext::Lookup(typeid(T), name)`.
    /// Returns `None` if not found.
    pub fn Lookup<T: 'static>(&self, name: &str) -> Option<Rc<RefCell<T>>> {
        let key = ModelKey {
            type_id: TypeId::of::<T>(),
            name: name.to_string(),
        };
        let reg = self.registry.borrow();
        reg.get(&key).map(|entry| {
            Rc::clone(&entry.model)
                .downcast::<RefCell<T>>()
                .expect("Context::lookup: type mismatch in registry (should be impossible)")
        })
    }

    /// emLook up a registered model by walking up the parent chain.
    ///
    /// Port of C++ `emContext::LookupInherited`.
    pub fn LookupInherited<T: 'static>(&self, name: &str) -> Option<Rc<RefCell<T>>> {
        if let Some(m) = self.Lookup::<T>(name) {
            return Some(m);
        }
        if let Some(parent) = self.GetParentContext() {
            return parent.LookupInherited::<T>(name);
        }
        None
    }

    /// emLook up a registered model, or create and register it if absent.
    ///
    /// Port of C++ `EM_IMPL_ACQUIRE_COMMON` macro. The `create` closure is
    /// called only when the model is not already registered.
    pub fn acquire<T: 'static>(&self, name: &str, create: impl FnOnce() -> T) -> Rc<RefCell<T>> {
        if let Some(existing) = self.Lookup::<T>(name) {
            return existing;
        }
        let model = Rc::new(RefCell::new(create()));
        self.register_model::<T>(name, Rc::clone(&model));
        model
    }

    /// Get the minimum common lifetime (seconds) for a registered model.
    ///
    /// Returns `None` if the model is not registered.
    pub fn get_min_common_lifetime<T: 'static>(&self, name: &str) -> Option<u32> {
        let key = ModelKey {
            type_id: TypeId::of::<T>(),
            name: name.to_string(),
        };
        self.registry
            .borrow()
            .get(&key)
            .map(|e| e.min_common_lifetime)
    }

    /// Set the minimum common lifetime (seconds) for a registered model.
    ///
    /// Port of C++ `emModel::SetMinCommonLifetime`. `u32::MAX` means infinite
    /// (lives as long as the context). No-op if the model is not registered.
    pub fn set_min_common_lifetime<T: 'static>(&self, name: &str, seconds: u32) {
        let key = ModelKey {
            type_id: TypeId::of::<T>(),
            name: name.to_string(),
        };
        if let Some(entry) = self.registry.borrow_mut().get_mut(&key) {
            entry.min_common_lifetime = seconds;
        }
    }

    /// Return the number of registered (common) models in this context.
    pub fn common_model_count(&self) -> usize {
        self.registry.borrow().len()
    }

    /// Return a list of `(type_name, model_name)` for all registered models.
    /// Intended for debugging, similar to C++ `emContext::GetListing`.
    pub fn GetListing(&self) -> Vec<(String, String)> {
        self.registry
            .borrow()
            .keys()
            .map(|k| (format!("{:?}", k.type_id), k.name.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_lookup() {
        let ctx = emContext::NewRoot();
        let model = Rc::new(RefCell::new(42_i32));
        ctx.register_model::<i32>("answer", Rc::clone(&model));

        assert!(ctx.is_registered::<i32>("answer"));
        assert!(!ctx.is_registered::<i32>("other"));
        assert!(!ctx.is_registered::<u32>("answer"));

        let found = ctx.Lookup::<i32>("answer").expect("should find model");
        assert_eq!(*found.borrow(), 42);
    }

    #[test]
    fn unregister() {
        let ctx = emContext::NewRoot();
        ctx.register_model::<i32>("x", Rc::new(RefCell::new(1)));
        assert!(ctx.is_registered::<i32>("x"));
        ctx.unregister_model::<i32>("x");
        assert!(!ctx.is_registered::<i32>("x"));
        assert!(ctx.Lookup::<i32>("x").is_none());
    }

    #[test]
    #[should_panic(expected = "duplicate common model")]
    fn duplicate_registration_panics() {
        let ctx = emContext::NewRoot();
        ctx.register_model::<i32>("dup", Rc::new(RefCell::new(1)));
        ctx.register_model::<i32>("dup", Rc::new(RefCell::new(2)));
    }

    #[test]
    fn lookup_inherited_walks_parents() {
        let root = emContext::NewRoot();
        root.register_model::<String>("greeting", Rc::new(RefCell::new("hello".to_string())));

        let child = emContext::NewChild(&root);
        // Not in child, but found via parent.
        let found = child
            .LookupInherited::<String>("greeting")
            .expect("inherited lookup");
        assert_eq!(*found.borrow(), "hello");

        // Direct lookup in child should fail.
        assert!(child.Lookup::<String>("greeting").is_none());
    }

    #[test]
    fn acquire_creates_or_returns_existing() {
        let ctx = emContext::NewRoot();
        let m1 = ctx.acquire::<Vec<u8>>("buf", Vec::new);
        m1.borrow_mut().push(42);

        let m2 = ctx.acquire::<Vec<u8>>("buf", Vec::new);
        assert_eq!(m2.borrow().len(), 1, "should return the same model");
        assert!(Rc::ptr_eq(&m1, &m2));
    }

    #[test]
    fn min_common_lifetime() {
        let ctx = emContext::NewRoot();
        ctx.register_model::<i32>("lt", Rc::new(RefCell::new(0)));
        assert_eq!(ctx.get_min_common_lifetime::<i32>("lt"), Some(0));
        ctx.set_min_common_lifetime::<i32>("lt", 300);
        assert_eq!(ctx.get_min_common_lifetime::<i32>("lt"), Some(300));
    }

    #[test]
    fn different_types_same_name() {
        let ctx = emContext::NewRoot();
        ctx.register_model::<i32>("val", Rc::new(RefCell::new(1_i32)));
        ctx.register_model::<u32>("val", Rc::new(RefCell::new(2_u32)));

        assert_eq!(*ctx.Lookup::<i32>("val").unwrap().borrow(), 1);
        assert_eq!(*ctx.Lookup::<u32>("val").unwrap().borrow(), 2);
    }

    #[test]
    fn common_model_count_and_listing() {
        let ctx = emContext::NewRoot();
        assert_eq!(ctx.common_model_count(), 0);
        ctx.register_model::<i32>("a", Rc::new(RefCell::new(1)));
        ctx.register_model::<u32>("b", Rc::new(RefCell::new(2)));
        assert_eq!(ctx.common_model_count(), 2);
        assert_eq!(ctx.GetListing().len(), 2);
    }

    #[test]
    fn get_root_context_returns_root_from_deep_child() {
        let root = emContext::NewRoot();
        let child = emContext::NewChild(&root);
        let grandchild = emContext::NewChild(&child);

        assert!(Rc::ptr_eq(&root.GetRootContext(), &root));
        assert!(Rc::ptr_eq(&child.GetRootContext(), &root));
        assert!(Rc::ptr_eq(&grandchild.GetRootContext(), &root));
    }
}
