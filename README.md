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

**On** repeats red, green, blue, and cyan: 500 ms lit, then 500 ms black.
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

VS Code extension recommendations are in `.vscode/extensions.json`. The workspace
settings pass `CLANG_PATH` and `LIBCLANG_PATH` explicitly to rust-analyzer's Cargo
processes; the build/flash tasks use the same values. This prevents Flycheck from
silently selecting Apple's `/usr/bin/clang` when VS Code did not inherit your terminal
exports. The paths assume Apple Silicon Homebrew at `/opt/homebrew`; adjust both VS Code
configuration files to the output of `brew --prefix llvm` on other installations.

After changing the environment, run **rust-analyzer: Restart server** from the Command
Palette. If necessary, use **Developer: Reload Window**. Shell exports still apply to
commands you run manually; the VS Code settings do not modify your shell or the board.
Run VS Code from a configured terminal if it cannot find Cargo, CMake, or other tools.

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

## Local control with chip-tool

`chip-tool` is the official Matter SDK's command-line controller. It talks to the
ESP32 through the local network and Thread border router: no USB connection or
Thread radio in the Mac is required. The Mac still needs network reachability
to the accessory. Installing the CLI alone does not authorize it to control the board.

### Native Apple Silicon installation

On this Mac, the SDK is checked out at `~/git/project-chip/connectedhomeip`, using
tag `v1.6.1.0`. The compiled executable is `out/chip-tool/chip-tool`, with a symlink
at `~/bin/chip-tool`. `~/bin` is already on this machine's PATH. Both the executable
and the ZAP code-generation tool were verified as ARM64; Rosetta is not required.

The SDK's downloaded ZAP was Intel-only. An official ARM64 ZAP release, `v2026.05.14`,
was installed at `~/.local/opt/zap-v2026.05.14-arm64`. It meets the SDK's minimum
ZAP version of `2026.5.12`. To rebuild with this native tool:

```sh
cd ~/git/project-chip/connectedhomeip
source scripts/activate.sh
export ZAP_INSTALL_PATH="$HOME/.local/opt/zap-v2026.05.14-arm64"
ninja -C out/chip-tool
```

This assumes the SDK's Darwin submodules, environment, and build directory are
already prepared. The existing binary does not need activation or the ZAP export
just to run. Use `chip-tool` without arguments to list commands; this version does
not support a global `--help` option.

Avoid rerunning `scripts/examples/gn_build_example.sh` just to resume this build:
at this SDK tag it prints the entire environment with `env`, potentially exposing
tokens and passwords. Run Ninja directly and review logs before sharing them.

