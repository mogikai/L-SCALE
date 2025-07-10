# FuzzyEmu - Emulate user space binaries for fuzzing

This crate can be used to emulate user space targets for Fuzzing (see [fuzzyfuzz](../fuzzer)).
It provides a hook API for the fuzzer to do observations of the target during its execution.
It also provides the capability to efficiently create snapshots of the target and later reset to these snapshots.

TODO: Document and give some examples maybe.