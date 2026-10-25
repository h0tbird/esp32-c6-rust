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

During project generation, `esp-generate` also requested the missing `esp-config` configuration tool.
Accepting its prompt installed it with:

```sh
cargo install esp-config --features=tui --locked
```

`tui` enables its terminal user interface. Like other Cargo-installed tools, it is placed in
`~/.cargo/bin`, which must be on `PATH`. It is already installed on this machine; this command
is recorded for setting up another machine, not something to repeat before each build.

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
The Cargo package and default binary are named `esp32-c6-rust`, matching the repository.

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
and the LED off for another 500 milliseconds. The shared `COLORS` table in `src/light.rs` sets
the per-channel brightness to 10 out of 255.
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
cargo clippy --bin esp32-c6-rust -- -D warnings
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

## Apple Home over Matter and Thread

Hardware for this project:

- **ESP32-C6-DevKitC-1:** the onboard addressable RGB LED is connected to GPIO8.
- **Apple TV model MN893LL/A:** Apple TV 4K (3rd generation), 128 GB Wi-Fi + Ethernet,
  which supports Thread and can act as the home hub / Thread border router.

On the iPhone, check **Home > ... > Home Settings > Home Hubs & Bridges** and confirm the Apple TV
is listed and available. Keep the phone and Apple TV on the same home network, not an isolated guest network.

### Matter-controlled onboard LED

There are now two firmware binaries. Only one runs on the board at a time:

| Binary | Purpose |
| --- | --- |
| `esp32-c6-rust` (default) | The existing, hardware-tested five-color blinking firmware |
| `matter-thread` | Matter-controlled onboard RGB blinking, BLE commissioning, Thread transport, and flash persistence |

The second binary is in `src/bin/matter_thread.rs` and requires the opt-in `matter-thread` feature.
Plain `cargo run --release` still selects the standalone blinking firmware, not the Apple Home accessory.
Use the explicit `--features matter-thread --bin matter-thread` options below for Matter.

The experimental binary allocates a 100 KiB heap for the radio/crypto dependencies, initializes
hardware entropy for cryptography, and allocates the Matter stack statically rather than on
the small execution stack. It uses the upstream example's public development attestation credentials
and setup passcode. This is an experimental accessory, not a certified commercial product.
Do not treat the example credentials as production secrets or publish your home network credentials.

The Matter device type is "On/Off Light", but the hardware is only the board's RGB LED on GPIO8:
no external light, relay, or wiring is needed.

- **On:** repeat red, green, cyan, blue, and magenta, with 500 ms on and 500 ms off.
- **Off:** wake the LED task without waiting for the 500 ms timer and send black to the LED.
- The first boot starts with the LED off. Later boots restore the saved On/Off state, unless the
  Matter `StartUpOnOff` attribute requests Off, On, or Toggle instead.
- Only changes to On/Off or startup settings are written to flash, not each blink or repeated
  command with the same value.
- `src/matter_light.rs` supplies the real device hooks. `src/light.rs` contains the testable sequence,
  settings encoding, and stable identity derivation. The upstream test light logic is no longer used.
- The standalone blinking binary still has no Thread or Matter support.

### Pairing storage and device identity

The integrated firmware reads the actual partition table and uses the existing **NVS partition**
exclusively as a `SeqMapKvBlobStore`. On this board the reserved range is `0x9000..0xf000`
(24 KiB), separate from the bootloader, partition table, PHY data, and application.
No new partition layout or whole-chip erase is required. This is the sequential-storage format,
not ESP-IDF's NVS format; do not share this partition with another firmware's NVS contents.

The same store is supplied to Matter and the Thread transport so fabric credentials and network
configuration can be loaded after a restart. The Thread transport reserves vendor key `0x1000`
for its SRP service-registration key; the application's On/Off settings use `0x1001`.
Flash I/O errors and invalid settings are reported as failures rather than silently
discarding pairing. Flash encryption is not configured by this project.

The first integrated firmware incorrectly shared key `0x1000` with Thread. On 2026-10-25,
startup logs confirmed that pairing credentials loaded, but decoding the overwritten LED
record caused a panic before networking started. This can show up as **No Response** in Home,
even with the board close to the Apple TV.

The corrected firmware uses the separate LED key and preserves the existing Matter fabrics,
network configuration, and Thread SRP record. If the old key still contains a recognizable
three-byte LED record, it migrates that record before removing it from Thread's slot; Thread
must then regenerate the SRP key that the old firmware had overwritten. If Thread already
overwrote the old LED record, its previous On/Off state cannot be recovered: the firmware logs
that it is initializing the LED off and leaves Thread's data intact. Invalid records at the
new LED key remain errors, not silent resets.

To apply this fix, close any serial monitor with Ctrl+C and flash the integrated firmware
normally using the command below. Do not erase NVS or factory-reset as the first troubleshooting
step. Retest the existing Home accessory after the update; loading credentials alone is not
proof that Home control has recovered.

