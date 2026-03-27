//! Type-safe physical units for CNC operations.
//!
//! Uses phantom types to enforce unit correctness at compile time.
//! Ported from cnc-sender with serde support added for JSON config.

use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

/// Sealed trait for distance units.
pub trait DistanceUnit: private::Sealed + Copy + Clone + std::fmt::Debug + Default {
    const NAME: &'static str;
    const TO_MM: f64;
}

mod private {
    pub trait Sealed {}
    impl Sealed for super::Millimeters {}
    impl Sealed for super::Inches {}
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Millimeters;

impl DistanceUnit for Millimeters {
    const NAME: &'static str = "mm";
    const TO_MM: f64 = 1.0;
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inches;

impl DistanceUnit for Inches {
    const NAME: &'static str = "in";
    const TO_MM: f64 = 25.4;
}

/// Type-safe distance value with unit phantom type.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Distance<U: DistanceUnit> {
    value: f64,
    _unit: PhantomData<U>,
}

impl<U: DistanceUnit> Serialize for Distance<U> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.value.serialize(serializer)
    }
}

impl<'de, U: DistanceUnit> Deserialize<'de> for Distance<U> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        f64::deserialize(deserializer).map(Self::new)
    }
}

impl<U: DistanceUnit> Distance<U> {
    #[must_use]
    pub const fn new(value: f64) -> Self {
        Self {
            value,
            _unit: PhantomData,
        }
    }

    #[must_use]
    pub const fn value(self) -> f64 {
        self.value
    }

    #[must_use]
    pub fn to_mm(self) -> f64 {
        self.value * U::TO_MM
    }

    #[must_use]
    pub const fn unit_name() -> &'static str {
        U::NAME
    }

    #[must_use]
    pub const fn zero() -> Self {
        Self::new(0.0)
    }

    #[must_use]
    pub fn abs(self) -> Self {
        Self::new(self.value.abs())
    }
}

impl<U: DistanceUnit> std::ops::Neg for Distance<U> {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self::new(-self.value)
    }
}

impl<U: DistanceUnit> Default for Distance<U> {
    fn default() -> Self {
        Self::zero()
    }
}

impl<U: DistanceUnit> std::ops::Add for Distance<U> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.value + rhs.value)
    }
}

impl<U: DistanceUnit> std::ops::Sub for Distance<U> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.value - rhs.value)
    }
}

impl<U: DistanceUnit> std::ops::Mul<f64> for Distance<U> {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self::Output {
        Self::new(self.value * rhs)
    }
}

impl<U: DistanceUnit> std::fmt::Display for Distance<U> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.3} {}", self.value, U::NAME)
    }
}

/// Feed rate in units per minute.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FeedRate<U: DistanceUnit> {
    value: f64,
    _unit: PhantomData<U>,
}

impl<U: DistanceUnit> Serialize for FeedRate<U> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.value.serialize(serializer)
    }
}

impl<'de, U: DistanceUnit> Deserialize<'de> for FeedRate<U> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        f64::deserialize(deserializer).map(Self::new)
    }
}

impl<U: DistanceUnit> FeedRate<U> {
    #[must_use]
    pub const fn new(value: f64) -> Self {
        Self {
            value,
            _unit: PhantomData,
        }
    }

    #[must_use]
    pub const fn value(self) -> f64 {
        self.value
    }

    #[must_use]
    pub fn to_mm_per_min(self) -> f64 {
        self.value * U::TO_MM
    }
}

impl<U: DistanceUnit> Default for FeedRate<U> {
    fn default() -> Self {
        Self::new(0.0)
    }
}

impl<U: DistanceUnit> std::fmt::Display for FeedRate<U> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.0} {}/min", self.value, U::NAME)
    }
}

/// Spindle speed in RPM.
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct SpindleSpeed(pub u32);

impl SpindleSpeed {
    #[must_use]
    pub const fn new(rpm: u32) -> Self {
        Self(rpm)
    }

