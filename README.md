# esp32-c6-rust

A Rust laboratory for **Matter over Thread** on the ESP32-C6. There is one firmware,
named `esp32-c6-rust`: Apple Home controls the board's RGB LED as an On/Off Light.
No external light, relay, or wiring is required.

This is bare-metal, `no_std` firmware: it runs directly on the microcontroller, using
`core` and embedded libraries rather than a desktop operating system. Embassy async
timers let the LED wait without blocking networking.

The accessory uses upstream **public development attestation credentials and setup
passcode**. It is a laboratory device, not a certified production product. Flash
encryption is not configured. Never commit home-network credentials or raw commissioning logs.

## Hardware and behavior

- **ESP32-C6-DevKitC-1:** one WS2812-compatible RGB LED on GPIO8, driven by RMT channel 0
  at 80 MHz with WS2812B timing and GRB wire order.
- **Thread border router/home hub:** this setup uses an Apple TV 4K, third generation,
  128 GB Wi-Fi + Ethernet (model MN893LL/A).
- **iPhone:** supplies the Thread network configuration during Bluetooth commissioning.
  Bluetooth is still required for initial setup even though normal control uses Thread.

**On** repeats red, green, cyan, blue, and magenta: 500 ms lit, then 500 ms black.
Brightness is 10/255 per active channel. **Off** interrupts the wait and sends black.
The first boot starts off; subsequent boots restore the saved state unless the Matter
`StartUpOnOff` attribute requests Off, On, or Toggle.

Keep the board within Thread coverage of the Apple TV or another Thread router.
A nearby iPhone does not relay Thread traffic, and a Wi-Fi extender alone does not
extend Thread coverage.

## Set up the Mac

Commands below assume macOS with Homebrew installed. Install tools only if missing:

```sh
brew install rustup cmake llvm
```

Select the Rust tools and Homebrew LLVM in the current terminal:

```sh
export PATH="$(brew --prefix rustup)/bin:$HOME/.cargo/bin:$PATH"
export PATH="$(brew --prefix llvm)/bin:$PATH"
export LIBCLANG_PATH="$(brew --prefix llvm)/lib"
```

`~/.cargo/bin` contains Cargo-installed tools such as `espflash`. LLVM is keg-only,
so its `bin` directory must precede Apple's system compiler on `PATH`.
`LIBCLANG_PATH` locates the shared library used to generate C bindings; it is not
another executable search path.

To persist these exports, add them to `~/.zshrc` and open a new terminal.
Nothing in this repository modifies your shell configuration automatically.
Selecting LLVM also affects other compiler commands in that shell; temporary exports
in a project terminal are sufficient if you prefer not to change your usual compiler.

If Rust has no default toolchain yet:

```sh
rustup default stable
```

Install the serial flashing/monitoring tool:

```sh
cargo install espflash --locked
```

`rust-toolchain.toml` requests stable Rust, `rust-src`, and the
`riscv32imac-unknown-none-elf` target. The compiler runs on the Mac; that target
produces code for the ESP32-C6's 32-bit RISC-V processor, without an operating system.
Cargo selects it automatically through `.cargo/config.toml`.

The native tools below were missing during the original Matter build:

| Tool | Why it is required | Version installed on 2026-10-25 |
| --- | --- | --- |
| CMake | Configures the native MbedTLS crypto library | 4.4.4 |
| Homebrew LLVM / Clang | Cross-compiles C for RISC-V and supplies `libclang`; Apple's Clang lacked the target | 23.1.4 |

Check the setup with:

```sh
command -v cargo
command -v espflash
cmake --version
command -v clang
clang --print-targets
echo "$LIBCLANG_PATH"
```

Clang should resolve inside Homebrew's LLVM directory and list `riscv32` as a target.
No project generation is needed after cloning: `esp-generate`, `esp-config`, and
`cargo init` are not part of the normal build workflow. `probe-rs` is also unnecessary
for the serial flashing/monitoring workflow used here.

VS Code extension recommendations are in `.vscode/extensions.json`. Run VS Code from
a terminal with the build environment configured if its tasks cannot find the tools.

## Build, flash, and monitor

Run commands from the repository root. Build without touching the board:

```sh
cargo build --release --locked
```

Build, flash, and open the serial monitor:

```sh
cargo run --release --locked
```

There is no firmware-selection feature or `--bin` option to remember. Plain
`cargo run --release` also selects the same Matter firmware; `--locked` additionally
prevents an implicit dependency-lockfile update.

The release profile enables optimization for the embedded networking stack.
The Cargo runner invokes `espflash` with `defmt` decoding. Ctrl+C closes the monitor,
not the application on the board. Close any existing monitor before flashing again.

To choose a serial port explicitly and flash an already-built image:

```sh
espflash flash \
  --port /dev/cu.usbserial-110 \
  --chip esp32c6 \
  --monitor --log-format defmt \
  target/riscv32imac-unknown-none-elf/release/esp32-c6-rust
```

Replace the port if your Mac assigns a different name. Normal flashing with the same
partition layout preserves pairing storage; do not use an erase-all option.

