use crate::location::LocationClient;
use anyhow::{Context, Result, bail};
use chrono::NaiveTime;
use core::time::Duration;
use reqwest::{Client, ClientBuilder, StatusCode, retry};
use serde::Deserialize;
use tokio::time::{sleep, timeout};
use weather_mon::{CurrentWeather, DailyWeather, HourlyWeather, Weather};

#[derive(Deserialize, Debug)]
struct FullResponse {
    current: CurrentResponse,
    hourly: HourlyResponse,
    daily: DailyResponse,
}

#[derive(Deserialize, Debug)]
struct CurrentResponse {
    temperature_2m: f32,
    weather_code: u8,
}

#[derive(Deserialize, Debug)]
struct HourlyResponse {
    time: Vec<i64>,
    temperature_2m: Vec<f32>,
    weather_code: Vec<u8>,
}

#[derive(Deserialize, Debug)]
struct DailyResponse {
    time: Vec<i64>,
    temperature_2m_min: Vec<f32>,
    temperature_2m_max: Vec<f32>,
    weather_code: Vec<u8>,
}

pub struct WeatherClient {
    location_client: LocationClient,
    client: Client,
    cached: Option<Weather>,
}

impl WeatherClient {
    const INITIAL_ATTEMPTS: u8 = 10;
    const REFRESH_ATTEMPTS: u8 = 10;

    pub(crate) fn new() -> Result<Self> {
        Ok(Self {
            location_client: LocationClient::new()?,
            client: ClientBuilder::new()
                .timeout(Duration::from_secs(2))
                .retry(retry::for_host("api.open-meteo.com").max_retries_per_request(0))
                .build()
                .context("failed to build Weather's client")?,
            cached: None,
        })
    }

    async fn try_get(&self) -> Result<Weather> {
        let (lat, lng) = self.location_client.get().await?;
        let lat = lat.to_string();
        let lng = lng.to_string();

        let tz = iana_time_zone::get_timezone()?;
        log::trace!("tz={tz}");

        let res = self
            .client
            .get("https://api.open-meteo.com/v1/forecast")
            .query(&[
                ("latitude", lat.as_str()),
                ("longitude", lng.as_str()),
                ("current", "temperature_2m,weather_code"),
                ("hourly", "temperature_2m,weather_code"),
                (
                    "daily",
                    "temperature_2m_min,temperature_2m_max,weather_code",
                ),
                ("timezone", tz.as_str()),
                ("timeformat", "unixtime"),
            ])
            .send()
            .await?;

        let status = res.status();
        let body = res
            .bytes()
            .await
            .with_context(|| format!("failed to read body (status: {status:?}"))?;
        let body = String::from_utf8(body.into()).map_err(|err| {
            anyhow::anyhow!(
                "non-utf8 response, status: {status:?}, body(lossy): {:?}",
                String::from_utf8_lossy(&err.into_bytes())
            )
        })?;

        if status != StatusCode::OK {
            bail!("weather API returned status {status:?}, body: {body:?}");
        }

        let res: FullResponse = serde_json::from_str(&body)
            .with_context(|| format!("malformed JSON response: {body:?}"))?;

        let weather = Weather::try_from(res)?;
        log::trace!("{weather:#?}");

        Ok(weather)
    }

    async fn refresh_once(&mut self) -> Option<Weather> {
        match timeout(Duration::from_secs(5), self.try_get()).await {
            Ok(Ok(weather)) => {
                self.cached = Some(weather);
                Some(weather)
            }
            Ok(Err(err)) => {
                log::error!("failed to refresh weather: {err:?}");
                None
            }
            Err(_elapsed) => {
                log::error!("failed to refresh weather: timeout error");
                None
            }
        }
    }

    pub(crate) async fn refresh(&mut self) -> Option<Weather> {
        for i in 1..=Self::REFRESH_ATTEMPTS {
            log::trace!("refreshing weather ({i})");
            if let Some(weather) = self.refresh_once().await {
                return Some(weather);
            }
            sleep(Duration::from_secs(3)).await;
        }
        log::error!(
            "failed to fetch weather after {} attempts",
            Self::REFRESH_ATTEMPTS
        );
        None
    }

    pub(crate) async fn fetch_initial_value(&mut self) -> Result<()> {
        for i in 1..=Self::INITIAL_ATTEMPTS {
            log::trace!("fetching initial weather ({i})");
            if self.refresh_once().await.is_some() {
                return Ok(());
            }
            sleep(Duration::from_secs(3)).await;
        }

        bail!(
            "failed to fetch weather after {} attempts",
            Self::INITIAL_ATTEMPTS
        )
    }

    pub(crate) const fn current(&self) -> Option<Weather> {
        self.cached
    }
}

impl From<CurrentResponse> for CurrentWeather {
    fn from(res: CurrentResponse) -> Self {
        Self {
            t: res.temperature_2m,
            code: res.weather_code,
        }
    }
}
impl TryFrom<HourlyResponse> for [HourlyWeather; Weather::HOURLIES] {
    type Error = anyhow::Error;

    fn try_from(res: HourlyResponse) -> Result<Self> {
        let now = chrono::Local::now().timestamp();
        res.time
            .into_iter()
            .zip(res.temperature_2m)
            .zip(res.weather_code)
            .map(|((time, t), code)| HourlyWeather { time, t, code })
            .filter(|hw| hw.time > now)
            .take(Weather::HOURLIES)
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| anyhow::anyhow!("not enough hourly data returned from the API"))
    }
}
impl TryFrom<DailyResponse> for [DailyWeather; Weather::DAILIES] {
    type Error = anyhow::Error;

    fn try_from(res: DailyResponse) -> Result<Self> {
        let now = chrono::Local::now();
        let beginning_of_day = now
            .with_time(
                NaiveTime::from_hms_nano_opt(0, 0, 0, 0).context("failed to build zero time")?,
            )
            .earliest()
            .context("failed to calculate beginning of day")?
            .timestamp();
        res.time
            .into_iter()
            .zip(res.temperature_2m_min)
            .zip(res.temperature_2m_max)
            .zip(res.weather_code)
            .map(|(((time, t_min), t_max), code)| DailyWeather {
                time,
                t_min,
                t_max,
                code,
            })
            .filter(|dw| dw.time >= beginning_of_day)
            .take(Weather::DAILIES)
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| anyhow::anyhow!("not enough daily data returned from the API"))
    }
}
impl TryFrom<FullResponse> for Weather {
    type Error = anyhow::Error;

    fn try_from(res: FullResponse) -> Result<Self> {
        Ok(Self {
            current: res.current.into(),
            hourly: res.hourly.try_into()?,
            daily: res.daily.try_into()?,
        })
    }
}
