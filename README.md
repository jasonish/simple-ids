# Easy-Suricata

Easy-Suricata is a tool to easily run Suricata (and optionally
EveBox) on Linux systems using Docker or Podman.

This program is considered experimental and many things may change,
break, change name, change repo, etc, etc... And I might even
force push!

## System Requirements

- An x86_64, ArmV7 or Aarch64 based Linux distribution with Docker or
  Podman. This includes most Linux distributions available today
  including Raspberry Pi OS (32 bit or 64 bit).
- Root access.

### NOTE for Raspberry Pi Users

If on a Raspberry Pi make sure to NOT use an SD card for the data
directory. Heavy logging on an SD card is not only bad for the life of
the SD card, but can lead to the system being unresponsive, especially
if the logs are also being processed by a tool like EveBox.

Be sure to set a data directory that is not on the SD card in the
configure menu.

## Installation the Easy Way

```
curl -sSf https://evebox.org/easy.sh | sh
```

Or download directly from https://evebox.org/files/easy/.

Once you have the program download, run it:

```
./easy-suricata
```

Under the configure menu select your network interface, enable EveBox
if desired than select "Start" from the main menu.

## Building

If you just want to use Easy-Suricata you can download a pre-compiled
binary. The following is only for those who wish to compile
Easy-Suricata themselves.

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

#### ArmV7 (Raspberry Pi 32 bit)

```
cross build --release --target arm-unknown-linux-musleabihf
```

#### Aarch64 (Raspberry Pi 64 bit)

```
cross build --release --target aarch64-unknown-linux-musl
```
