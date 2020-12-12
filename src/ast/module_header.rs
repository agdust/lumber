use super::*;
use std::collections::{HashMap, HashSet};

/// Lists the predicates and exports of the module, but does not bind them to any
/// actual definitions.
#[derive(Clone, Debug)]
pub(crate) struct ModuleHeader {
    /// The path to this module.
    pub scope: Scope,
    /// Modules from which imports are globbed.
    pub globs: HashSet<Scope>,
    /// Native functions bound to this module.
    pub natives: HashSet<Handle>,
    /// Publicly available predicates.
    pub exports: HashSet<Handle>,
    /// Predicates that are modifyable at runtime.
    pub mutables: HashSet<Handle>,
    /// Predicates which are not completely defined in this module.
    pub incompletes: HashSet<Handle>,
    /// All (private and public) predicates.
    pub definitions: HashSet<Handle>,
    /// Imported predicates and their alises.
    pub aliases: HashMap<Handle, Handle>,
    /// Operators which have been imported from other modules. An operator can be imported
    /// from many modules, and a conflict will be detected later if both define the same
    /// operator that is used.
    pub operator_aliases: HashMap<Atom, Vec<Scope>>,
    /// Operators defined in this module.
    pub operators: HashMap<OpKey, Operator>,
}

macro_rules! add_lib {
    ($to:expr, $lib:expr) => {
        $to = $to
            .into_iter()
            .map(|mut item| {
                item.add_lib($lib.clone());
                item
            })
            .collect()
    };
}

impl ModuleHeader {
    pub fn new(scope: Scope) -> Self {
        Self {
            scope,
            globs: Default::default(),
            natives: Default::default(),
            exports: Default::default(),
            mutables: Default::default(),
            incompletes: Default::default(),
            definitions: Default::default(),
            aliases: Default::default(),
            operator_aliases: Default::default(),
            operators: Default::default(),
        }
    }

    pub fn into_library(mut self, lib: Atom) -> Self {
        self.scope.add_lib(lib.clone());
        add_lib!(self.globs, lib);
        add_lib!(self.natives, lib);
        add_lib!(self.exports, lib);
        add_lib!(self.mutables, lib);
        add_lib!(self.incompletes, lib);
        add_lib!(self.definitions, lib);
        self.aliases = self
            .aliases
            .into_iter()
            .map(|(mut key, mut value)| {
                key.add_lib(lib.clone());
                value.add_lib(lib.clone());
                (key, value)
            })
            .collect();
        self.operators.iter_mut().for_each(|(_, op)| {
            op.add_lib(lib.clone());
        });
        self.operator_aliases
            .iter_mut()
            .flat_map(|(_, scopes)| scopes.iter_mut())
            .for_each(|scope| {
                scope.add_lib(lib.clone());
            });
        self
    }

    pub fn insert_glob(&mut self, module: Scope) -> Option<Scope> {
        self.globs.replace(module)
    }

    pub fn insert_public(&mut self, handle: Handle) -> Option<Handle> {
        self.exports.replace(handle)
    }

    pub fn insert_native(&mut self, handle: Handle) -> Option<Handle> {
        self.natives.replace(handle)
    }

    pub fn insert_mutable(&mut self, handle: Handle) -> Option<Handle> {
        self.definitions.insert(handle.clone());
        self.mutables.replace(handle)
    }

    pub fn insert_operator(&mut self, operator: Operator) -> Option<Operator> {
        self.operators.insert(operator.key(), operator)
    }

    pub fn insert_incomplete(&mut self, handle: Handle) -> (Option<Handle>, Option<Handle>) {
        self.definitions.insert(handle.clone());
        (
            self.exports.replace(handle.clone()),
            self.incompletes.replace(handle),
        )
    }

    pub fn insert(&mut self, handle: Handle) {
        self.definitions.insert(handle);
    }

    pub fn insert_alias(&mut self, alias: Handle, source: Handle) -> Option<(Handle, Handle)> {
        self.aliases
            .insert(alias.clone(), source)
            .map(|source| (alias, source))
    }

