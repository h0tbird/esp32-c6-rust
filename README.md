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

This command writes firmware to the connected board. The firmware logs each LED color using `defmt`;
the configured runner decodes that output. Press Ctrl+C to exit the monitor.

### Command executed

On 2026-10-25, I ran the firmware using the release profile:

```sh
cargo run --release
```

`--release` selects the release build profile configured in `Cargo.toml`, then uses the same
`espflash` runner to flash the firmware and monitor its output.

### Next step: add smart LED support

I plan to run this from the repository root:

```sh
cargo add smart-leds
```

This adds `smart-leds` as a firmware dependency in `Cargo.toml` and updates `Cargo.lock`.
The crate provides common color types and traits for smart LEDs; it does not by itself configure
the ESP32-C6 hardware or drive an LED. That also requires a compatible driver and the correct board pin.

### Add the ESP32-C6 smart LED driver

On 2026-10-25, I added the RMT peripheral adapter for smart LEDs:

```sh
cargo add esp-hal-smartled@0.18.0 --features esp32c6
```

`esp-hal-smartled` connects the smart LED traits to the ESP32-C6's RMT peripheral, which generates
the precisely timed pulses needed by supported addressable LEDs.
Version `0.18.0` requires `esp-hal ~1.2`, which is compatible with this project's `esp-hal 1.2.2`.
The `esp32c6` feature selects the chip; the existing `esp-hal` dependency enables the required unstable APIs.

The command updates `Cargo.toml` and `Cargo.lock`. Adding the dependency does not change the running
firmware: using it still requires initializing RMT, selecting the board's LED data pin, and writing LED colors.

To check that the dependencies compile together without flashing the board:

```sh
cargo build
```

### Blink and cycle through LED colors

The firmware in `src/bin/main.rs` assumes one WS2812-compatible addressable RGB LED connected to
GPIO8, using WS2812B timing and GRB wire order. Check your board's pinout: this is not the same as
an ordinary single-color LED, and other boards may use a different pin or LED protocol.

RMT channel 0 generates the LED signal using an 80 MHz peripheral clock. The firmware repeats
red, green, cyan, blue, and magenta, with each color on for 500 milliseconds
and the LED off for another 500 milliseconds. `LEVEL` sets the per-channel brightness to 10 out of 255.
The 500 ms waits use Embassy async timers, letting other tasks run while the LED waits.
The short RMT LED transmissions still use the blocking driver.

Build, flash, and monitor it with:

```sh
cargo run --release
```

The driver uses the `RmtSmartLeds` API from `esp-hal-smartled 0.18.0`; older examples using
`SmartLedsAdapter` and `smart_led_buffer!` do not match this version.
`SmartLedsWrite` supplies the `write` method, and `[color]` sends one pixel.
Writing black (`RGB8::new(0, 0, 0)`) turns it off. Initialization or transmission errors
stop the firmware with a panic rather than being ignored.

## Embassy: async firmware

Embassy is now enabled in this project. It lets firmware tasks wait without keeping the CPU
busy in a delay loop. This is a first step toward running the LED alongside networking;
it does not provide Thread or Matter by itself.

On 2026-10-25, these dependencies were added:

```sh
cargo add esp-rtos@0.4.0 --features esp32c6,embassy,defmt
cargo add embassy-executor@0.10.0 embassy-time@0.5.0
```

These are setup history, not commands to repeat after cloning: the dependencies are already
in `Cargo.toml`, and `Cargo.lock` records the resolved versions.

- `esp-rtos` supplies the Espressif runtime and its Embassy integration. The `esp32c6` feature
  selects the chip, `embassy` enables async execution and timers, and `defmt` enables logging support.
- `embassy-executor` supplies the executor types, including `Spawner`, which can launch async tasks.
  Do not enable a separate executor platform feature: `esp-rtos` provides the integration.
- `embassy-time` supplies async timers.

### What changed in the code

`src/bin/main.rs` now uses `#[esp_rtos::main]` and `async fn main` instead of the synchronous
`#[main]` entry point. After initializing the chip, it passes timer group 0's first timer
and the `FROM_CPU_INTR0` software interrupt to `esp_rtos::start`.
Those resources are now owned by the runtime and must not be initialized again elsewhere.

Each previous `delay.delay_millis(500)` is now:

```rust
Timer::after_millis(500).await;
```

`.await` suspends the current task until the timer expires, so the executor can run other ready
tasks. The LED loop currently runs in the main task; no separate background tasks are needed yet.
The colors, brightness, GPIO8 connection, and on/off timing have not changed.

### Verify this milestone

```sh
cargo build --release
cargo clippy --bin esp32-c6-starter -- -D warnings
```

Both checks passed after the conversion. They compile and lint the firmware without flashing.
To check the physical board yourself, run:

```sh
cargo run --release
```

Expect the same red, green, cyan, blue, and magenta sequence, with 500 ms on and 500 ms off.
On 2026-10-25, the diagnostic release build was flashed to the ESP32-C6-DevKitC-1.
Serial monitoring showed five complete color cycles with a measured on-time of 500 ms
for every color, including magenta, and no unexpected restart during the observation.
This verifies the reported software timing on hardware, not the perceived duration of the emitted light.

If a color appears to turn off too soon, compare the serial logs from `cargo run --release`.
Each color has the same 500 ms timer, and the firmware logs its measured on-time after sending
the off command, for example `LED: magenta off after 500 ms`. This measures the software interval,
including the short off transmission, not the LED's optical output. Small scheduling delays are possible.
The startup message should appear only once per boot; if it repeats unexpectedly, check the
surrounding output for a reset or panic rather than assuming the color delay is wrong.

## Next milestone: Apple Home over Matter and Thread

Hardware for this project:

- **ESP32-C6-DevKitC-1:** the onboard addressable RGB LED is connected to GPIO8.
- **Apple TV model MN893LL/A:** Apple TV 4K (3rd generation), 128 GB Wi-Fi + Ethernet,
  which supports Thread and can act as the home hub / Thread border router.

On the iPhone, check **Home > ... > Home Settings > Home Hubs & Bridges** and confirm the Apple TV
is listed and available. Keep the phone and Apple TV on the same home network, not an isolated guest network.

**Not implemented yet:** this firmware does not advertise a Matter accessory, join Thread,
or supply a pairing code. Enabling Embassy alone does not make it discoverable in Apple Home.

The next integration will need:

1. A compatible Thread and Matter stack, with BLE for initial commissioning.
2. A Matter On/Off Light endpoint: **On** enables the blinking sequence; **Off** stops it and
   turns the LED off. The blink timer must be interruptible so Off does not wait for a whole cycle.
3. Persistent storage for pairing credentials and the Thread network configuration, plus a
   deliberate factory-reset mechanism. Do not erase flash casually after pairing.
4. A firmware-generated Matter setup code for **Home > + > Add Accessory**.
   The board's serial number is not a Matter setup code.

The [rs-matter-embassy project](https://github.com/sysgrok/rs-matter-embassy) provides a candidate
ESP32-C6 Thread light integration. Its networking dependencies and flash-storage requirements
still need to be checked against this project before adopting it. Development attestation
credentials are for experiments, not a certified commercial accessory.

References:

- [Espressif esp-rtos 0.4.0 setup](https://docs.espressif.com/projects/rust/esp-rtos/0.4.0/esp32c6/esp_rtos/index.html)
- [Apple TV model identification and Thread support](https://support.apple.com/en-us/101605)
- [Adding Matter accessories to Apple Home](https://support.apple.com/en-us/102135)