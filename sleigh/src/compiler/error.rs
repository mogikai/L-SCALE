use crate::compiler::ast::Spanned;
use crate::compiler::inputcache::InputCacheError;
use crate::compiler::lexer::InputSpan;
use crate::preprocessor::error::PreprocessorError;
use crate::spec::builder::{MOIRSymbolId, StringId};
use crate::spec::moir::common::MOIRSize;
use crate::spec::ProcessorSpecificationError;
use ariadne::Report;
use std::fmt::{
    Debug,
    Display,
    Formatter,
};
use thiserror::Error;
use crate::compiler::ast::define::SASTAddressSpaceAttribute;
use crate::spec::moir::endianness::MOIREndianness;

#[derive(Error)]
pub enum SleighCompilerError {
    #[error("The preprocessor encountered an error.")]
    PreprocessorError(#[from] PreprocessorError),
    #[error("The input cache encountered an error.")]
    InputCacheError(#[from] InputCacheError),
    #[error("The processor specification encountered an error")]
    ProcessorSpecificationError(#[from] ProcessorSpecificationError),
    #[error("An error occurred during lexical analysis.")]
    LexerError(String),
    #[error("An error  occurred during parsing")]
    ParserError(String),
    #[error("The requested source was not found.")]
    MissingSource,
    #[error("The input is not yet tokenized.")]
    NotTokenized,
    #[error("The input is not yet preprocessed. Parsing requires a preprocessed token stream.")]
    NotPreprocessed,
    #[error("The compiler encountered an error during the semantic analysis phase.")]
    SemanticError(String),
    #[error("The value cannot be converted into a boolean value")]
    CannotConvertToBool,
    #[error("The operation encountered an overflow during constant folding")]
    ConstEvalOverflow,
    #[error("A detailed error report referencing the inputs.")]
    ErrorReport(Box<Report<'static, InputSpan>>),
    #[error("The identifier is not mapped.")]
    IdentifierNotMapped(Spanned<StringId>),
    #[error("The symbol was not declared before being defined.")]
    SymbolUndeclared,
    #[error("The value cannot be sliced, since the requested slice is either too large or too small. New size: {new:?}, current size: {current:?}")]
    InvalidSliceSize { new: MOIRSize, current: MOIRSize },
    #[error("The value cannot be sliced, since the slice range is invalid")]
    InvalidSliceRange {
        current_start: MOIRSize,
        current_end: MOIRSize,
        new_start: MOIRSize,
        new_end: MOIRSize,
    },
    #[error("This error needs replacement")]
    TODO,
    #[error("The slice cannot be sliced as long as the end is not determined")]
    CannotSliceUndetermined,
    #[error("The compiler was configured to stop on the first error encountered")]
    StopOnFirstError,
    #[error("The constant value being constructed does not fit into the requested size. Value: {val:?}, Requested size: {size:?}")]
    ConstDoesNotFit { val: u128, size: MOIRSize },
    #[error("The symbol is already defined")]
    DuplicateDefinition,
    #[error("The builtin operation was called with an incorrect number of arguments")]
    InvalidArgumentCount,
    #[error("Generic const eval error")]
    ConstEvalError(ConstEvalError),
    #[error("Cannot slice left hand side")]
    CannotSliceLhs(CannotSliceLhs),
    #[error("Identifier already exists: {0}")]
    IdentifierExists(IdentifierExists),
    #[error("Endianness already exists: {0}")]
    EndiannessExists(EndiannessExists),
    #[error("Alignment already specified: {0}")]
    AlignmentSpecified(AlignmentSpecified),
    #[error("Undefined symbol: {0}")]
    UndefinedSymbol(UndefinedSymbol),
    #[error("Invalid symbol type: {0}")]
    InvalidSymbolType(InvalidSymbolType),
    #[error("Undefined identifier: {0}")]
    UndefinedIdentifier(UndefinedIdentifier),
    #[error("Space attribute is already defined: {0}")]
    DuplicateSpaceAttribute(DuplicateSpaceAttribute),
    #[error("Atomize statement failed: {0}")]
    AtomizeStatementFailed(AtomizeStatementFailed),
    #[error("Process item failed: {0}")]
    ProcessItemFailed(ProcessItemFailed),
    #[error("The error should have been handled earlier")]
    UnhandledError(Box<SleighCompilerError>),
}

#[derive(Error, Debug)]
#[error("Cannot slice left hand side")]
pub struct ConstEvalError {
    pub err: Box<SleighCompilerError>,
    pub lhs: InputSpan,
    pub rhs: InputSpan,
}

#[derive(Error, Debug)]
#[error("Cannot slice left hand side")]
pub struct CannotSliceLhs {
    pub err: Box<SleighCompilerError>,
    pub ident_str: String,
    pub ident: InputSpan,
    pub expected_size: MOIRSize,
}

#[derive(Error, Debug)]
#[error("Identifier exists")]
pub struct IdentifierExists {
    pub name: String,
    pub old: InputSpan,
    pub new: InputSpan,
}

#[derive(Error, Debug)]
#[error("Endianness exists")]
pub struct EndiannessExists {
    pub old: Spanned<MOIREndianness>,
    pub new: InputSpan,
}

#[derive(Error, Debug)]
#[error("Alignment specified")]
pub struct AlignmentSpecified {
    pub old: Spanned<u16>,
    pub new: InputSpan
}

#[derive(Error, Debug)]
#[error("Undefined symbol")]
pub struct UndefinedSymbol {
    pub symbol: Spanned<String>,
}

#[derive(Error, Debug)]
#[error("Invalid symbol type")]
pub struct InvalidSymbolType {
    pub defined_symbol: InputSpan,
    pub symbol_reference: Spanned<String>,
    pub expected: String,
    pub found: MOIRSymbolId,
}

#[derive(Error, Debug)]
#[error("Undefined identifier")]
pub struct UndefinedIdentifier {
    pub identifier: Spanned<String>,
}

#[derive(Error, Debug)]
#[error("Duplicate space attribute")]
pub struct DuplicateSpaceAttribute {
    pub attribute: SASTAddressSpaceAttribute,
    pub prev_input: InputSpan,
    pub input: InputSpan,
}

#[derive(Error, Debug)]
#[error("{err:?}")]
pub struct AtomizeStatementFailed {
    pub err: Box<SleighCompilerError>,
    pub span: InputSpan,
}

#[derive(Error, Debug)]
#[error("{err:?}")]
pub struct ProcessItemFailed {
    pub err: Box<SleighCompilerError>,
    pub span: InputSpan,
}


impl Debug for SleighCompilerError {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            SleighCompilerError::PreprocessorError(e) => Debug::fmt(e, f),
            SleighCompilerError::ProcessorSpecificationError(e) => Debug::fmt(e, f),
            SleighCompilerError::LexerError(e) => Display::fmt(e, f),
            SleighCompilerError::InputCacheError(e) => Debug::fmt(e, f),
            SleighCompilerError::SemanticError(e) => Display::fmt(e, f),
            _ => Display::fmt(self, f),
        }
    }
}