After flashing the correction on 2026-10-25, the user's logs confirmed that both saved fabrics
loaded, LED initialization no longer panicked, and Thread reached `Role detached -> child` and
`Netif up`. Service discovery still reported `SRP callback error: code=29`: in this SRP context,
OpenThread maps duplicate DNS names/records to error 29. This is consistent with a stale
registration or the signing-key collision in the earlier firmware, not proof of lost radio
connectivity. A `Registered SRP host` message alone only shows local configuration; subsequent
callback errors mean server acceptance has not succeeded.

As a non-destructive recovery attempt, restart the Apple TV normally (not a factory reset),
wait for its home-hub connection to recover, then restart the ESP32 and retry the existing
accessory. This is not guaranteed to clear persisted SRP registrations. Do not erase pairing
or recreate the Thread network merely to diagnose this error.

Subsequent logs on the same day confirmed saved LED settings loading from `0x1001` after
another reset, Thread reattachment, and working Matter control without re-pairing. Home briefly
displayed Off while the board was blinking; after another On/Off attempt, the user confirmed
the displayed state was consistent. The log showed `Matter LED enabled: false`, followed by
`LED: off` with no further color cycle. This confirms reset recovery and On/Off operation in
that test, not long-term reliability or the cause of the transient display mismatch.
MLE `Failed to process UDP: Duplicated` warnings remained during working control; they are
distinct from an SRP callback error and do not alone establish that the accessory is unreachable.

Before sharing serial logs publicly, redact the entire `Connecting to Thread network, dataset:`
line: the dataset contains network credentials. Do not commit raw commissioning logs.

The Thread EUI-64, commissioning discriminator, serial number, and Basic Information unique ID
are derived from the chip's factory MAC address. They remain stable across reboots and reflashing.
Radio cryptography still uses hardware entropy; the identity is not used as a cryptographic key.

Normal flashing with the same partition layout leaves the NVS contents intact. Do not use an
erase-all command or change the partition layout after pairing unless you intend to reset the accessory.

### Native build tools on macOS

The networking stack includes C crypto dependencies. The first build found the following missing tools:

| Tool | Why it was needed | Version installed on 2026-10-25 |
| --- | --- | --- |
| CMake | Configures and builds the native MbedTLS crypto library; the first build failed because `cmake` was missing | 4.4.4 |
| Homebrew LLVM / Clang | Cross-compiles C code for the ESP32-C6's RISC-V processor and provides `libclang` for generating Rust bindings; Apple's system Clang on this machine lacks the RISC-V target | 23.1.4 |

They were installed with the following commands (no need to repeat them on this machine):

```sh
brew install cmake llvm
```

**Yes, LLVM needs a PATH update for the Matter build.** Homebrew installs LLVM as "keg-only",
so its `clang` is not selected automatically. Prepending its `bin` directory makes the build
find it before Apple's `/usr/bin/clang`. CMake is installed in Homebrew's normal `bin` directory,
which is already on this machine's PATH; it needs no additional entry.

For the following commands, select Homebrew LLVM in the current terminal:

```sh
export PATH="$(brew --prefix llvm)/bin:$PATH"
export LIBCLANG_PATH="$(brew --prefix llvm)/lib"
```

This does not change your shell configuration. Repeat the exports in a new terminal when building
the experimental binary. `LIBCLANG_PATH` points the bindings generator to LLVM's shared libraries;
it is a separate setting, not another executable search path.

To make these settings persistent, add the same two export lines to `~/.zshrc`, after the existing
Rust/Cargo PATH entries, and reload it:

```sh
source ~/.zshrc
```

This selects Homebrew Clang for other commands in that shell too. If you prefer to leave your
normal compiler selection alone, use only the temporary exports in a terminal dedicated to this project.
No shell configuration file was changed automatically.

Check the setup with:

```sh
command -v cmake
cmake --version
command -v clang
clang --version
clang --print-targets
echo "$LIBCLANG_PATH"
```

`clang` should resolve inside Homebrew's LLVM directory, and its target list should include `riscv32`.
The original blink-only build does not require these native build tools.

Build without flashing:

```sh
cargo build --release --locked --features matter-thread --bin matter-thread
```

Check both binaries with the networking feature enabled:

```sh
cargo clippy --release --locked --features matter-thread --bins -- -D warnings
```

The integrated release build and combined Clippy check passed on 2026-10-25.
Build success alone does not establish successful commissioning or Apple Home control.

The integrated firmware was also flashed to the board on 2026-10-25. Serial logs confirmed
the reserved NVS range, a generated onboarding code, BLE host initialization, GATT advertising,
and the initial LED-off command. After a reset, the identity and onboarding code stayed the same,
and the firmware loaded LED settings and the events counter from flash. No unexpected panic
or reboot was observed during these startup checks.

Later on 2026-10-25, the user confirmed successful Apple Home commissioning and working On/Off
control of the onboard LED. The first pairing attempt timed out; pairing succeeded after retrying
near the Apple TV. This confirms operational Matter-over-Thread control, but does not establish
distance as the cause of the earlier failure. Paired power-cycle recovery and the physical BOOT
factory-reset procedure have not yet been confirmed.

### Flash and add to Apple Home

With the board connected by USB and the LLVM environment above selected, run:

```sh
cargo run --release --locked --features matter-thread --bin matter-thread
```

