use crate::ast::*;
use crate::program::*;
use std::collections::HashMap;
use std::path::Path;

pub struct LumberBuilder<'p> {
    core: bool,
    context: Context<'p>,
    natives: HashMap<Handle, NativeFunction<'p>>,
}

impl<'p> LumberBuilder<'p> {
    fn new() -> Self {
        Self {
            core: true,
            context: Context::default(),
            natives: HashMap::default(),
        }
    }

    pub fn core(mut self, core: bool) -> Self {
        self.core = core;
        self
    }

    pub fn bind<H, F>(mut self, handle: H, native: F) -> crate::Result<Self>
    where
        H: AsHandle,
        F: Fn() + 'p, // TODO: this is not the final type
    {
        self.natives.insert(
            handle.as_handle(&mut self.context)?,
            NativeFunction::new(native),
        );
        Ok(self)
    }

    pub fn link<S>(mut self, name: S, program: Lumber<'p>) -> Self
    where
        S: AsRef<str>,
    {
        self.context
            .libraries
            .insert(self.context.atomizer.atomize_str(name.as_ref()), program);
        self
    }

    pub fn build_from_file<S>(self, source: S) -> crate::Result<Lumber<'p>>
    where
        S: AsRef<Path>,
    {
        let source_code = std::fs::read_to_string(&source)?;
        self.build(source, source_code)
    }

    pub fn build_from_str<S>(self, source: S) -> crate::Result<Lumber<'p>>
    where
        S: AsRef<str>,
    {
        let source_dir = std::env::current_dir()?;
        self.build(source_dir, source)
    }

    pub fn build<P, S>(mut self, root: P, source: S) -> crate::Result<Lumber<'p>>
    where
        P: AsRef<Path>,
        S: AsRef<str>,
    {
        if self.core {
            crate::core::LIB.with(|lib| {
                let core = self.context.atomizer.atomize_str("core");
                self.context.libraries.insert(core, lib.clone());
            });
        }
        Lumber::new(self.context, root, source, self.natives)
    }
}

/// A full Lumber program, ready to have queries run against it.
#[derive(Default, Clone, Debug)]
pub struct Lumber<'p> {
    libraries: HashMap<Atom, Lumber<'p>>,
    database: Database<'p>,
}

impl<'p> Lumber<'p> {
    pub fn from_file<P: AsRef<Path>>(source_file: P) -> crate::Result<Self> {
        let source_code = std::fs::read_to_string(&source_file)?;
        Self::new(
            Context::default(),
            source_file,
            source_code,
            HashMap::default(),
        )
    }

    pub fn from_str<S: AsRef<str>>(source_code: S) -> crate::Result<Self> {
        let source_dir = std::env::current_dir()?;
        Self::new(
            Context::default(),
            source_dir,
            source_code,
            HashMap::default(),
        )
    }

    pub fn builder() -> LumberBuilder<'p> {
        LumberBuilder::new()
    }

    fn new<P: AsRef<Path>, S: AsRef<str>>(
        context: Context<'p>,
        source_file: P,
        source_code: S,
        natives: HashMap<Handle, NativeFunction<'p>>,
    ) -> crate::Result<Self> {
        let source_str = source_code.as_ref();
        context.compile(source_file.as_ref().to_owned(), source_str, natives)
    }

    pub(crate) fn build(libraries: HashMap<Atom, Lumber<'p>>, database: Database<'p>) -> Self {
        Self {
            libraries,
            database,
        }
    }

    pub(crate) fn exports(&self, handle: &Handle) -> bool {
        self.database.exports(&handle.without_lib())
    }
}
