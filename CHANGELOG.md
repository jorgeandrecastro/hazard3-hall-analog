# Changelog

All notable changes to this project are documented in this file.

## [0.5.0] 2026-11-24

### Breaking changes

- Removed RP2040 support. This driver now targets only RP2350 (Pico 2) with Hazard3 RISC-V cores.
- Updated all Cargo dependencies to use Hazard3 packages (`hazard3-rp`, `hazard3-time`).
- Changed default feature from `rp2040` to `rp235xa`.

### Documentation

- Updated README and examples for RP235x only.
- Removed RP2040-specific conversion examples.
- Updated installation instructions for Hazard3 ecosystem.

## [0.4.0] - 2026-05-02

### Improvements

- Replaced compatibility ranges with explicit dependency versions for more stable and reproducible builds.

### Dependencies

```toml
[dependencies]
embassy-rp = "0.10"
embassy-time = "0.5"
```

## [0.3.0] - 2026-04-25

### Added

- Automatic zero-point calibration with `calibrate()`.
- Sample averaging to determine the real offset of the sensor.
- `embassy-time` integration with 100 microsecond delays between calibration samples.
- Complete usage examples for calibration and polarity detection.
- Compatibility tables aligned with the Embassy dependency versions.

### Improved

- Measurement precision is improved by separating calibration reads and reducing repeated noise peaks.
- The API remains compatible with previous releases for easier migration from 0.2.x.

### Hardware notes

#### RP2350B (Pico 2 Zero / 80 pins)

On some RP2350B development boards, ADC pins can be multiplexed or shared physically with the
SD card or QSPI/SDIO bus.

- Symptom: unstable raw `read_raw()` values, noise, or phantom readings.
- Cause: interference from high-frequency digital traffic on the shared bus.
- Solution: use ADC pins isolated from digital traffic and ensure that no shared peripheral is active
  during analog reads. Filtering, such as a 100 nF capacitor, may also help.

#### RP2350A (Pico 2)

The dedicated ADC pin GP26 was tested successfully.

#### Real zero points

Theoretical 12-bit and 14-bit midpoints do not always match the measured hardware midpoint.

Measured example in the absence of a magnetic field:

- RP2350A with calibration: approximately 2060 instead of 2048.
- Variation depends on sensor resistance, temperature, supply stability, and PCB layout.

Recommendation: call `calibrate()` at startup to obtain the real midpoint for the installation.

## Previous versions (< 0.3.0)

### v0.2.0 limitations

Earlier versions did not provide automatic zero-point calibration or stabilization between
ADC reads. They assumed that the midpoint was always exactly 2048 for 12-bit readings or 8192
for 14-bit readings.

Practical consequences:

- inaccurate polarity detection;
- incorrect signed deviation over time;
- unpredictable behavior with temperature changes;
- poor compatibility with Hall modules that have real component variation.

### Migration

Before 0.3.0:

```rust
let deviation = sensor.read_deviation(ZERO_FIELD_RAW_12BIT).await;
```

With 0.3.0 or later:

```rust
let zero_offset = sensor.calibrate(64).await;
let deviation = sensor.read_deviation(zero_offset).await;
```

Versions 0.4.0 and later are strongly recommended for new projects.

## Format

This changelog follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

Releases follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
