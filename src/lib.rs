// Copyright (C) 2026 Jorge Andre Castro
// GPL-2.0-or-later
//! # embassy-hall-analog
//!
//! Async `no_std` driver for the OPEN-SMART linear Hall-effect sensor module (49E / SS49E)
//! on RP235x (Pico 2) microcontrollers, designed for the Hazard3 async RISC-V ecosystem.
//!
//! ## About the sensor
//!
//! The OPEN-SMART Hall sensor outputs an analog voltage proportional to the surrounding
//! magnetic field. With no field, the output stabilizes at approximately **VCC / 2** (~1.65 V
//! at 3.3 V). A south pole moves the voltage toward VCC; a north pole moves it toward GND.
//!
//! ## Wiring
//!
//! ```text
//! OPEN-SMART Hall module       RP235x
//! ─────────────────────────    ──────
//!        VCC  ────────────────── 3.3V
//!        GND  ────────────────── GND
//!        AO   ────────────────── GP26 (ADC0)
//! ```
//!
//! > **Note:** The AO pin connects directly to the ADC input. No external resistor is needed;
//! > the module has internal biasing.
//!
//! ## Available features
//!
//! | Feature | Target | Default |
//! |---|---|---|
//! | `rp235xa` | RP2350A (Pico 2) | Yes |
//! | `rp235xb` | RP2350B (Pico 2 80-pin) | No |
//!
//! These features are **mutually exclusive**. The build will fail if zero or more than one
//! is enabled.
//!
//! ## Example usage
//!
//! ```rust,no_run
//! #![no_std]
//! #![no_main]
//!
//! use embassy_executor::Spawner;
//! use embassy_rp::adc::{Adc, Channel, Config as AdcConfig};
//! use embassy_rp::bind_interrupts;
//! use embassy_rp::adc::InterruptHandler;
//! use embassy_hall_analog::HallAnalog;
//!
//! bind_interrupts!(struct Irqs {
//!     ADC_IRQ_FIFO => InterruptHandler;
//! });
//!
//! #[embassy_executor::main]
//! async fn main(_spawner: Spawner) {
//!     let p = embassy_rp::init(Default::default());
//!     let adc = Adc::new(p.ADC, Irqs, AdcConfig::default());
//!     let channel = Channel::new_pin(p.PIN_26, embassy_rp::gpio::Pull::None);
//!     let mut sensor = HallAnalog::new(adc, channel);
//!
//!     loop {
//!         let raw = sensor.read_raw().await;
//!         // ~8192 (RP235x) → no field
//!         // > 8192 → south pole  |  < 8192 → north pole
//!     }
//! }
//! ```
//!
//! ## Voltage and field calculation
//!
//! Raw ADC value to voltage (RP235x, 14-bit):
//!
//! ```text
//! V = raw × 3.3 / 16383
//! ```
//!
//! Deviation from midpoint indicates field intensity and polarity:
//!
//! ```text
//! ΔV = V - 1.65          (positive → south pole, negative → north pole)
//! ```
//!
//! Typical 49E sensitivity is **0.92 mV/Gauss** at 3.3 V:
//!
//! ```text
//! B (Gauss) = ΔV / 0.00092
//! ```
//!
//! ## Characteristics
//!
//! | Parameter | Value |
//! |---|---|
//! | Supply voltage | 3.3 V |
//! | ADC resolution | 14-bit (0–16383) RP235x |
//! | Resting output | ~VCC / 2 (~1.65 V at 3.3 V) |
//! | South pole output | Moves toward VCC |
//! | North pole output | Moves toward GND |
//! | Typical sensitivity | ~0.92 mV/Gauss at 3.3 V (49E) |
//! | Interface | Analog (AO) — direct ADC read |
//!
//! ## `no_std`
//!
//! This crate does not depend on the standard library and is designed to run
//! on bare-metal microcontrollers with the Hazard3 async executor.

#![no_std]
#![forbid(unsafe_code)]

// ---------------------------------------------------------------------------
// Garde de compilation : exactement une feature cible doit être activée.
// Le build.rs émet une erreur explicite si zéro ou plusieurs sont présentes,
// mais ce cfg guard est une sécurité supplémentaire au niveau de la crate.
// ---------------------------------------------------------------------------
#[cfg(not(any(feature = "rp235xa", feature = "rp235xb")))]
compile_error!(
    "hazard3-hall-analog: no target selected.\n\
     Enable exactly one feature: rp235xa or rp235xb."
);

#[cfg(all(feature = "rp235xa", feature = "rp235xb"))]
compile_error!(
    "hazard3-hall-analog: features `rp235xa` and `rp235xb` enabled simultaneously.\n\
     These features are mutually exclusive."
);