    pub fn insert_operator_alias(&mut self, operator: Atom, scope: Scope) -> Option<Scope> {
        let scopes = self
            .operator_aliases
            .entry(operator)
            .or_insert_with(Default::default);
        if scopes.contains(&scope) {
            return Some(scope);
        }
        scopes.push(scope);
        None
    }

    pub fn resolve<'a>(
        &'a self,
        handle: &'a Handle,
        from_scope: &Scope,
        context: &'a Context,
    ) -> crate::Result<&'a Handle> {
        match self.resolve_inner(handle, from_scope, context, &mut vec![])? {
            None => Err(crate::Error::parse(&format!(
                "Unresolved predicate {} in scope {}.",
                handle, from_scope
            ))),
            Some(resolved) => Ok(resolved),
        }
    }

    fn resolve_inner<'a: 'b, 'b>(
        &'a self,
        handle: &'b Handle,
        from_scope: &Scope,
        context: &'a Context,
        path: &mut Vec<&'b Handle>,
    ) -> crate::Result<Option<&'a Handle>> {
        if path.contains(&handle) {
            path.push(handle);
            return Err(crate::Error::parse(&format!(
                "Alias loop detected: {}",
                path.iter_mut()
                    .map(|handle| handle.to_string())
                    .collect::<Vec<_>>()
                    .join(" -> "),
            )));
        }
        path.push(handle);
        let resolved = if let Some(resolved) = self.definitions.get(handle) {
            resolved
        } else if let Some(resolved) = self.natives.get(handle) {
            resolved
        } else if let Some(alias) = self.aliases.get(handle) {
            if let Some(lib) = alias.library().first() {
                let lib = context.libraries.get(&lib).unwrap();
                let handle = lib.resolve(alias, true);
                return Ok(handle);
            }
            match context
                .modules
                .get(&alias.module())
                .unwrap()
                .resolve_inner(alias, &self.scope, context, path)?
            {
                Some(resolved) => resolved,
                None => return Ok(None),
            }
        } else {
            let candidates = self
                .globbed_modules()
                .map(|scope| context.modules.get(scope).unwrap())
                .filter_map(|module| {
                    module
                        .resolve_like(handle, from_scope, context, path)
                        .transpose()
                })
                .collect::<crate::Result<HashSet<_>>>()?;

            if candidates.len() == 1 {
                candidates.into_iter().next().unwrap()
            } else if candidates.is_empty() {
                return Ok(None);
            } else {
                return Err(crate::Error::parse(&format!(
                    "Ambiguous reference {}. Could be referring to any of:\n{}",
                    handle,
                    candidates
                        .iter()
                        .map(|candidate| format!("\t{}", candidate))
                        .collect::<Vec<_>>()
                        .join("\n"),
                )));
            }
        };

        if self.scope >= *from_scope || self.exports.contains(handle) {
            Ok(Some(resolved))
        } else {
            Err(crate::Error::parse(&format!(
                "Predicate {} is not visible from scope {}.",
                handle, from_scope
            )))
        }
    }

    fn resolve_like<'a, 'b>(
        &'a self,
        handle: &Handle,
        from_scope: &Scope,
        context: &'a Context,
        path: &[&'b Handle],
    ) -> crate::Result<Option<&'a Handle>> {
        let handle = handle.relocate(&self.scope);
        self.resolve_inner(&handle, from_scope, context, &mut path.to_vec())
    }

    pub fn globbed_modules(&self) -> impl Iterator<Item = &Scope> {
        self.globs.iter()
    }

    pub fn errors(&self, context: &Context, native_handles: &[&Handle]) -> Vec<crate::Error> {
        let mut errors = vec![];
        for module in &self.globs {
            if let Some(lib) = module.library().first() {
                if !context.libraries.contains_key(lib) {
                    errors.push(crate::Error::parse(&format!(
                        "Referencing unlinked library {} in glob import {}.",
                        lib, module,
                    )));
                }
            } else {
                if !context.modules.contains_key(module) {
                    errors.push(crate::Error::parse(&format!(
                        "Unresolved module {} in glob import.",
                        module,
                    )));
                }
            }
        }
        for native in &self.natives {
            if !native_handles.contains(&native) {
                errors.push(crate::Error::parse(&format!(
                    "Native function {} is not bound.",
                    native,
                )));
            }
            if self.definitions.contains(native) {
                errors.push(crate::Error::parse(&format!(
                    "Native function {} cannot also be implemented.",
                    native,
                )));
            } else if self.aliases.contains_key(native) {
                errors.push(crate::Error::parse(&format!(
                    "Native function {} cannot also be imported.",
                    native,
                )));
            } else if self.mutables.contains(native) {
                errors.push(crate::Error::parse(&format!(
                    "Native function {} cannot be set as mutable.",
                    native,
                )));
            } else if self.incompletes.contains(native) {
                errors.push(crate::Error::parse(&format!(
                    "Native function {} cannot be set as incomplete.",
                    native,
                )));
            }
        }
        for export in &self.exports {
            if self.resolve(export, &self.scope, context).is_err() {
                errors.push(crate::Error::parse(&format!(
                    "Exported predicate {} cannot be found.",
                    export.head(),
                )));
            }
        }
        for mutable in &self.mutables {
            if self.aliases.contains_key(mutable) {
                errors.push(crate::Error::parse(&format!(
                    "Cannot set alias {} as mutable.",
                    mutable.head(),
                )));
            }
        }
        for incomplete in &self.incompletes {
            if self.aliases.contains_key(incomplete) {
                errors.push(crate::Error::parse(&format!(
                    "Cannot set alias {} as incomplete.",
                    incomplete.head(),
                )));
            }
        }
        for definition in &self.definitions {
            if let Some((key, value)) = self.aliases.get_key_value(definition) {
                let incomplete = context
                    .modules
                    .get(&value.module())
                    .map(|module| module.incompletes.contains(value))
                    .unwrap_or(false);
                if !incomplete {
                    let mut message = format!(
                        "Definition of {} conflicts with imported {}",
                        definition.head(),
                        value,
                    );
                    if !key.like(value) {
                        message.push_str(&format!(" (aliased as {})", key.head()));
                    }
                    message.push('.');
                    errors.push(crate::Error::parse(&message));
                }
            }
        }
        let mut reported: HashSet<Handle> = HashSet::new();
        for alias in self.aliases.values() {
            if reported.contains(alias) {
                continue;
            }
            match alias.library().first() {
                Some(library) => match context.libraries.get(&library) {
                    None => {
                        errors.push(crate::Error::parse(&format!(
                            "Referencing predicate {} from unlinked library {}.",
                            alias, library,
                        )));
                    }
                    Some(lib) if lib.exports(alias) => continue,
                    Some(..) => {
                        errors.push(crate::Error::parse(&format!(
                            "No predicate {} is exported by the library {}.",
                            alias, library,
                        )));
                    }
                },
                None => {
                    match context.modules.get(&alias.module()).unwrap().resolve(
                        alias,
                        &self.scope,
                        context,
                    ) {
                        Ok(..) => {}
                        Err(error) => errors.push(error),
                    }
                    let aliases = self
                        .aliases
                        .iter()
                        .filter(|&(_, value)| alias == value)
                        .map(|(key, _)| key)
                        .collect::<Vec<_>>();
                    if aliases.len() != 1 {
                        reported.insert(alias.clone());
                        errors.push(crate::Error::parse(&format!(
                            "{} is aliased multiple times, as:\n\t{}",
                            alias,
                            aliases
                                .into_iter()
                                .map(|alias| format!("\t{}", alias))
                                .collect::<Vec<_>>()
                                .join("\n"),
                        )));
                    }
                }
            }
        }
        errors
    }
}
