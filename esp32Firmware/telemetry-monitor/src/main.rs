use monoprop_telemetry_monitor::{Schema, decode_datagram};
use serde::Serialize;
use std::{
    env,
    io::{self, Write},
    net::UdpSocket,
    process::ExitCode,
};

const HELP: &str = "monoprop-telemetry-monitor [--bind 0.0.0.0:9999] [--schema modern|legacy|echo] [--pretty] [--max-packets N]\nReceive-only Postcard/COBS telemetry. Defaults to the Teensy's modern TelemetryFrame.\nUSB Serial diagnostics are text and remain in PuTTY. No control packets are sent.";

#[derive(Serialize)]
struct Record {
    source: std::net::SocketAddr,
    #[serde(flatten)]
    message: monoprop_telemetry_monitor::Telemetry,
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut bind = "0.0.0.0:9999".to_owned();
    let mut schema = Schema::Modern;
    let mut pretty = false;
    let mut max_packets: usize = 0;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => bind = args.next().ok_or("--bind requires an address:port")?,
            "--schema" => {
                schema = args
                    .next()
                    .ok_or("--schema requires modern, legacy or echo")?
                    .parse()?
            }
            "--pretty" => pretty = true,
            "--max-packets" => {
                max_packets = args
                    .next()
                    .ok_or("--max-packets requires a count")?
                    .parse()?
            }
            "--help" | "-h" => {
                println!("{HELP}");
                return Ok(());
            }
            _ => return Err(format!("unknown argument {arg:?}\n{HELP}").into()),
        }
    }
    let socket = UdpSocket::bind(&bind)?;
    eprintln!(
        "Listening on {} (UDP), schema={schema:?}, Postcard + COBS",
        socket.local_addr()?
    );
    eprintln!("Receive-only; waiting for configured Wi-Fi and Teensy telemetry. Ctrl+C to stop.");
    // Read the whole datagram so oversized input cannot be mistaken for a valid prefix.
    let mut buffer = [0; 65535];
    let mut stdout = io::stdout().lock();
    let mut packets = 0;
    loop {
        let (len, source) = match socket.recv_from(&mut buffer) {
            Ok(packet) => packet,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        };
        packets += 1;
        match decode_datagram(&buffer[..len], schema) {
            Ok(message) => {
                let record = Record { source, message };
                if pretty {
                    serde_json::to_writer_pretty(&mut stdout, &record)?;
                } else {
                    serde_json::to_writer(&mut stdout, &record)?;
                }
                writeln!(stdout)?;
                stdout.flush()?;
            }
            Err(error) => eprintln!(
                "Rejected {len} bytes from {source}: {error:?}; expected {schema:?} Postcard/COBS"
            ),
        }
        if max_packets != 0 && packets >= max_packets {
            return Ok(());
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Monitor error: {error}");
            ExitCode::FAILURE
        }
    }
}
