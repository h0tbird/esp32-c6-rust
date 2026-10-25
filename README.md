# esp32-c6-rust
This is a bare-metal Rust project for the ESP32-C6 that does not use the standard library (`no_std`).
Bare metal means the firmware runs directly on the microcontroller rather than under a desktop operating system.
Without `std`, firmware uses `core` and embedded libraries instead of OS features such as files and threads.

These are setup notes, not yet a complete build-and-flash guide. Run shell commands in a terminal.
The Homebrew commands assume macOS with Homebrew already installed.

## Install Rust

`rustup` manages Rust toolchains (the compiler, Cargo build tool, and target support).

```sh
brew install rustup
```

The following line belongs in `~/.zshrc`, the startup configuration for interactive Zsh shells.
It lets the shell find Homebrew's Rust tools. `/opt/homebrew` is the usual Homebrew location on Apple Silicon;
if Homebrew is installed elsewhere, adjust the path.

```sh
export PATH="${PATH}:/opt/homebrew/opt/rustup/bin"
export PATH="$HOME/.cargo/bin:$PATH"
```

The second line makes tools installed with `cargo install`, such as `esp-generate` and `espflash`, available.

Open a new terminal after saving that change. To check for Rust-related lines in the configuration:

```sh
cat ~/.zshrc | grep rust
```

This only displays matching lines; it does not install or configure Rust.
If no default Rust toolchain is configured yet, run `rustup default stable` to install and select one.

## Install VS Code extensions

These are extension IDs to search for in VS Code's Extensions view, not shell commands:

```text
rust-lang.rust-analyzer
tamasfe.even-better-toml
usernamehw.errorlens
```

- `rust-lang.rust-analyzer`: Rust code completion, navigation, and diagnostics.
- `tamasfe.even-better-toml`: editing support for TOML files, including Cargo manifests.
- `usernamehw.errorlens`: displays editor diagnostics alongside the affected code.

## Install support for the ESP32-C6 target

The compiler runs on the Mac, but the firmware must run on the ESP32-C6's RISC-V processor.
This command adds target support to the active Rust toolchain:

```sh
rustup target add riscv32imac-unknown-none-elf
```

In this target name, `riscv32imac` identifies the 32-bit RISC-V architecture and supported extensions,
`none` means there is no operating system, and `elf` is the executable file format.
Installing the target does not select it for builds; the project's Cargo configuration or a
`--target riscv32imac-unknown-none-elf` build option must do that.

## Install the Espressif project generator

`esp-generate` creates a Rust firmware project with chip-specific dependencies and build configuration.
Unlike `cargo init`, it provides an embedded starting point rather than a desktop "Hello, world!" program.

```sh
cargo install esp-generate --locked
```

This installs the generator on the Mac; it does not create or modify a firmware project yet.
`--locked` uses the generator's packaged dependency lockfile, but does not pin the version of `esp-generate` itself.

## Install debugging tools

`probe-rs` can flash and debug supported microcontrollers through a compatible debug probe.
Debugging includes setting breakpoints and inspecting the running firmware.
Using it requires a supported probe connection and board configuration; installing it alone does not connect to the board.

```sh
brew install probe-rs/probe-rs/probe-rs
```

## Install the Espressif flashing tool

`espflash` writes compiled firmware to Espressif chips over a supported serial connection.
It can also monitor serial output from the board. This is distinct from stepping through code with a debugger.

```sh
cargo install espflash --locked
```

`cargo install` installs a command-line tool on the Mac, not a dependency in the firmware project.
`--locked` uses the tool's packaged dependency lockfile for a more reproducible installation;
it does not pin the version of `espflash` itself.

## Build and run the firmware

The generated firmware is now part of this repository; there is no need to run `cargo init` or generate it again.
The Cargo package and binary retain the generated name `esp32-c6-starter`.

Run this from the repository root to compile without connecting to a board:

```sh
cargo build
```

`rust-toolchain.toml` selects stable Rust and requests the Rust source component and ESP32-C6 target.
`.cargo/config.toml` selects that target automatically and configures `espflash` as the runner.
`Cargo.lock` records the resolved dependency versions and should be committed with this firmware project.

With the board connected through a supported serial connection, build, flash, and monitor its output with:

```sh
cargo run
```

This command writes firmware to the connected board. The starter firmware logs "Hello world!" every
500 milliseconds using `defmt`; the configured runner decodes that output. Press Ctrl+C to exit the monitor.