# MOGI - Emulator Based Testing

MOGI is a comprehensive emulator-based testing framework designed for binary analysis, fuzzing, and state learning.
This multi-crate Rust project provides tools for emulating user space binaries, performing coverage-guided fuzzing (
WIP), and implementing state learning algorithms.

## Architecture

The project is organized into several specialized crates:

### Core Components

- **`emulator/`** - Binary emulation engine for user space targets
- **`system/`** - System interfaces used by the emulator.
- **`unicorn/`** - Unicorn backend for the CPU and MMU interfaces
- **`fuzzer/`** - Currently only contains the learning backend implementation of L-SCALE
- **`statelearner/`** - State learning algorithms and utilities (currently only L*)

### Experimental CPU Emulator Components

- **`cpu/`** - CPU emulation and instruction handling
- **`mem/`** - Memory management and virtualization
- **`sleigh/`** - Sleigh processor specification compiler
- **`sleigh_parser/`** - Parser for Sleigh specifications

### Applications

- **`apps/lscale/`** - L-SCALE state learning binary ([README](apps/lscale/README.md))

### Utilities

- **`utilities/`** - Common utilities and helper functions
- **`derive/`** - Procedural macros and derive implementations
- **`gui/`** - Graphical user interface components (Currently abandoned. Needs restructuring)
- **`samples/`** - Example code and test cases
