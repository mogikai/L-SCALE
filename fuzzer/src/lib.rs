#![feature(return_position_impl_trait_in_trait)]
#![feature(iterator_try_collect)]

pub mod executor;
pub mod feedback;
pub mod inputs;
pub mod io;
pub mod mutators;
pub mod observers;
pub mod state;

pub fn add(
    left: usize,
    right: usize,
) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