VS Code tasks provide `build-release` (the default build task), `clippy`, and
`flash-and-monitor`. There is no configured breakpoint debugger.

## Add to Apple Home

For a board that is not already paired:

1. Power the ESP32 near the Apple TV, with the iPhone nearby and Bluetooth enabled.
2. Confirm the Apple TV is available under **Home Settings > Home Hubs & Bridges**.
   Keep it and the iPhone on the same home network, not an isolated guest network.
3. Read the onboarding QR code/manual code from the serial output.
4. Open **Home > + > Add Accessory** on the iPhone. Use the displayed code, not the
   board's serial number. A development/uncertified-accessory warning may appear.
5. Test On and Off. A dark LED before an On command is expected on first setup.

The iPhone supplies the Thread dataset; no SSID/password or dataset needs to be added
to the source. An already-paired board should reconnect using its existing Home
accessory after an ordinary reset or firmware update.

## Persistence and factory reset

The firmware validates the partition table and reserves the existing NVS partition
exclusively for `SeqMapKvBlobStore`. On this board it is `0x9000..0xf000` (24 KiB),
separate from the bootloader, PHY data, and application. This is sequential-storage
format, not ESP-IDF NVS; do not share it with another firmware's NVS contents.

The store contains Matter fabrics, network configuration, the Thread SRP registration
key at `0x1000`, and LED settings at `0x1001`. Only changed LED settings are written,
not each blink. Flash errors and invalid current settings are reported, not silently
replaced with defaults.

The hardware-derived Thread EUI-64, commissioning discriminator, serial number, and
Basic Information unique ID stay stable across resets and reflashing. The operational
Thread extended address is managed by OpenThread, not forced to that EUI-64.
Cryptographic randomness comes from hardware entropy.

### Compatibility with earlier firmware

The first integrated firmware accidentally shared key `0x1000` between LED settings
and Thread, causing a panic on a later boot. The current firmware retains recovery:

- A recognizable legacy three-byte LED record is saved at `0x1001` before being
  removed from Thread's slot. Thread must regenerate its previously overwritten SRP key.
- If Thread already overwrote the LED record, its previous On/Off state is unrecoverable.
  The firmware explicitly logs initialization to Off and preserves Thread's data.
- Existing fabrics and network configuration are not erased. Invalid records at the
  new LED key remain errors.

Keep this migration while boards with older data may be updated. Removing the former
standalone blink firmware or renaming the executable does not change this storage format.

### Deliberate factory reset

Only use this when you intend to remove pairing:

1. With the firmware running, hold **BOOT (GPIO9)** for at least five seconds.
   Releasing sooner cancels the operation.
2. The firmware stops networking and the LED, then erases only the reserved NVS partition.
3. Release BOOT to restart with fresh pairing and the LED off.
4. Remove the old Home accessory entry before adding it again.

Do not hold BOOT while pressing RESET or reconnecting power: GPIO9 is also a boot-mode
strap and can select serial download mode. A brief **RST/RESET** press, or Ctrl+R in
the interactive serial monitor, restarts the board without erasing pairing.

## Tests and checks

Lint the firmware without flashing:

```sh
cargo clippy --release --locked --bins -- -D warnings
```

Run the native regression suite on an Apple Silicon Mac:

```sh
CARGO_TARGET_DIR=target/host-tests cargo test \
  --manifest-path tests/host/Cargo.toml \
  --locked --target aarch64-apple-darwin
```

For an Intel Mac, use `--target x86_64-apple-darwin`. The explicit host target overrides
the embedded default. This separate test crate is not another firmware: it compiles
the same light modules against real Matter storage interfaces without the ESP32 HAL.
Its native `defmt` test mode is isolated from firmware builds.

The ten tests cover the color sequence, Off/restart behavior, settings encoding,
identity, preservation of Thread/fabric records, legacy migration, interrupted
migration, failed writes, and rejection of invalid current records.

Check formatting for both crates:

```sh
cargo fmt --all -- --check
cargo fmt --manifest-path tests/host/Cargo.toml -- --check
```

## Troubleshooting

### Home shows No Response

1. Check power, the active home hub, and Thread coverage. Compare operation near the
   Apple TV before concluding that the firmware or office location is at fault.
2. Attach a serial monitor using the ELF that matches the flashed firmware:

   ```sh
   espflash monitor \
     --port /dev/cu.usbserial-110 \
     --chip esp32c6 \
     --log-format defmt \
     --elf target/riscv32imac-unknown-none-elf/release/esp32-c6-rust
   ```

3. If only espflash's connection messages appear, press Ctrl+R in that terminal to
   capture startup. The monitor's "Using flash stub" is not an application-health message.
4. Look for restored credentials, Thread attachment, and actual command handling:

