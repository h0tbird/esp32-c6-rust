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
```

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

Flashing still requires a firmware build, the board's connection details, and an appropriate flash command.