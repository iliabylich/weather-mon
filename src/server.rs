use std::{
    io::ErrorKind,
    os::fd::{AsFd, OwnedFd},
};

use crate::args::RunMode;
use anyhow::{Context, Result, bail};
use rustix::{
    fs::{Mode, OFlags, fcntl_getfl, fcntl_setfl},
    net::{AddressFamily, SocketAddrUnix, SocketType},
};
use tokio::net::{UnixListener, UnixStream};

pub struct Server {
    listener: UnixListener,
    clientid: u64,
}

impl Server {
    pub(crate) fn new(mode: RunMode) -> Result<Self> {
        let fd = match mode {
            RunMode::Systemd => {
                let (_, fd) = sd_listen_fds::get()
                    .context("sd_listen_fds failed()")?
                    .into_iter()
                    .next()
                    .context("sd_listen_fds() returned no FDs")?;
                log::trace!("Using systemd-provided socket");
                fd.into_std()
            }

            RunMode::Dev => {
                let xdg_runtime_dir =
                    std::env::var("XDG_RUNTIME_DIR").context("no $XDG_RUNTIME_DIR")?;
                let path = format!("{xdg_runtime_dir}/weather-mon-dev.sock");
                log::trace!("Listening on {path}");
                socket_at(&path)?
            }
        };
        set_nonblocking(&fd)?;
        let listener = UnixListener::from_std(std::os::unix::net::UnixListener::from(fd))?;

        Ok(Self {
            listener,
            clientid: 0,
        })
    }

    pub(crate) async fn accept(&mut self) -> Result<(u64, UnixStream)> {
        let (client, _) = self.listener.accept().await?;
        self.clientid = self.clientid.wrapping_add(1);
        Ok((self.clientid, client))
    }
}

fn socket_at(socket_path: &str) -> Result<OwnedFd> {
    const SOMAXCONN: i32 = 4_096;

    let addr = SocketAddrUnix::new(socket_path)?;
    ensure_addr_is_free(&addr)?;

    let fd = rustix::net::socket(AddressFamily::UNIX, SocketType::STREAM, None)?;
    match rustix::fs::unlink(socket_path) {
        Ok(()) => {}
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => return Err(err.into()),
    }
    rustix::net::bind(&fd, &addr)?;
    rustix::fs::chmod(socket_path, Mode::from_raw_mode(0o666))?;
    rustix::net::listen(&fd, SOMAXCONN)?;

    Ok(fd)
}

fn ensure_addr_is_free(addr: &SocketAddrUnix) -> Result<()> {
    let fd = rustix::net::socket(AddressFamily::UNIX, SocketType::STREAM, None)?;
    if rustix::net::connect(&fd, addr).is_ok() {
        bail!("already running")
    }
    Ok(())
}

fn set_nonblocking(fd: impl AsFd) -> Result<()> {
    let mut flags = fcntl_getfl(&fd)?;
    flags.insert(OFlags::NONBLOCK);
    fcntl_setfl(fd, flags)?;
    Ok(())
}
