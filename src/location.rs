use anyhow::{Context, Result};
use core::time::Duration;
use reqwest::{Client, ClientBuilder, retry};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct Response {
    location: Vec<Location>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct Location {
    lat: f64,
    lng: f64,
    source: Source,
}

#[derive(Deserialize, Debug, PartialEq)]
enum Source {
    #[serde(rename = "freegeoip")]
    FreeGeoIP,
    #[serde(rename = "ipapi")]
    IpAPI,
    #[serde(rename = "ipwhois")]
    IpWhoIs,
}

pub(crate) struct LocationClient {
    client: Client,
}

impl LocationClient {
    pub(crate) fn new() -> Result<Self> {
        Ok(Self {
            client: ClientBuilder::new()
                .timeout(Duration::from_secs(2))
                .retry(retry::for_host("myip.ibylich.dev").max_retries_per_request(0))
                .build()
                .context("failed to build Location's client")?,
        })
    }

    pub(crate) async fn get(&self) -> Result<(f64, f64)> {
        let response: Response = self
            .client
            .get("https://myip.ibylich.dev")
            .send()
            .await?
            .json()
            .await?;

        let get = |source: Source| -> Option<(f64, f64)> {
            response
                .location
                .iter()
                .find(|loc| loc.source == source)
                .map(|loc| (loc.lat, loc.lng))
        };

        let (lat, lng) = get(Source::FreeGeoIP)
            .or_else(|| get(Source::IpAPI))
            .or_else(|| get(Source::IpWhoIs))
            .context("failed to get at least one location")?;
        log::trace!("lat={lat} lng={lng}");

        Ok((lat, lng))
    }
}
