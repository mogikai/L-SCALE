#![feature(let_else)]

pub mod emulator;
pub mod fuzzer;

#[macro_use] extern crate log;
extern crate core;

use static_assertions::assert_eq_size;

// This library emulates 64 bit targets and thus requires usize to be 64 bits
assert_eq_size!(u64, usize);

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