This replaces the currently running application with the integrated Matter firmware.
The board may look idle because its LED starts off until you send On.
Pressing Ctrl+C stops the serial monitor, not the firmware on the board.

1. Keep your iPhone near the board with Bluetooth enabled. Your Apple TV must be available as
   the home hub, as confirmed in **Home Settings > Home Hubs & Bridges**.
2. Watch the serial output for the Matter onboarding QR payload / manual pairing code.
   Use the code printed by this firmware, not the board's serial number or an unrelated example code.
3. On the iPhone, open **Home > + > Add Accessory**. Scan the displayed onboarding QR code,
   or use the manual code option. A development accessory may produce an uncertified-accessory warning.
4. Name it something like **ESP32 Blink**. On should start the five-color loop; Off should stop it.
5. Power-cycle the board and confirm the same accessory reconnects without being added again.
   The log `Stored Matter fabric present: true` indicates fabric credentials were loaded.
   A successful Thread attachment and working Home control are still needed to prove end-to-end recovery.

The iPhone supplies the Thread network configuration during commissioning; there is no Wi-Fi
SSID/password or Thread dataset to paste into this repository.

### Deliberate factory reset

To remove pairing from the board:

1. While the integrated firmware is running, press and hold **BOOT (GPIO9)** for at least
   five seconds. A shorter press cancels the operation.
2. The firmware stops the network and LED tasks and erases **only the reserved NVS partition**,
   including pairing, Thread configuration, and LED settings.
3. Release BOOT when prompted. The firmware then restarts with the LED off and fresh commissioning.
4. Remove any old accessory entry from Apple Home before pairing it again.

Do not hold BOOT while pressing RESET or reconnecting power: GPIO9 is also the chip's boot-mode
strap and can enter the serial download mode instead of running firmware.
There is also an early-startup hold check before loading storage, but an ordinary reboot alone
does not erase anything.

### Local logic tests

After a build has created `target`, run the hardware-independent tests on the Mac:

```sh
rustc --edition 2024 --test src/light.rs -o target/light-tests
./target/light-tests
```

They cover all five colors and black intervals, Off at every sequence phase, restarting at red,
the 500 ms interval constant, valid settings round-trips, rejection of malformed settings,
and deterministic board-specific identity. They do not substitute for radio or Apple Home tests.

To also test the actual Matter light hooks and storage migration on this Apple Silicon Mac:

```sh
CARGO_TARGET_DIR=target/host-tests cargo test \
  --manifest-path tests/host/Cargo.toml \
  --locked --target aarch64-apple-darwin
```

For an Intel Mac, use `--target x86_64-apple-darwin` instead. The explicit host target overrides
the project's embedded target. This separate test crate compiles the same light modules against
the real Matter storage interfaces without pulling in the ESP32 HAL; native `defmt` test mode
is isolated from the firmware build.

The combined suite includes preservation of Thread and fabric records across LED commands and
reloads, migration of legacy LED records, interrupted migration recovery, failed-write handling,
and rejection of invalid new records without overwriting stored data.

### Dependency compatibility

This baseline keeps `esp-hal 1.2.2`, `esp-rtos 0.4.0`, and the current LED driver. It uses:

- `esp-radio 1.0.0-beta.1` for BLE and IEEE 802.15.4 radio integration.
- `rs-matter 0.4.1` and `rs-matter-stack 0.3.1`, resolved in `Cargo.lock`.
- The [rs-matter-embassy Thread example revision](https://github.com/sysgrok/rs-matter-embassy/tree/2a806b0b1b521293b34a773618499e87402c843c/examples/esp),
  pinned to commit `2a806b0b1b521293b34a773618499e87402c843c`.
- [OpenThread Rust bindings](https://github.com/esp-rs/openthread/tree/b01e7e63a48943d060e971e7ea364a84609c7ffb),
  pinned to commit `b01e7e63a48943d060e971e7ea364a84609c7ffb` for both `openthread` and `openthread-sys`.

The upstream Embassy crate is not available as a usable release on crates.io yet. The selected
Git revisions avoid relying on moving upstream branches or replacing the existing HAL with an
experimental fork. `Cargo.lock` pins the remaining resolved dependencies. Logging consistently
uses `defmt`; mixing the HAL's `log-04` and `defmt` features causes a build error.

### Remaining verification

Apple Home commissioning, control after a reset with saved pairing, and consistent displayed
On/Off state after retesting have been confirmed by the user.
To separately verify a full power cycle, unplug the board, wait five seconds, reconnect it,
and confirm the existing accessory
responds to On/Off in Home without being added again. Allow time for it to rejoin Thread.
This verifies paired power-cycle recovery; loading credentials from flash alone does not.
The physical BOOT factory-reset procedure also remains to be verified.

References:

- [Espressif esp-rtos 0.4.0 setup](https://docs.espressif.com/projects/rust/esp-rtos/0.4.0/esp32c6/esp_rtos/index.html)
- [Apple TV model identification and Thread support](https://support.apple.com/en-us/101605)
- [Adding Matter accessories to Apple Home](https://support.apple.com/en-us/102135)