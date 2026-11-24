# Hazard3 Hall Analog

[![Crates.io](https://img.shields.io/crates/v/embassy-hall-analog.svg)](https://crates.io/crates/embassy-hall-analog)
[![Documentation](https://docs.rs/embassy-hall-analog/badge.svg)](https://docs.rs/embassy-hall-analog)
[![License: GPL v2](https://img.shields.io/badge/License-GPL_v2-blue.svg)](https://www.gnu.org/licenses/old-licenses/gpl-2.0.en.html)

`hazard3-hall-analog` is an async, `no_std` Rust driver for the OPEN-SMART linear Hall-effect
sensor module, compatible with 49E and SS49E sensors. It is designed for RP235x (Pico 2)
targets and is part of the [Hazard3](https://github.com/jorgeandrecastro/Hazard3) async-RISC-V
ecosystem.

Hazard3 is an independent RISC-V fork of the Embassy ecosystem maintained by Jorge Andre Castro.
This driver keeps the Rust library names such as `embassy_rp`, `embassy_time`, and
`embassy_hall_analog`, while Cargo can resolve the corresponding packages from the Hazard3 ecosystem.
It is not affiliated with or endorsed by the official Embassy project.

## Project

The OPEN-SMART module produces an analog voltage proportional to the surrounding magnetic field:

| Condition | Output voltage at 3.3 V |
|---|---|
| No magnetic field | About 1.65 V (`VCC / 2`) |
| South pole nearby | Moves toward 3.3 V |
| North pole nearby | Moves toward 0 V |

The driver provides:

- asynchronous ADC reads;
- signed field deviation;
- magnetic polarity detection with a configurable deadband;
- automatic zero-point calibration;
- support for 14-bit RP235x ADC configurations.

## Wiring

No external resistor is required for normal operation.

```text
OPEN-SMART Hall module       RP235x
────────────────────────     ──────
VCC ──────────────────────── 3.3 V
GND ──────────────────────── GND
AO  ──────────────────────── GP26 (ADC0)
```

The AO pin is connected directly to the microcontroller ADC input.

## Installation

For the RP2350A (Pico 2), use the default `rp235xa` feature:

```toml
[dependencies]
embassy-hall-analog = "0.4.0"
```

For the RP2350B:

```toml
[dependencies]
embassy-hall-analog = { version = "0.4.0", default-features = false, features = ["rp235xb"] }
```

The package automatically uses the Hazard3 replacement packages:

```toml
embassy-rp = { package = "hazard3-rp", version = "0.2", features = ["rp235xa"] }
embassy-time = { package = "hazard3-time", version = "0.1" }
```

The two target features are mutually exclusive. Compilation fails when none or more than one is enabled.

## Building

This driver compiles only for the RP235x Hazard3 RISC-V cores. Add the RISC-V target first:

```bash
rustup target add riscv32imac-unknown-none-elf
cargo build --target riscv32imac-unknown-none-elf
```

For documentation:

```bash
cargo doc --no-deps --target riscv32imac-unknown-none-elf
```

For checking:

```bash
cargo check --target riscv32imac-unknown-none-elf
```

## Features

| Feature | Target | Default |
|---|---|---|
| `rp235xa` | RP2350A (Pico 2) | Yes |
| `rp235xb` | RP2350B (Pico 2 with 80 pins) | No |

| Feature | ADC resolution | Theoretical zero |
|---|---|---|
| `rp235xa`, `rp235xb` | 14-bit | 8192 |

Real ADC values can differ from the theoretical midpoint because of sensor resistance,
temperature, supply variation, PCB layout, and ADC configuration. Use `calibrate()` at startup
to determine the actual zero point.

## Usage

### Raw reading

```rust
#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::adc::{Adc, Channel, Config as AdcConfig};
use embassy_rp::adc::InterruptHandler;
use embassy_rp::bind_interrupts;
use embassy_hall_analog::HallAnalog;

bind_interrupts!(struct Irqs {
    ADC_IRQ_FIFO => InterruptHandler;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let adc = Adc::new(p.ADC, Irqs, AdcConfig::default());
    let channel = Channel::new_pin(p.PIN_26, embassy_rp::gpio::Pull::None);
    let mut sensor = HallAnalog::new(adc, channel);

    loop {
        let raw = sensor.read_raw().await;
        let _ = raw;
    }
}
```

### Polarity detection

```rust
use embassy_hall_analog::MagneticPolarity;

match sensor.read_polarity(50).await {
    MagneticPolarity::SouthPole => { /* South pole detected */ }
    MagneticPolarity::NorthPole => { /* North pole detected */ }
    MagneticPolarity::NoField => { /* No significant field */ }
}
```

The `deadband` argument is the half-width of the dead zone around the midpoint. A value of `50`
LSB is typical for 12-bit reads; `200` LSB is a typical starting point for 14-bit reads.

### Zero-point calibration

```rust
let zero_offset = sensor.calibrate(64).await;
let deviation = sensor.read_deviation(zero_offset).await;
// Positive deviation: south pole
// Negative deviation: north pole
```

Run calibration once at startup without a disturbing magnetic field.

### Signed deviation

```rust
use embassy_hall_analog::{HallAnalog, ZERO_FIELD_RAW_12BIT};

let deviation = sensor.read_deviation(ZERO_FIELD_RAW_12BIT).await;
```

This manual approach assumes that `2048` is the actual midpoint and is less accurate than
calibration.

### Conversions

Raw ADC value to voltage (RP235x, 14-bit):

```rust
let voltage = raw as f32 * 3.3 / 16383.0;
```

Estimated magnetic field:

```rust
// Typical 49E sensitivity at 3.3 V: about 0.92 mV/Gauss
let delta_v = voltage - 1.65;
let field_gauss = delta_v / 0.00092;
```

## API

### `HallAnalog::new(adc, channel) -> Self`

Creates the driver with an async ADC and channel.

### `async fn read_raw(&mut self) -> u16`

Reads one raw ADC value. It returns `0` if the ADC operation fails.

### `async fn read_polarity(&mut self, deadband: u16) -> MagneticPolarity`

Returns `SouthPole`, `NorthPole`, or `NoField`.

### `async fn read_deviation(&mut self, zero: u16) -> i32`

Returns a signed deviation in ADC LSB relative to the supplied midpoint.

### `async fn calibrate(&mut self, samples: u8) -> u16`

Averages `samples` readings and inserts a 100 microsecond delay between samples. The returned
value is the measured zero point.

### Constants

| Constant | Value | Description |
|---|---|---|
| `ZERO_FIELD_RAW_14BIT` | 8192 | RP235x theoretical midpoint |

## RP2350B note

The ADC pins on some RP2350B boards can be multiplexed or shared with the SD card or QSPI/SDIO
bus. This can produce unstable raw readings or phantom values. Use isolated ADC pins and avoid
activity on the shared digital bus during analog reads. The RP2350A has been tested on dedicated
ADC pin GP26.

## Compatibility

| Dependency | Version |
|---|---|
| `hazard3-rp` | `0.2` |
| `hazard3-time` | `0.1` |
| Rust edition | 2024 |
| `no_std` | Supported |
| Target | RP2350A, RP2350B (Hazard3 RISC-V cores) |

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for the detailed release history.

## License

Distributed under the **GPL-2.0-or-later** license. See [LICENSE](LICENSE) for details.

## About

Developed and tested by Jorge Andre Castro. This project is part of the broader Hazard3
RISC-V and async embedded Rust ecosystem.