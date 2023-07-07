# FUZE - Emulator Based Fuzzing

TODO: Extend documentation

The repository consists of two sub-crates [fuzzyemu](emulator) and [fuzzyfuzz](fuzzer).
[fuzzyemu](emulator) provides means to emulate userspace binary targets.
It currently only supports targets that are intended to run on an x86 linux kernel.

To run the demo target:
```shell
cargo run -r -p fuzzyfuzz --bin=simple_fuzzer
```