References: [chip-tool guide](https://project-chip.github.io/connectedhomeip-doc/development_controllers/chip-tool/chip_tool_guide.html)
and [ZAP ARM64 release](https://github.com/project-chip/zap/releases/tag/v2026.05.14).

### Add the CLI without removing Apple Home

Matter multi-admin lets Apple Home and chip-tool each have their own authorized
fabric. Do not remove the Home accessory or factory-reset the board to add the CLI.

1. Open the accessory's settings in Home and select **Turn On Pairing Mode**.
   This worked from Home on the Mac in this setup; an iPhone can also provide it.
2. Copy the new setup code shown by Home. Use it while the commissioning window
   remains open, not the original boot-time code.
3. Create a private, persistent directory for the CLI controller's credentials:

   ```sh
   mkdir -p "$HOME/.config/chip-tool"
   chmod 700 "$HOME/.config/chip-tool"
   ```

4. Replace the placeholder below with the new code and run:

   ```sh
   chip-tool pairing code 1234 'NEW_CODE_FROM_HOME' \
     --storage-directory "$HOME/.config/chip-tool" \
     --paa-trust-store-path "$HOME/git/project-chip/connectedhomeip/credentials/development/paa-root-certs"
   ```

   The PAA directory contains development attestation trust roots for this test
   accessory. Adjust its path if the SDK is elsewhere; do not bypass attestation
   verification simply to avoid configuring the trust store.

`1234` is the node ID assigned to this ESP32 in the CLI's fabric, not an Apple Home
ID or the board's serial number. Wait for successful commissioning before sending
commands. This setup is already paired as node `1234`; do not repeat pairing for
normal use.

Always use the same storage directory and default commissioner identity for the
commands below. Without the directory option, this version defaults to temporary
storage and may use a different controller identity. Keep the directory private,
do not commit it, and do not delete it as routine cleanup: losing it loses the CLI's
credentials, even though the board may retain that fabric. A setup code is not
needed again for each command.

### On, Off, and state

Run these from any directory:

```sh
# On: start the four-color blinking sequence.
chip-tool onoff on 1234 1 \
  --storage-directory "$HOME/.config/chip-tool"

# Off: stop blinking and clear the LED.
chip-tool onoff off 1234 1 \
  --storage-directory "$HOME/.config/chip-tool"

# Read the actual Matter OnOff attribute.
chip-tool onoff read on-off 1234 1 \
  --storage-directory "$HOME/.config/chip-tool"
```

The final `1` is the LED's endpoint. A read reports `OnOff: TRUE` when the blinking
sequence is enabled and `OnOff: FALSE` when disabled. The black intervals within
the sequence do not change the logical OnOff attribute.

On 2026-10-25, CLI commissioning completed successfully while preserving Apple Home.
Off and On each returned `SUCCESS`; subsequent reads returned `FALSE` and `TRUE`,
respectively. This verifies authenticated Matter commands independently of Home's UI,
not optical LED output or long-term reliability.

If a command times out, first check Thread coverage and the home hub. Use the same
storage directory as commissioning and confirm the board still holds its pairing.
Do not factory-reset or delete controller credentials as the first troubleshooting step.

`CHIP Error 0x000000DB: The Resource is busy and cannot process the request`
with a received Busy status during `CASE_Sigma1` means the accessory was reachable
but could not accept secure-session setup at that moment. It is not an Off-command
success or proof that pairing was lost. Each separate CLI process may establish a
new secure session. There are at least two relevant paths in the pinned Matter
library that advertise the same 500 ms minimum retry delay:

- The Busy responder handles exchanges not accepted by normal handlers in time.
- The transport rejects a new unencrypted session when its session table is full,
  sends Busy, and then attempts to evict an existing session. This can leave space
  for the next attempt without accepting the original request.

The advertised delay does not identify which path was taken or guarantee success
after exactly 500 ms. Board-side logs confirmed the session-table path in this lab:
after a normal reboot, eight separate CLI reads succeeded, then reads 9, 11, 13,
and 15 failed with `No space for a new unencrypted session, sending Busy`.
Reads 10, 12, and 14 succeeded. All successful reads reported `OnOff: FALSE`.
The reboot preserved all three stored fabrics and the light setting.

This distinguished session-table exhaustion from the delayed Busy responder.
The original transport rejected the first request before attempting eviction, so a
later request could succeed after a slot had been freed. Rebooting only cleared
volatile sessions; increasing session limits would merely postpone exhaustion.

The local `rs-matter` patch now queues the normal CloseSession notification in the
separate TX buffer, evicts an eligible inactive session, and retries decoding the
original plaintext handshake once. It preserves the received bytes, uses the
existing eviction policy, and leaves encrypted-message and replay checks unchanged.
Reserved sessions and sessions with active exchanges remain protected. If none can
be evicted, Busy remains the correct response. Session limits are unchanged.

After flashing the patch without erasing NVS, 32 consecutive independent CLI reads
succeeded with no retries, followed by successful On/read-TRUE/Off/read-FALSE
commands. All three fabrics and the LED settings were restored. This verifies the
reproduced exhaustion case, not a guarantee that Busy can never occur under load.
Redact Thread datasets before sharing board logs.

### Reuse a session for repeated commands

Interactive mode keeps the controller running and reuses an established secure
session instead of opening a new one for every command:

```sh
chip-tool interactive start \
  --storage-directory "$HOME/.config/chip-tool"
```

At its prompt, enter commands without the `chip-tool` prefix:

```text
onoff read on-off 1234 1
onoff on 1234 1
onoff off 1234 1
quit
```

Before the admission patch, a six-read interactive test received Busy on its first
read, succeeded on its second, and succeeded on the remaining four using the live
controller session. All successful reads reported `OnOff: FALSE`; that test did not
change the light state. Interactive mode remains useful for session reuse, but is
no longer required to avoid the reproduced alternating-failure pattern.
Legitimate Busy responses can still occur when resources are occupied. Persistent
failures need diagnosis rather than re-pairing or endless retries.

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

Run the six local Matter session-admission regression tests:

```sh
cargo test --manifest-path vendor/rs-matter/Cargo.toml \
  --locked --lib --no-default-features --features os,rustcrypto,log \
  rx_session_admission --target aarch64-apple-darwin \
  --target-dir target/host-tests
```

These exercise CASE and PASE admission with a full session table, preservation of
the incoming packet and duplicate detection, active/reserved-session protection,
the Busy status report when eviction is impossible, normal admission, and rejection
of non-handshake or unknown encrypted-session traffic without eviction.

Check formatting for both crates:

```sh
cargo fmt --all -- --check
cargo fmt --manifest-path tests/host/Cargo.toml -- --check
rustfmt --check --edition 2021 --config skip_children=true \
  vendor/rs-matter/src/transport.rs
```

## Troubleshooting

### rust-analyzer / Flycheck cannot compile MbedTLS

`Forcing on-the-fly build` means the selected crypto configuration differs from the
prebuilt library, so the dependency must compile C locally. The long list of disabled
options and inherited-frame-pointer warnings are not, by themselves, the fatal error.

If the output identifies `AppleClang`, `/usr/bin/clang`, and
`No available targets are compatible with triple "riscv32-unknown-unknown"`, VS Code is
using a compiler without the required RISC-V backend. Check the explicit LLVM paths
above and restart rust-analyzer. Do not enable unrelated crypto features to match the
prebuilt library, disable Flycheck, or erase firmware data to work around this.
This is a host-side editor/build failure, not a panic in the running ESP32 firmware.

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
On this board, opening `espflash monitor --non-interactive --no-reset` reproduced
CASE timeouts: reads failed both while that monitor was open and after it closed.
A normal monitor startup/reset restored Thread attachment and successful reads,
including after closing the monitor. Avoid `--no-reset` for this setup; it has not
proved to be a non-disruptive way to observe the running device. The exact serial
control-line/reset mechanism has not been established. This failure is distinct
from a received Matter Busy response, and normal reset preserves stored fabrics.

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
| `vendor/rs-matter/` | Apache-2.0 rs-matter 0.4.1 with the local session-admission patch and regression tests |
| `.cargo/` | Embedded target, serial runner and `DEFMT_LOG=info` |

The firmware allocates a 100 KiB heap and a static Matter stack. Dependencies retain
the compatible `esp-hal 1.2.2`, `esp-rtos 0.4.0`, `esp-hal-smartled 0.18.0`, and
`esp-radio 1.0.0-beta.1` integration. Logging uses `defmt` consistently.

`Cargo.lock` pins registry dependencies. Git dependencies are also pinned:

- [rs-matter-embassy](https://github.com/sysgrok/rs-matter-embassy/tree/2a806b0b1b521293b34a773618499e87402c843c)
  at `2a806b0b1b521293b34a773618499e87402c843c`; its published placeholder is not a usable release.
- [OpenThread bindings](https://github.com/esp-rs/openthread/tree/b01e7e63a48943d060e971e7ea364a84609c7ffb)
  at `b01e7e63a48943d060e971e7ea364a84609c7ffb` for both `openthread` and `openthread-sys`.

Both the firmware and host-test manifests patch `rs-matter` to
[`vendor/rs-matter`](vendor/rs-matter). It is the crates.io 0.4.1 package, whose
upstream source revision is
[`3f3528ef5c492b6feb0a2e043264dc47e9da4340`](https://github.com/project-chip/rs-matter/tree/3f3528ef5c492b6feb0a2e043264dc47e9da4340/rs-matter),
with local changes confined to `src/transport.rs`. The upstream license and
provenance metadata are retained. Vendoring makes the fix reproducible without
editing Cargo's shared cache or upgrading unrelated dependencies. Reassess the
patch when adopting an upstream release with equivalent admission behavior.

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
