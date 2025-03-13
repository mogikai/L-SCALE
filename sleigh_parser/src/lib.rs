use crate::ast::item::SASTItem;
use crate::inputcache::InputCacheError;
use crate::lexer::InputSpan;
use crate::tokens::SleighToken;
use serde_derive::{
    Deserialize,
    Serialize,
};
use std::fmt::{
    Display,
    Formatter,
};
use std::ops::{
    Deref,
    DerefMut,
};
use string_interner::backend::{
    Backend,
    StringBackend,
};
use string_interner::{
    StringInterner,
    Symbol,
};
use thiserror::Error;

pub mod ast;
pub mod inputcache;
pub mod lexer;
pub mod parser;
pub mod preprocessor;
pub mod tokens;

pub trait ParserContext {
    /// Intern the provided value that can be converted to a string.
    /// If the value already exists, a key to that value is returned.
    /// Otherwise, a new entry is created and the key to that is returned.
    fn intern<T: AsRef<str>>(
        &mut self,
        string: T,
    ) -> StringId;

    /// Get the id for an interned string if it exists.
    fn lookup<T: AsRef<str>>(
        &self,
        string: T,
    ) -> Option<StringId>;

    fn string(
        &self,
        string_id: StringId,
    ) -> Option<&str>;

    fn string_unchecked(
        &self,
        string_id: StringId,
    ) -> &str {
        self.string(string_id).expect("The string id must exist")
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SleighInterner(StringInterner<StringBackend>);

impl SleighInterner {
    pub fn shrink_to_fit(&mut self) {
        self.0.shrink_to_fit();
    }
}

impl ParserContext for SleighInterner {
    fn intern<T: AsRef<str>>(
        &mut self,
        string: T,
    ) -> StringId {
        StringId(self.0.get_or_intern(string).to_usize())
    }

    fn lookup<T: AsRef<str>>(
        &self,
        string: T,
    ) -> Option<StringId> {
        self.0.get(string).map(|id| StringId(id.to_usize()))
    }

    /// Resolve a string id to the corresponding interned string.
    fn string(
        &self,
        string_id: StringId,
    ) -> Option<&str> {
        self.0
            .resolve(<<StringBackend as Backend>::Symbol as Symbol>::try_from_usize(string_id.0)?)
    }
}

#[derive(Debug)]
pub struct SleighAST {
    pub items: Vec<SASTItem>,
}

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("The parser encountered an error during lexing: {0:?}")]
    LexerError(String),
    #[error("The parser encountered an input cache error: {0:?}")]
    InputCacheError(#[from] InputCacheError),
    #[error("The parser encountered an error during parsing: {0:?}")]
    ParserError(String),
}

pub type SleighParserResult<T> = Result<T, ParseError>;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
/// StringId given out by interner. The interner is used by the compiler
/// to simplify string comparisons by mapping ids to strings.
pub struct StringId(usize);

impl Display for StringId {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub type SpannedToken = (SleighToken, InputSpan);
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Spanned<T> {
    value: T,
    span: InputSpan,
}

impl<T: Copy> Copy for Spanned<T> {}

impl<T> Spanned<T> {
    pub fn new(
        value: T,
        span: InputSpan,
    ) -> Self {
        Self { value, span }
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn span(&self) -> &InputSpan {
        &self.span
    }

    /// Converts the spanned value into the contained value, discarding the span.
    pub fn into_value(self) -> T {
        self.value
    }

    /// Converts the spanned value into its span, discarding the value.
    pub fn into_span(self) -> InputSpan {
        self.span
    }

    pub fn into_tuple(self) -> (T, InputSpan) {
        (self.value, self.span)
    }

    pub fn as_tuple(&self) -> (&T, &InputSpan) {
        (&self.value, &self.span)
    }

    /// Transmute the inner value from type `T` into type `U` as long as T implements Into<U>.
    pub fn transmute<U>(self) -> Spanned<U>
    where
        T: Into<U>,
    {
        Spanned {
            value: self.value.into(),
            span: self.span,
        }
    }

    /// Transmute the inner value from type `T` into type `U` as long as T implements TryInto<U>.
    pub fn try_transmute<U>(self) -> Result<Spanned<U>, T::Error>
    where
        T: TryInto<U>,
    {
        Ok(Spanned {
            value: self.value.try_into()?,
            span: self.span,
        })
    }

    pub fn map<F, U>(
        self,
        map: F,
    ) -> Spanned<U>
    where
        F: FnOnce(T) -> U,
    {
        Spanned::new(map(self.value), self.span.clone())
    }
}

pub trait TransmuteIntoVec<T>: IntoIterator<Item = Spanned<T>> {
    /// Transmute the inner value from type `T` into type `U` as long as T implements Into<U> for a given iterable.
    /// The output will be collected into a vec.
    fn transmute_into_vec<U>(self) -> Vec<Spanned<U>>
    where
        Self: Sized,
        T: Into<U>,
    {
        self.into_iter().map(Spanned::transmute).collect()
    }
}

impl<T> TransmuteIntoVec<T> for Vec<Spanned<T>> {}

impl<T> Spanned<Option<T>> {
    pub fn transpose(self) -> Option<Spanned<T>> {
        self.value.map(|x| Spanned::new(x, self.span))
    }
}

impl<T> Deref for Spanned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for Spanned<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}
