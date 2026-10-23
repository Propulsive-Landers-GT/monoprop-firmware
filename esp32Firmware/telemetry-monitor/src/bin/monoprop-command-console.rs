use monoprop_telemetry_monitor::commands::encode_command;
use monoprop_telemetry_monitor::encode_echo;
use std::{
    env,
    io::{self, BufRead, Write},
    net::{SocketAddr, UdpSocket},
    process::ExitCode,
};

const HELP: &str = r#"Commands (nothing is transmitted automatically):
  send {"kind":"text","command":"Disarm"}
  send {"kind":"control","command":{...shared ControlCommand fields...}}
  help
  quit
Text command names: Ready, Arm, Disarm, Launch, Hover, Land, Abort, Kill,
                    MissionStandard, MissionTetheredHover.
WARNING: These commands can operate flight hardware. Send only intentional commands.
UDP send success is NOT an acknowledgement from the ESP32 or Teensy.
Closing this console does NOT disarm the vehicle."#;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut bind = "0.0.0.0:10000".to_string();
    let mut target: SocketAddr = "192.168.1.50:8888".parse()?;
    let mut echo = false;
    let mut sequence: u32 = 0;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--echo" => echo = true,
            "--bind" => bind = args.next().ok_or("--bind requires address:port")?,
            "--target" => target = args.next().ok_or("--target requires IP:port")?.parse()?,
            "--help" | "-h" => {
                println!(
                    "monoprop-command-console [--bind 0.0.0.0:10000] [--target IP:8888] [--echo]\n--echo: bench firmware only; send <message> encodes an EchoMessage instead of a flight command.\n{HELP}"
                );
                return Ok(());
            }
            _ => return Err(format!("unknown argument {arg:?}").into()),
        }
    }
    let socket = UdpSocket::bind(bind)?;
    println!(
        "Sender bound to {} -> {target} (UDP, Postcard/COBS)",
        socket.local_addr()?
    );
    let help = if echo {
        "ECHO BENCH TEST: use only with featheresp32-echo firmware (UART disabled).\nType: send hello ESP32\nThe receiver must use --schema echo on UDP 9999. Nothing is sent automatically.\nCommands: send <message>, help, quit. Receipt confirms the ESP32 round trip, not Teensy execution."
    } else {
        HELP
    };
    println!(
        "Destination is configured, not verified. Replies arrive separately on UDP 9999.\n{help}"
    );
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    loop {
        print!("send> ");
        io::stdout().flush()?;
        let Some(line) = lines.next() else {
            return Ok(());
        };
        let line = line?;
        match line.trim() {
            "quit" | "exit" => return Ok(()),
            "help" => println!("{help}"),
            "" => (),
            text => {
                let Some(json) = text.strip_prefix("send ") else {
                    eprintln!("Nothing sent. Use: send <JSON>, help, or quit.");
                    continue;
                };
                let encoded = if echo {
                    encode_echo(sequence, json)
                } else {
                    encode_command(json)
                };
                match encoded {
                    Ok(bytes) => match socket.send_to(&bytes, target) {
                        Ok(len) if len == bytes.len() => {
                            println!(
                                "Submitted {len} bytes to {target}; sequence={sequence}; delivery/execution unconfirmed."
                            );
                            sequence = sequence.wrapping_add(1);
                        }
                        Ok(len) => eprintln!("Incomplete send: {len}/{} bytes", bytes.len()),
                        Err(error) => eprintln!("Send failed: {error}"),
                    },
                    Err(error) => eprintln!("Nothing sent: {error}"),
                }
            }
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Sender error: {error}");
            ExitCode::FAILURE
        }
    }
}
