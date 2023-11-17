//! Emulator specific observers that interact with the emulator from `FUZE`
//! There are several different coverage observers (e.g. `EdgeObserver`).
//! Another important part is the `StreamObserver` that is responsible for
//! checking whether or not new input streams are pending.
pub mod edge;
pub mod tcp;