| Log | What it establishes |
| --- | --- |
| `Stored Matter fabric present: true` | Pairing credentials loaded, not necessarily connectivity |
| `Loaded persistent LED settings from key 0x1001` | The separate LED record decoded |
| `Role detached -> child` / `Netif up` | Thread attachment/network availability at that point |
| `Matter LED enabled: true` / `false` | An application state change |
| Panic/backtrace or repeated startup | A firmware failure or reset to investigate |

The `LED: off` frames between colors are blink intervals, not proof of an Off command.
If Home's display disagrees with the board, compare iPhone and Mac, reopen Home, and
capture one explicit On/Off attempt. Do not erase pairing as the first diagnostic step.

**Redact the entire `Connecting to Thread network, dataset:` line before sharing logs.**
The upstream dependency prints network credentials there. Do not commit raw logs.

### SRP registration conflict

`SRP callback error: code=29` indicates a duplicate DNS name/record conflict in this
context. It can be consistent with a stale registration or a signing-key mismatch
after the old storage collision. `Registered SRP host` only reports local configuration,
not successful server acceptance.

A normal Apple TV restart, followed by waiting for hub recovery and restarting the
ESP32, is a non-destructive recovery attempt, not a guaranteed way to clear persisted
SRP registrations. Do not recreate the Thread network merely to diagnose this error.

### MLE duplicate-message warnings

`Mle-----------: Failed to process UDP: Duplicated` is different from the SRP conflict.
In the pinned OpenThread receive path, an authenticated neighbor message is rejected
if its frame counter is not newer than the last accepted counter, or its key sequence
is older. This is receive-side replay protection, not an LED error.

The [OpenThread maintainer's explanation](https://github.com/orgs/openthread/discussions/9553)
distinguishes retransmissions from a sender losing counter state. The warning alone
does not identify the sender or counters, so it cannot establish the cause here.
Working Home control has been observed while these warnings were present.

Keep replay checks and warnings enabled. Do not change device identity, reset counters,
or suppress all OpenThread warnings just to quiet the log. Keep Thread routers updated.
If warnings coincide with failures, capture the control attempt and surrounding network
events. Identifying the exact cause requires targeted sender/counter diagnostics in
OpenThread's C++ rejection path, not more LED logs or a blanket Rust log-level change.

## Code and dependencies

| Path | Responsibility |
| --- | --- |
| `src/main.rs` | Hardware initialization, Matter endpoints, persistent store, concurrent tasks and reset |
| `src/light.rs` | Pure blink sequence, settings encoding and board identity |
| `src/matter_light.rs` | Matter On/Off hooks, interruptible LED task and storage migration |
| `src/lib.rs` | Shared light modules |
| `tests/host/` | Native regression harness and its own dependency lockfile |
| `.cargo/` | Embedded target, serial runner and `DEFMT_LOG=info` |

The firmware allocates a 100 KiB heap and a static Matter stack. Dependencies retain
the compatible `esp-hal 1.2.2`, `esp-rtos 0.4.0`, `esp-hal-smartled 0.18.0`, and
`esp-radio 1.0.0-beta.1` integration. Logging uses `defmt` consistently.

`Cargo.lock` pins registry dependencies. Git dependencies are also pinned:

- [rs-matter-embassy](https://github.com/sysgrok/rs-matter-embassy/tree/2a806b0b1b521293b34a773618499e87402c843c)
  at `2a806b0b1b521293b34a773618499e87402c843c`; its published placeholder is not a usable release.
- [OpenThread bindings](https://github.com/esp-rs/openthread/tree/b01e7e63a48943d060e971e7ea364a84609c7ffb)
  at `b01e7e63a48943d060e971e7ea364a84609c7ffb` for both `openthread` and `openthread-sys`.

Do not casually update these together with unrelated cleanup: newer upstream examples
may require a different HAL integration. Earlier generator/blink milestones remain in
Git history rather than in the current build instructions.

## Hardware verification status

On 2026-10-25, Apple Home commissioning, On/Off control, restoration of saved pairing
and LED settings after reset, Thread reattachment, and control without re-pairing were
confirmed. Home's displayed state became consistent after retesting On/Off; the cause
of its temporary mismatch was not established.

After consolidation into one firmware, the release image was flashed on the same day.
Startup restored both saved fabrics and the LED record at `0x1001`, then reattached to
Thread. The Mac resolved the existing Matter service at port 5540 and received all
three IPv6 echo replies. No panic or SRP callback error was observed during monitoring;
MLE duplicate warnings remained. No On/Off commands were observed in that session;
the user subsequently reported that the consolidated firmware appeared to work in Home.
This is a manual smoke check, not a long-term reliability result.

A separate full power-cycle check, physical BOOT factory-reset test, and long-term
reliability testing remain outstanding. After flashing a new build, test the existing
accessory near the hub without re-pairing. Successful compilation or credential loading
alone does not prove end-to-end recovery.

References:

- [Espressif esp-rtos setup](https://docs.espressif.com/projects/rust/esp-rtos/0.4.0/esp32c6/esp_rtos/index.html)
- [Apple TV model identification](https://support.apple.com/en-us/101605)
- [Adding Matter accessories to Apple Home](https://support.apple.com/en-us/102135)
