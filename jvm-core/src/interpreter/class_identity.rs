use std::collections::HashMap;

/// VM-internal identity for a defining or initiating class loader.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct LoaderId(u64);

impl LoaderId {
    pub(crate) const BOOTSTRAP: Self = Self(0);
    pub(crate) const SYSTEM: Self = Self(1);

    pub(crate) fn new(id: u64) -> Self {
        Self(id)
    }
}

/// VM-internal handle for a class identity record.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct ClassId(u64);

impl ClassId {
    fn new(id: u64) -> Self {
        Self(id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClassRecord {
    pub(crate) internal_name: String,
    pub(crate) binary_name: String,
    pub(crate) defining_loader: LoaderId,
}

#[derive(Debug)]
pub(crate) struct ClassIdentityRegistry {
    defined_classes: HashMap<(LoaderId, String), ClassId>,
    class_records: HashMap<ClassId, ClassRecord>,
    initiating_classes: HashMap<(LoaderId, String), ClassId>,
    next_class_id: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DefineClassError {
    Duplicate(ClassId),
}

impl ClassIdentityRegistry {
    pub(crate) fn new() -> Self {
        Self {
            defined_classes: HashMap::new(),
            class_records: HashMap::new(),
            initiating_classes: HashMap::new(),
            next_class_id: 0,
        }
    }

    /// Register class identity for normal load paths.
    ///
    /// This is intentionally idempotent: repeated loads of the same binary name
    /// by the same defining loader must reuse the existing ClassId. Use
    /// `try_register_defined_class` for `ClassLoader#defineClass`, where a
    /// duplicate definition is a Java-visible LinkageError.
    pub(crate) fn register_defined_class(
        &mut self,
        defining_loader: LoaderId,
        internal_name: impl Into<String>,
    ) -> ClassId {
        let internal_name = internal_name.into();
        let key = (defining_loader, internal_name.clone());
        if let Some(class_id) = self.defined_classes.get(&key) {
            return *class_id;
        }

        let class_id = ClassId::new(self.next_class_id);
        self.next_class_id += 1;
        let record = ClassRecord {
            binary_name: binary_name_from_internal_name(&internal_name),
            internal_name,
            defining_loader,
        };
        self.defined_classes.insert(key, class_id);
        self.class_records.insert(class_id, record);
        class_id
    }

    pub(crate) fn try_register_defined_class(
        &mut self,
        defining_loader: LoaderId,
        internal_name: impl Into<String>,
    ) -> Result<ClassId, DefineClassError> {
        let internal_name = internal_name.into();
        let key = (defining_loader, internal_name.clone());
        if let Some(class_id) = self.defined_classes.get(&key) {
            return Err(DefineClassError::Duplicate(*class_id));
        }

        let class_id = ClassId::new(self.next_class_id);
        self.next_class_id += 1;
        let record = ClassRecord {
            binary_name: binary_name_from_internal_name(&internal_name),
            internal_name,
            defining_loader,
        };
        self.defined_classes.insert(key, class_id);
        self.class_records.insert(class_id, record);
        Ok(class_id)
    }

    pub(crate) fn record_initiating_loader(
        &mut self,
        initiating_loader: LoaderId,
        lookup_name: impl Into<String>,
        class_id: ClassId,
    ) {
        self.initiating_classes
            .insert((initiating_loader, lookup_name.into()), class_id);
    }

    pub(crate) fn class_id_for_defined(
        &self,
        defining_loader: LoaderId,
        internal_name: &str,
    ) -> Option<ClassId> {
        self.defined_classes
            .get(&(defining_loader, internal_name.to_owned()))
            .copied()
    }

    pub(crate) fn class_id_for_initiating(
        &self,
        initiating_loader: LoaderId,
        lookup_name: &str,
    ) -> Option<ClassId> {
        self.initiating_classes
            .get(&(initiating_loader, lookup_name.to_owned()))
            .copied()
    }

    pub(crate) fn class_record(&self, class_id: ClassId) -> Option<&ClassRecord> {
        self.class_records.get(&class_id)
    }
}

fn binary_name_from_internal_name(internal_name: &str) -> String {
    internal_name.replace('/', ".")
}

#[cfg(test)]
mod tests {
    use super::{ClassIdentityRegistry, LoaderId};

    #[test]
    fn same_name_and_defining_loader_reuses_class_id() {
        let mut registry = ClassIdentityRegistry::new();

        let first = registry.register_defined_class(LoaderId::BOOTSTRAP, "pkg/Thing");
        let second = registry.register_defined_class(LoaderId::BOOTSTRAP, "pkg/Thing");

        assert_eq!(first, second);
    }

    #[test]
    fn same_name_under_distinct_defining_loaders_gets_distinct_class_ids() {
        let mut registry = ClassIdentityRegistry::new();

        let bootstrap = registry.register_defined_class(LoaderId::BOOTSTRAP, "pkg/Thing");
        let system = registry.register_defined_class(LoaderId::SYSTEM, "pkg/Thing");

        assert_ne!(bootstrap, system);
        assert_ne!(LoaderId::BOOTSTRAP, LoaderId::SYSTEM);
        assert_eq!(
            registry.class_id_for_defined(LoaderId::BOOTSTRAP, "pkg/Thing"),
            Some(bootstrap),
        );
        assert_eq!(
            registry.class_id_for_defined(LoaderId::SYSTEM, "pkg/Thing"),
            Some(system),
        );
    }

    #[test]
    fn initiating_loader_record_does_not_change_defining_loader_identity() {
        let mut registry = ClassIdentityRegistry::new();

        let class_id = registry.register_defined_class(LoaderId::BOOTSTRAP, "pkg/Thing");
        registry.record_initiating_loader(LoaderId::SYSTEM, "pkg/Thing", class_id);

        assert_eq!(
            registry.class_id_for_initiating(LoaderId::SYSTEM, "pkg/Thing"),
            Some(class_id),
        );
        assert_eq!(
            registry
                .class_record(class_id)
                .map(|record| record.defining_loader),
            Some(LoaderId::BOOTSTRAP),
        );
    }

    #[test]
    fn duplicate_define_reports_existing_class_id() {
        let mut registry = ClassIdentityRegistry::new();

        let first = registry
            .try_register_defined_class(LoaderId::BOOTSTRAP, "pkg/Thing")
            .expect("first definition");
        let duplicate = registry.try_register_defined_class(LoaderId::BOOTSTRAP, "pkg/Thing");

        assert!(matches!(
            duplicate,
            Err(super::DefineClassError::Duplicate(existing)) if existing == first
        ));
    }
}
