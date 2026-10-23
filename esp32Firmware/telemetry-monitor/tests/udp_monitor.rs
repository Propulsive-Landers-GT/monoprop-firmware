use monoprop_common::TelemetryFrame;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::UdpSocket,
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

struct Monitor(Child);

impl Drop for Monitor {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn sender_transmits_only_explicit_send_and_uses_shared_postcard_command() {
    let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    for (input, should_send) in [
        ("help\nhello\nquit\n", false),
        (
            "send {\"kind\":\"text\",\"command\":\"Disarm\"}\nquit\n",
            true,
        ),
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_monoprop-command-console"))
            .args([
                "--bind",
                "127.0.0.1:0",
                "--target",
                &receiver.local_addr().unwrap().to_string(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let mut bytes = [0; 1024];
        if should_send {
            let (len, _) = receiver.recv_from(&mut bytes).unwrap();
            assert_eq!(bytes[len - 1], 0);
            let command: monoprop_common::TextCommand =
                postcard::from_bytes_cobs(&mut bytes[..len]).unwrap();
            assert_eq!(command, monoprop_common::TextCommand::Disarm);
        } else {
            let error = receiver.recv_from(&mut bytes).unwrap_err();
            assert!(matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ));
        }
    }
}

#[test]
fn executable_rejects_bad_packet_then_prints_shared_schema_as_json() {
    let mut monitor = Monitor(
        Command::new(env!("CARGO_BIN_EXE_monoprop-telemetry-monitor"))
            .args(["--bind", "127.0.0.1:0", "--max-packets", "2"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let stderr = monitor.0.stderr.take().unwrap();
    let (ready_tx, ready_rx) = mpsc::channel();
    let logs = thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut first_line = String::new();
        reader.read_line(&mut first_line).unwrap();
        ready_tx.send(first_line).unwrap();
        let mut remaining = String::new();
        reader.read_to_string(&mut remaining).unwrap();
        remaining
    });
    let startup = ready_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let destination = startup.split_whitespace().nth(2).unwrap();
    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    sender.send_to(&[255, 1, 0], destination).unwrap();
    let frame = TelemetryFrame {
        sequence: 987,
        ..Default::default()
    };
    sender
        .send_to(&postcard::to_stdvec_cobs(&frame).unwrap(), destination)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = monitor.0.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(
            Instant::now() < deadline,
            "receiver did not process both packets"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let mut output = String::new();
    monitor
        .0
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut output)
        .unwrap();
    let json: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(json["schema"], "modern");
    assert_eq!(json["telemetry"]["sequence"], 987);
    assert_eq!(json["source"], sender.local_addr().unwrap().to_string());
    assert!(logs.join().unwrap().contains("InvalidCobs"));
}
