use std::collections::HashMap;

pub trait ContextProvider {
    fn set<S: Into<String>>(&mut self, name: S, value: u64);
    fn values(&self) -> impl Iterator<Item = (&String, &u64)>;
}

#[derive(Debug, Default)]
/// Context for sleigh during matching is transient and handled through the [ContextStack].
/// The [ContextProvider] is responsible for permanent context, i.e. user-provided, initial
/// values as well as values originating from globalset directives.
pub struct SimpleContextProvider {
    values: HashMap<String, u64>,
}

impl ContextProvider for SimpleContextProvider {
    fn set<S: Into<String>>(&mut self, name: S, value: u64) {
        self.values.insert(name.into(), value);
    }

    fn values(&self) -> impl Iterator<Item = (&String, &u64)> {
        self.values.iter()
    }
}
