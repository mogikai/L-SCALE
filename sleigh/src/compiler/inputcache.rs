use crate::compiler::InputId;
use crate::compiler::lexer::SleighTokens;
use ariadne::{
    Cache,
    Source,
};
use slotmap::{
    SecondaryMap,
    SlotMap,
};
use std::collections::HashMap;
use std::fmt::{
    Debug,
    Display,
};
use std::fs;
use std::io::Error;
use std::path::{
    Path,
    PathBuf,
};
use thiserror::Error;

/// The input cache stores inputs, their paths, and their tokens once they have been lexed.
/// Each loaded path is associated with an input id.
#[derive(Debug, Default, Clone)]
pub struct CompilerInputCache {
    loaded_files: HashMap<PathBuf, InputId>,
    input_paths: SlotMap<InputId, PathBuf>,
    input_sources: SecondaryMap<InputId, Source>,
    input_tokens: SecondaryMap<InputId, SleighTokens>,
}

impl Cache<InputId> for CompilerInputCache {
    type Storage = String;

    fn fetch(
        &mut self,
        id: &InputId,
    ) -> Result<&Source<Self::Storage>, Box<dyn Debug + '_>> {
        self.input_sources
            .get(*id)
            .ok_or(Box::new("Could not find input") as _)
    }

    fn display<'a>(
        &self,
        id: &'a InputId,
    ) -> Option<Box<dyn Display + 'a>> {
        self.input_paths
            .get(*id)
            .and_then(|input| input.to_str().map(|s| Box::new(s.to_string()) as _))
    }
}

impl CompilerInputCache {
    pub fn load_input_from_str(
        &mut self,
        name: &str,
        data: &str,
    ) -> Result<InputId, InputCacheError> {
        let mut path = PathBuf::from("/PSEUDO/");
        path.push(name);
        let source = Source::from(data.to_string());
        let id = self.input_paths.insert(path.clone());
        self.input_sources.insert(id, source);
        self.loaded_files.insert(path, id);
        Ok(id)
    }

    pub fn load_input_from_file(
        &mut self,
        path: &Path,
    ) -> Result<InputId, InputCacheError> {
        let path = path.to_path_buf();
        if let Some(&id) = self.loaded_files.get(&path) {
            Ok(id)
        } else {
            let source = Source::from(fs::read_to_string(path.as_path())?);
            let id = self.input_paths.insert(path.clone());
            self.input_sources.insert(id, source);
            self.loaded_files.insert(path, id);
            Ok(id)
        }
    }

    pub fn input_path(
        &self,
        input_id: InputId,
    ) -> Result<&Path, InputCacheError> {
        self.input_paths
            .get(input_id)
            .ok_or(InputCacheError::FileNotLoaded)
            .map(PathBuf::as_path)
    }

    pub fn input_source(
        &self,
        input_id: InputId,
    ) -> Result<&Source, InputCacheError> {
        self.input_sources
            .get(input_id)
            .ok_or(InputCacheError::FileNotLoaded)
    }

    pub fn input_tokens(
        &self,
        input_id: InputId,
    ) -> Result<&SleighTokens, InputCacheError> {
        self.input_tokens
            .get(input_id)
            .ok_or(InputCacheError::InputNotTokenized)
    }

    pub fn attach_tokens(
        &mut self,
        input_id: InputId,
        tokens: SleighTokens,
    ) -> Result<(), InputCacheError> {
        self.input_tokens.insert(input_id, tokens);
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum InputCacheError {
    #[error("Could not read the provided sleigh file.")]
    ReadError(#[from] Error),
    #[error("The requested file was not loaded.")]
    FileNotLoaded,
    #[error("The input has not been tokenized yet and has no attached tokens.")]
    InputNotTokenized,
}
