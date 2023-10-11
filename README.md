# SimpleNSM - Suricata/EveBox

SimpleNSM is a tool to easily run Suricata and EveBox Linux systems
using Docker or Podman.

This program is considered experimental and many things may change,
break, change name (I'm thinking simpleids is better), change repo,
etc, etc... And I might even force push!

## System Requirements

- An x86_64, Aarch64 or Arm32 based Linux distribution with Docker or
  Podman. This includes most Linux distributions available today
  including Raspberry Pi OS.
- Root access.

## Installation the Easy Way

```
mkdir ~/simplensm
curl -sSf https://evebox.org/simplensm.sh | sh
```

Or download directly from https://evebox.org/files/simplensm/.

Once you have the program downloaded, run it:

```
./simplensm
```

Under the configure menu select your network interface, then select
"Start" from the main menu.

## Building

If you just want to use SimpleNSM you can download a pre-compiled
binary. The following is only for those who wish to compile SimpleNSM
themselves.

### For Host OS

```
cargo build --release
```

### Static Targets

Static binaries for x86_64 and other platforms can be built with the
`cross` tool. To install `cross`:

```
cargo install cross
```

#### x86_64

```
cross build --release --target x86_64-unknown-linux-musl
```

#### Aarch64 (Raspberry Pi 64 bit)

```
cross build --release --target aarch64-unknown-linux-musl
```

#### ArmV7 (Raspberry Pi 32 bit)

```
cross build --release --target arm-unknown-linux-musleabihf
```
