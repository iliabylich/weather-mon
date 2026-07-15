#[derive(Debug, Clone, Copy)]
pub struct Args {
    pub(crate) mode: RunMode,
}

#[derive(Debug, Clone, Copy)]
pub enum RunMode {
    Systemd,
    Dev,
}

const USAGE: &str = "
Usage: weather-mon [--systemd|--dev]

--systemd: runs daemon under systemd and expects it to pass a socket via sd_listen_fds()
    --dev: starts listening on $XDG_RUNTIME_DIR/weather-mon-dev.sock
";
fn print_usage_and_exit() -> ! {
    eprintln!("{USAGE}");
    std::process::exit(1);
}

impl Args {
    pub(crate) fn parse() -> Self {
        let mode = std::env::args()
            .nth(1)
            .unwrap_or_else(|| print_usage_and_exit());
        let mode = match mode.as_str() {
            "--systemd" => RunMode::Systemd,
            "--dev" => RunMode::Dev,
            other => {
                eprintln!("Unknown argument {other:?}");
                print_usage_and_exit();
            }
        };

        Self { mode }
    }
}