use embassy_rp::adc::{Adc, Channel, Error as AdcError};
use embassy_rp::mode::Async;
use embassy_time::Timer;

// ---------------------------------------------------------------------------
// Constantes publiques nommées explicitement (rétrocompatibilité)
// ---------------------------------------------------------------------------

/// ADC value for no magnetic field on **RP235x (14-bit)**.
///
/// This value may vary slightly between modules. Use [`HallAnalog::read_raw`]
/// or [`HallAnalog::calibrate`] to find the actual zero point.
pub const ZERO_FIELD_RAW_14BIT: u16 = 8192;

// ---------------------------------------------------------------------------
// Constante interne sélectionnée automatiquement selon la feature active.
// Utilisée par `read_polarity` pour éviter toute duplication de logique.
// ---------------------------------------------------------------------------

/// Automatically selected resting point for the target.
///
/// - `rp235xa`, `rp235xb` → [`ZERO_FIELD_RAW_14BIT`] (8192)
#[cfg(any(feature = "rp235xa", feature = "rp235xb"))]
const ZERO_FIELD_RAW: u16 = ZERO_FIELD_RAW_14BIT;

// ---------------------------------------------------------------------------
// Types publics
// ---------------------------------------------------------------------------

/// Driver for the OPEN-SMART linear Hall-effect sensor via the RP235x ADC.
///
/// This driver encapsulates an ADC channel and provides asynchronous reads
/// of the sensor's raw value. The raw value is centered around
/// [`ZERO_FIELD_RAW_14BIT`] (8192 on RP235x) when no magnetic field is present.
///
/// The ADC must be created in **async** mode (`Adc::new(p.ADC, Irqs, config)`),
/// since the reads are interrupt-driven.
///
/// # Minimal example
///
/// ```rust,no_run
/// # use embassy_rp::adc::{Adc, Channel};
/// # use embassy_hall_analog::HallAnalog;
/// // let mut sensor = HallAnalog::new(adc, channel);
/// // let raw: u16 = sensor.read_raw().await;
/// ```
pub struct HallAnalog<'d> {
    adc: Adc<'d, Async>,
    channel: Channel<'d>,
}