    #[must_use]
    pub const fn rpm(self) -> u32 {
        self.0
    }

    pub const OFF: Self = Self(0);
}

impl std::fmt::Display for SpindleSpeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} RPM", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_operations() {
        let d1 = Distance::<Millimeters>::new(10.0);
        let d2 = Distance::<Millimeters>::new(5.0);
        assert_eq!((d1 + d2).value(), 15.0);
        assert_eq!((d1 - d2).value(), 5.0);
        assert_eq!((d1 * 2.0).value(), 20.0);
    }

    #[test]
    fn distance_conversion() {
        let inches = Distance::<Inches>::new(1.0);
        assert!((inches.to_mm() - 25.4).abs() < f64::EPSILON);
    }

    #[test]
    fn feed_rate_display() {
        let feed = FeedRate::<Millimeters>::new(1000.0);
        assert_eq!(format!("{feed}"), "1000 mm/min");
    }

    #[test]
    fn distance_serde_roundtrip() {
        let d = Distance::<Millimeters>::new(42.5);
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(json, "42.5");
        let d2: Distance<Millimeters> = serde_json::from_str(&json).unwrap();
        assert_eq!(d, d2);
    }

    #[test]
    fn spindle_speed_serde() {
        let s = SpindleSpeed::new(12000);
        let json = serde_json::to_string(&s).unwrap();
        let s2: SpindleSpeed = serde_json::from_str(&json).unwrap();
        assert_eq!(s, s2);
    }

    #[test]
    fn feed_rate_serde_roundtrip() {
        let feed = FeedRate::<Millimeters>::new(1500.0);
        let json = serde_json::to_string(&feed).unwrap();
        assert_eq!(json, "1500.0");
        let feed2: FeedRate<Millimeters> = serde_json::from_str(&json).unwrap();
        assert_eq!(feed, feed2);
    }

    #[test]
    fn distance_abs() {
        let d = Distance::<Millimeters>::new(-7.5);
        assert_eq!(d.abs().value(), 7.5);
    }

    #[test]
    fn distance_default() {
        let d = Distance::<Millimeters>::default();
        assert_eq!(d.value(), 0.0);
    }

    #[test]
    fn distance_display() {
        let d = Distance::<Millimeters>::new(3.2568);
        assert_eq!(format!("{d}"), "3.257 mm");
        let d_in = Distance::<Inches>::new(1.0);
        assert_eq!(format!("{d_in}"), "1.000 in");
    }

    #[test]
    fn distance_neg() {
        let d = Distance::<Millimeters>::new(5.0);
        assert_eq!((-d).value(), -5.0);
    }

    #[test]
    fn distance_unit_name() {
        assert_eq!(Distance::<Millimeters>::unit_name(), "mm");
        assert_eq!(Distance::<Inches>::unit_name(), "in");
    }

    #[test]
    fn distance_zero() {
        let z = Distance::<Millimeters>::zero();
        assert_eq!(z.value(), 0.0);
    }

    #[test]
    fn feed_rate_default() {
        let f = FeedRate::<Millimeters>::default();
        assert_eq!(f.value(), 0.0);
    }

    #[test]
    fn feed_rate_to_mm_per_min() {
        let f_mm = FeedRate::<Millimeters>::new(100.0);
        assert_eq!(f_mm.to_mm_per_min(), 100.0);
        let f_in = FeedRate::<Inches>::new(1.0);
        assert!((f_in.to_mm_per_min() - 25.4).abs() < f64::EPSILON);
    }

    #[test]
    fn feed_rate_value() {
        let f = FeedRate::<Millimeters>::new(42.0);
        assert_eq!(f.value(), 42.0);
    }

    #[test]
    fn spindle_speed_display() {
        let s = SpindleSpeed::new(12000);
        assert_eq!(format!("{s}"), "12000 RPM");
    }
}
