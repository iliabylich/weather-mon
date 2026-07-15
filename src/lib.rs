#![warn(trivial_casts)]
#![warn(trivial_numeric_casts)]
#![warn(unused_qualifications)]
#![warn(deprecated_in_future)]
#![warn(unused_lifetimes)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::panic)]
#![warn(clippy::indexing_slicing)]
#![warn(clippy::arithmetic_side_effects)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::std_instead_of_alloc)]
#![warn(clippy::std_instead_of_core)]

#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct Weather {
    pub current: CurrentWeather,
    pub hourly: [HourlyWeather; Self::HOURLIES],
    pub daily: [DailyWeather; Self::DAILIES],
}

#[derive(Clone, Copy, PartialEq)]
#[must_use]
pub struct CurrentWeather {
    pub t: f32,
    pub code: u8,
}

#[derive(Clone, Copy, PartialEq)]
#[must_use]
pub struct HourlyWeather {
    pub time: i64,
    pub t: f32,
    pub code: u8,
}

#[derive(Clone, Copy, PartialEq)]
#[must_use]
pub struct DailyWeather {
    pub time: i64,
    pub t_min: f32,
    pub t_max: f32,
    pub code: u8,
}

impl core::fmt::Debug for CurrentWeather {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "CurrentW({} - {})", self.t, self.code)
    }
}
impl core::fmt::Debug for HourlyWeather {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "HourlyW({} - {} - {})", self.time, self.t, self.code)
    }
}
impl core::fmt::Debug for DailyWeather {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "DaylyW({} - {}..{} - {})",
            self.time, self.t_min, self.t_max, self.code
        )
    }
}

impl CurrentWeather {
    pub const BYTESIZE: usize = 8;

    #[must_use]
    pub fn serialize(self) -> [u8; Self::BYTESIZE] {
        let mut buf = [0; _];
        buf[0..4].copy_from_slice(&self.t.to_be_bytes());
        buf[4] = self.code;
        buf
    }

    pub const fn deserialize(buf: [u8; Self::BYTESIZE]) -> Self {
        Self {
            t: f32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]),
            code: buf[4],
        }
    }

    #[cfg(test)]
    fn random() -> Self {
        Self {
            t: rand::random(),
            code: rand::random(),
        }
    }
}
impl HourlyWeather {
    pub const BYTESIZE: usize = 16;

    #[must_use]
    pub fn serialize(self) -> [u8; Self::BYTESIZE] {
        let mut buf = [0; _];
        buf[0..8].copy_from_slice(&self.time.to_be_bytes());
        buf[8..12].copy_from_slice(&self.t.to_be_bytes());
        buf[12] = self.code;
        buf
    }

    pub const fn deserialize(buf: [u8; Self::BYTESIZE]) -> Self {
        Self {
            time: i64::from_be_bytes([
                buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
            ]),
            t: f32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]),
            code: buf[12],
        }
    }

    #[cfg(test)]
    fn random() -> Self {
        Self {
            time: rand::random(),
            t: rand::random(),
            code: rand::random(),
        }
    }
}
impl DailyWeather {
    pub const BYTESIZE: usize = 24;

    #[must_use]
    pub fn serialize(self) -> [u8; Self::BYTESIZE] {
        let mut buf = [0; _];
        buf[0..8].copy_from_slice(&self.time.to_be_bytes());
        buf[8..12].copy_from_slice(&self.t_min.to_be_bytes());
        buf[12..16].copy_from_slice(&self.t_max.to_be_bytes());
        buf[16] = self.code;
        buf
    }

    pub const fn deserialize(buf: [u8; Self::BYTESIZE]) -> Self {
        Self {
            time: i64::from_be_bytes([
                buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
            ]),
            t_min: f32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]),
            t_max: f32::from_be_bytes([buf[12], buf[13], buf[14], buf[15]]),
            code: buf[16],
        }
    }

    #[cfg(test)]
    fn random() -> Self {
        Self {
            time: rand::random(),
            t_min: rand::random(),
            t_max: rand::random(),
            code: rand::random(),
        }
    }
}

impl Weather {
    pub const HOURLIES: usize = 10;
    pub const DAILIES: usize = 6;

    pub const BYTESIZE: usize = (CurrentWeather::BYTESIZE
        + HourlyWeather::BYTESIZE * Self::HOURLIES
        + DailyWeather::BYTESIZE * Self::DAILIES);

    #[must_use]
    #[expect(clippy::indexing_slicing, clippy::arithmetic_side_effects)]
    pub fn serialize(self) -> [u8; Self::BYTESIZE] {
        let mut buf = [0; _];
        let mut offset = 0;

        buf[offset..offset + CurrentWeather::BYTESIZE].copy_from_slice(&self.current.serialize());
        offset += CurrentWeather::BYTESIZE;

        for hourly in self.hourly {
            buf[offset..offset + HourlyWeather::BYTESIZE].copy_from_slice(&hourly.serialize());
            offset += HourlyWeather::BYTESIZE;
        }

        for daily in self.daily {
            buf[offset..offset + DailyWeather::BYTESIZE].copy_from_slice(&daily.serialize());
            offset += DailyWeather::BYTESIZE;
        }

        buf
    }

    #[expect(clippy::arithmetic_side_effects)]
    pub fn deserialize(buf: [u8; Self::BYTESIZE]) -> Self {
        struct Cursor<const N: usize> {
            buf: [u8; N],
            offset: usize,
        }
        impl<const N: usize> Cursor<N> {
            fn take<const M: usize>(&mut self) -> [u8; M] {
                let mut out = [0; M];
                out.copy_from_slice(&self.buf[self.offset..self.offset + M]);
                self.offset += M;
                out
            }
        }

        let mut cursor = Cursor { buf, offset: 0 };

        Self {
            current: CurrentWeather::deserialize(cursor.take()),
            hourly: core::array::from_fn(|_| HourlyWeather::deserialize(cursor.take())),
            daily: core::array::from_fn(|_| DailyWeather::deserialize(cursor.take())),
        }
    }

    #[cfg(test)]
    fn random() -> Self {
        Self {
            current: CurrentWeather::random(),
            hourly: std::array::from_fn(|_| HourlyWeather::random()),
            daily: std::array::from_fn(|_| DailyWeather::random()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_current_weather() {
        let w = CurrentWeather::random();
        assert_eq!(CurrentWeather::deserialize(w.serialize()), w);
    }

    #[test]
    fn test_serialize_hourly_weather() {
        let w = HourlyWeather::random();
        assert_eq!(HourlyWeather::deserialize(w.serialize()), w);
    }

    #[test]
    fn test_serialize_daily_weather() {
        let w = DailyWeather::random();
        assert_eq!(DailyWeather::deserialize(w.serialize()), w);
    }

    #[test]
    fn test_serialize_weather() {
        let w = Weather::random();
        assert_eq!(Weather::deserialize(w.serialize()), w);
    }
}