/// Magnetic polarity of the detected field.
///
/// Detection is based on the deviation of the raw ADC value from the resting point
/// (midscale), with a configurable dead zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MagneticPolarity {
    /// South pole detected; voltage rises above VCC/2.
    SouthPole,
    /// North pole detected; voltage falls below VCC/2.
    NorthPole,
    /// No significant magnetic field detected (within dead zone).
    NoField,
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl<'d> HallAnalog<'d> {
    /// Creates a new `HallAnalog` driver instance.
    ///
    /// # Arguments
    ///
    /// * `adc` — An async ADC peripheral.
    /// * `channel` — The ADC channel connected to the Hall module's AO pin.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use embassy_rp::adc::{Adc, Channel, Config as AdcConfig};
    /// # use embassy_hall_analog::HallAnalog;
    /// // let adc = Adc::new(p.ADC, Irqs, AdcConfig::default());
    /// // let channel = Channel::new_pin(p.PIN_26, embassy_rp::gpio::Pull::None);
    /// // let sensor = HallAnalog::new(adc, channel);
    /// ```
    #[inline]
    pub fn new(adc: Adc<'d, Async>, channel: Channel<'d>) -> Self {
        Self { adc, channel }
    }

    /// Reads the raw ADC value, reporting conversion errors.
    ///
    /// Same as [`HallAnalog::read_raw`], but returns the ADC error instead of
    /// silently falling back to `0`.
    #[inline]
    pub async fn try_read_raw(&mut self) -> Result<u16, AdcError> {
        self.adc.read(&mut self.channel).await
    }

    /// Reads the raw ADC value.
    ///
    /// | Target | Resolution | Range | Resting point |
    /// |---|---|---|---|
    /// | RP235x | 14-bit | `0..=16383` | ~8192 |
    ///
    /// Returns `0` if an ADC error occurs (use [`HallAnalog::try_read_raw`] to
    /// detect errors).
    ///
    /// # Note: Hardware calibration
    ///
    /// Although RP235x is 14-bit, the returned value may be 12-bit (0..4095) depending
    /// on the Hazard3 HAL configuration. Your actual resting point may differ from the
    /// theoretical value (~8192).
    ///
    /// **Example measured on hardware (RP2350A with calibration)**:
    /// - No magnetic field, powered at 3.3V: **~8200** (varies with component tolerance,
    ///   temperature, and supply stability)
    ///
    /// **Recommendation**: Call [`HallAnalog::calibrate()`] at startup to determine
    /// the actual resting point.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use embassy_hall_analog::HallAnalog;
    /// # async fn example(mut sensor: HallAnalog<'_>) {
    /// let raw: u16 = sensor.read_raw().await;
    ///
    /// // Raw to voltage (V) on RP235x, 14-bit
    /// let voltage = raw as f32 * 3.3 / 16383.0;
    ///
    /// // Deviation from resting point (ΔV)
    /// let delta_v = voltage - 1.65;
    ///
    /// // Estimated field in Gauss (49E sensitivity @ 3.3V ≈ 0.92 mV/Gauss)
    /// let field_gauss = delta_v / 0.00092;
    /// # }
    /// ```
    #[inline]
    pub async fn read_raw(&mut self) -> u16 {
        self.try_read_raw().await.unwrap_or(0)
    }

    /// Reads the magnetic field polarity.
    ///
    /// The resting point (`ZERO_FIELD_RAW`) is automatically selected for the
    /// compiled target (`rp235xa`/`rp235xb` → 8192).
    ///
    /// # Arguments
    ///
    /// * `deadband` — Half-width of the dead zone in LSB. Typical value: `200` for 14-bit.
    ///
    ///   | Condition | Result |
    ///   |---|---|
    ///   | `raw > ZERO + deadband` | [`MagneticPolarity::SouthPole`] |
    ///   | `raw < ZERO - deadband` | [`MagneticPolarity::NorthPole`] |
    ///   | otherwise | [`MagneticPolarity::NoField`] |
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use embassy_hall_analog::{HallAnalog, MagneticPolarity};
    /// # async fn example(mut sensor: HallAnalog<'_>) {
    /// match sensor.read_polarity(200).await {
    ///     MagneticPolarity::SouthPole => { /* south pole */ }
    ///     MagneticPolarity::NorthPole => { /* north pole */ }
    ///     MagneticPolarity::NoField   => { /* no field */ }
    /// }
    /// # }
    /// ```
    pub async fn read_polarity(&mut self, deadband: u16) -> MagneticPolarity {
        let raw = self.read_raw().await;

        if raw > ZERO_FIELD_RAW.saturating_add(deadband) {
            MagneticPolarity::SouthPole
        } else if raw < ZERO_FIELD_RAW.saturating_sub(deadband) {
            MagneticPolarity::NorthPole
        } else {
            MagneticPolarity::NoField
        }
    }

    /// Reads the raw ADC and returns signed deviation from the resting point.
    ///
    /// Useful for evaluating relative field intensity independent of polarity,
    /// or for implementing custom threshold logic.
    ///
    /// # Arguments
    ///
    /// * `zero` — Calibrated resting point in LSB. Use [`ZERO_FIELD_RAW_14BIT`] (RP235x)
    ///   or a measured value from your module for better precision.
    ///
    /// # Returns
    ///
    /// * `i32` — Signed deviation in ADC LSB.
    ///   - Positive → south pole
    ///   - Negative → north pole
    ///   - Close to zero → no field
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use embassy_hall_analog::{HallAnalog, ZERO_FIELD_RAW_14BIT};
    /// # async fn example(mut sensor: HallAnalog<'_>) {
    /// let deviation = sensor.read_deviation(ZERO_FIELD_RAW_14BIT).await;
    /// // deviation > 0: south pole, deviation < 0: north pole
    /// # }
    /// ```
    pub async fn read_deviation(&mut self, zero: u16) -> i32 {
        let raw = self.read_raw().await;
        raw as i32 - zero as i32
    }

    /// Automatically calibrates the zero point.
    ///
    /// Averages `samples` readings with a 100-microsecond delay between reads
    /// to allow ADC stabilization and avoid repeated noise peaks.
    ///
    /// # Arguments
    ///
    /// * `samples` — Number of samples to average (e.g., 64 for high precision).
    ///   If `0`, the theoretical resting point [`ZERO_FIELD_RAW_14BIT`] is returned
    ///   without reading the ADC.
    ///
    /// # Returns
    ///
    /// The measured zero point.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use embassy_hall_analog::HallAnalog;
    /// # async fn example(mut sensor: HallAnalog<'_>) {
    /// // Calibrate with 64 samples for high precision
    /// let zero_offset = sensor.calibrate(64).await;
    /// // Then use zero_offset for precise readings
    /// let deviation = sensor.read_deviation(zero_offset).await;
    /// # }
    /// ```
    pub async fn calibrate(&mut self, samples: u8) -> u16 {
        if samples == 0 {
            return ZERO_FIELD_RAW;
        }

        let mut total: u32 = 0;
        for _ in 0..samples {
            total += self.read_raw().await as u32;
            // Small delay to allow the ADC to stabilize
            // and avoid reading the same noise peak repeatedly.
            Timer::after_micros(100).await;
        }
        (total / samples as u32) as u16
    }
}