use super::evaltree::*;
use super::*;
use crate::ast::ModuleHeader;
use std::collections::HashMap;

mod definition;
mod entry;

pub(crate) use definition::DatabaseDefinition;
use entry::DatabaseEntry;

#[derive(Clone, Default, Debug)]
pub(crate) struct Database<'p> {
    /// All currently active definitions in this program. They may not be the same as they
    /// were when the program was created, due to mutable definitions.
    pub(super) definitions: HashMap<Handle, DatabaseEntry<'p>>,
    pub(super) operators: HashMap<Scope, HashMap<OpKey, Operator>>,
}

impl<'p> Database<'p> {
    pub fn new<D, O>(definitions: D, operators: O) -> Self
    where
        D: IntoIterator<Item = (crate::ast::Handle, crate::ast::Definition)>,
        O: IntoIterator<
            Item = (
                crate::ast::Scope,
                HashMap<crate::ast::OpKey, crate::ast::Operator>,
            ),
        >,
    {
        let definitions = definitions
            .into_iter()
            .fold(
                HashMap::<Handle, Vec<Definition>>::default(),
                |mut handles, (handle, entry)| {
                    handles
                        .entry(Handle::from(handle))
                        .or_default()
                        .push(Definition::from(entry));
                    handles
                },
            )
            .into_iter()
            .map(|(handle, definition)| {
                (
                    handle,
                    DatabaseEntry::new(DatabaseDefinition::Static(
                        definition.into_iter().map(Definition::from).collect(),
                    )),
                )
            })
            .collect();
        let operators = operators
            .into_iter()
            .map(|(key, op)| (key.into(), op.into()))
            .collect();
        Self {
            definitions,
            operators,
        }
    }

    pub fn apply_header(
        &mut self,
        header: &ModuleHeader,
        natives: &HashMap<Handle, NativeFunction<'p>>,
    ) {
        for (output, input) in &header.aliases {
            self.definitions.insert(
                output.clone(),
                DatabaseEntry::new(DatabaseDefinition::Alias(input.clone())),
            );
        }
        for native in &header.natives {
            self.definitions.insert(
                native.clone(),
                DatabaseEntry::new(DatabaseDefinition::Native(
                    natives.get(native).unwrap().clone(),
                )),
            );
        }
        for incomplete in &header.incompletes {
            self.definitions
                .entry(incomplete.clone())
                .or_insert_with(|| {
                    DatabaseEntry::new(DatabaseDefinition::Static(Definition::default()))
                });
        }
        for mutable in &header.mutables {
            self.definitions.entry(mutable.clone()).or_insert_with(|| {
                DatabaseEntry::new(DatabaseDefinition::Static(Definition::default()))
            });
        }
        for export in &header.exports {
            self.definitions.get_mut(export).unwrap().set_public();
        }
        for handle in &header.mutables {
            self.definitions
                .get_mut(handle)
                .unwrap()
                .definition
                .set_mutable();
        }
    }

    pub fn lookup(&self, handle: &Handle, public: bool) -> Option<&DatabaseDefinition<'p>> {
        let entry = self.definitions.get(handle)?;
        if public && !entry.public {
            return None;
        }
        match &entry.definition {
            DatabaseDefinition::Alias(handle) => self.lookup(handle, false),
            definition => Some(definition),
        }
    }

    pub fn resolve<'a>(&'a self, handle: &'a Handle, public: bool) -> Option<&'a Handle> {
        let entry = self.definitions.get(handle)?;
        if public && !entry.public {
            return None;
        }
        match &entry.definition {
            DatabaseDefinition::Alias(handle) => self.resolve(handle, false),
            _ => Some(handle),
        }
    }

    pub fn resolve_operator<'a>(&'a self, key: &OpKey) -> Option<&'a Operator> {
        self.operators
            .get(&Default::default())
            .and_then(|operators| operators.get(key))
    }

    pub fn exports(&self, handle: &Handle) -> bool {
        self.definitions
            .get(handle)
            .map(|entry| entry.public)
            .unwrap_or(false)
    }

    pub fn into_library(mut self, lib: Atom) -> Self {
        self.definitions = self
            .definitions
            .into_iter()
            .map(|(mut handle, mut entry)| {
                handle.add_lib(lib.clone());
                for handle in entry.definition.handles_mut() {
                    handle.add_lib(lib.clone());
                }
                (handle, entry)
            })
            .collect();
        self.operators = self
            .operators
            .into_iter()
            .map(|(mut scope, mut operators)| {
                scope.add_lib(lib.clone());
                operators
                    .values_mut()
                    .for_each(|op| op.handle_mut().add_lib(lib.clone()));
                (scope, operators)
            })
            .collect();
        self
    }

    pub fn merge(mut self, library: Self) -> Self {
        self.definitions.extend(library.definitions);
        self.operators.extend(library.operators);
        self
    }
}
