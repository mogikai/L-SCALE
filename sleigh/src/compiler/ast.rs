use crate::compiler::ast::item::SASTItem;
use crate::compiler::lexer::InputSpan;
use crate::compiler::tokens::SleighToken;
use std::ops::{
    Deref,
    DerefMut,
};

pub mod attach;
pub mod common;
pub mod constructor;
pub mod define;
pub mod item;
pub mod pcode;
pub mod pcode_macro;
pub mod with;

#[derive(Debug)]
pub struct SleighAST {
    pub(crate) items: Vec<SASTItem>,
}

pub type SpannedToken = (SleighToken, InputSpan);
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Spanned<T> {
    value: T,
    span: InputSpan,
}

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

    pub fn map<F, U>(
        &self,
        map: F,
    ) -> Spanned<U>
    where
        F: FnOnce(&T) -> U,
    {
        Spanned::new(map(&self.value), self.span.clone())
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
    pub fn lift_option(self) -> Option<Spanned<T>> {
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
