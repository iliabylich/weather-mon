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
#![expect(clippy::redundant_pub_crate)]

use anyhow::{Context, Result};
use core::time::Duration;
use tokio::{
    select,
    time::{Instant, interval_at},
};

mod args;
mod clients;
mod location;
mod server;
mod weather;

use args::Args;
use clients::Clients;
use server::Server;
use weather::WeatherClient;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_default_env()
        .format_target(false)
        .write_style(env_logger::WriteStyle::Always)
        .init();
    let args = Args::parse();
    log::trace!("Running with args {args:?}");

    let mut wclient = WeatherClient::new()?;
    let mut server = Server::new(args.mode)?;
    let mut clients = Clients::new();
    let refresh_every = Duration::from_mins(30);
    wclient.fetch_initial_value().await?;
    let mut timer = interval_at(
        Instant::now()
            .checked_add(refresh_every)
            .context("refresh internal is too large")?,
        refresh_every,
    );

    loop {
        select! {
            Ok((id, client)) = server.accept() => {
                log::trace!("new client: {id}");
                clients.insert(id, client);
            }

            _ = timer.tick() => {
                if let Some(weather) = wclient.refresh().await {
                    clients.broadcast(&weather.serialize()).await;
                }
            }

            Some(id) = clients.recv() => {
                log::trace!("received a request from client {id}");
                if let Some(weather) = wclient.current() {
                    clients.send(id, &weather.serialize()).await;
                }
            }
        }
    }
}
