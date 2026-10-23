//! Host decoding for the ESP32's delimiter-mode, one-frame-per-datagram bridge.
//! Wire types come from the Teensy project, not a second schema maintained here.

use monoprop_common::{LegacyDroneTelemetry, TelemetryFrame};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub mod commands;

/// Includes the trailing COBS sentinel; matches esp32Firmware/src/main.cpp.
pub const MAX_FRAME_BYTES: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Schema {
    Modern,
    Legacy,
    Echo,
}

/// Host-defined bench-test payload. The ESP32 echoes its encoded bytes unchanged.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EchoMessage {
    pub sequence: u32,
    pub message: String,
}

pub fn encode_echo(sequence: u32, message: &str) -> Result<Vec<u8>, String> {
    let bytes = postcard::to_stdvec_cobs(&EchoMessage {
        sequence,
        message: message.to_owned(),
    })
    .map_err(|error| error.to_string())?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err("echo message exceeds the 1024-byte bridge limit".into());
    }
    Ok(bytes)
}

impl std::str::FromStr for Schema {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "modern" => Ok(Self::Modern),
            "legacy" => Ok(Self::Legacy),
            "echo" => Ok(Self::Echo),
            _ => Err(format!(
                "unknown schema {value:?}; use modern, legacy or echo"
            )),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "schema", content = "telemetry", rename_all = "lowercase")]
pub enum Telemetry {
    Modern(TelemetryFrame),
    Legacy(LegacyDroneTelemetry),
    Echo(EchoMessage),
}

#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    EmptyFrame,
    OversizedFrame,
    MissingDelimiter,
    MultipleFrames,
    InvalidCobs,
    InvalidPostcard(postcard::Error),
    TrailingPayload,
}

fn decode_payload<T: DeserializeOwned>(payload: &[u8]) -> Result<T, DecodeError> {
    let (message, rest) =
        postcard::take_from_bytes(payload).map_err(DecodeError::InvalidPostcard)?;
    if !rest.is_empty() {
        return Err(DecodeError::TrailingPayload);
    }
    Ok(message)
}

/// Never stitch UDP datagrams together: loss or another sender must not corrupt
/// framing. A schema must be selected explicitly because Postcard is not self-describing.
pub fn decode_datagram(packet: &[u8], schema: Schema) -> Result<Telemetry, DecodeError> {
    if packet.len() < 2 {
        return Err(DecodeError::EmptyFrame);
    }
    if packet.len() > MAX_FRAME_BYTES {
        return Err(DecodeError::OversizedFrame);
    }
    if packet.last() != Some(&0) {
        return Err(DecodeError::MissingDelimiter);
    }
    let encoded = &packet[..packet.len() - 1];
    if encoded.contains(&0) {
        return Err(DecodeError::MultipleFrames);
    }
    let mut scratch = [0; MAX_FRAME_BYTES];
    scratch[..encoded.len()].copy_from_slice(encoded);
    let len = cobs::decode_in_place(&mut scratch[..encoded.len()])
        .map_err(|_| DecodeError::InvalidCobs)?;
    match schema {
        Schema::Modern => decode_payload(&scratch[..len]).map(Telemetry::Modern),
        Schema::Legacy => decode_payload(&scratch[..len]).map(Telemetry::Legacy),
        Schema::Echo => decode_payload(&scratch[..len]).map(Telemetry::Echo),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_round_trip_and_size_limit() {
        let bytes = encode_echo(7, "hello ESP32\0").unwrap();
        let Telemetry::Echo(message) = decode_datagram(&bytes, Schema::Echo).unwrap() else {
            panic!("not echo")
        };
        assert_eq!(message.sequence, 7);
        assert_eq!(message.message, "hello ESP32\0");
        assert!(encode_echo(0, &"x".repeat(1024)).is_err());
    }

    fn modern_packet() -> Vec<u8> {
        let mut telemetry = TelemetryFrame {
            sequence: 42,
            link_ok: true,
            ..Default::default()
        };
        telemetry.sensor_data.timestamp_us = 1_234_567;
        telemetry.sensor_data.lidar_range_m = Some(1.25);
        telemetry.vehicle_state.position_m = [0.0, -2.0, 1.25];
        postcard::to_stdvec_cobs(&telemetry).unwrap()
    }

    #[test]
    fn modern_round_trip_uses_shared_teensy_schema() {
        let decoded = decode_datagram(&modern_packet(), Schema::Modern).unwrap();
        let Telemetry::Modern(frame) = decoded else {
            panic!("wrong schema")
        };
        assert_eq!(frame.sequence, 42);
        assert_eq!(frame.sensor_data.timestamp_us, 1_234_567);
        assert_eq!(frame.sensor_data.lidar_range_m, Some(1.25));
        assert_eq!(frame.vehicle_state.attitude_quat, [1.0, 0.0, 0.0, 0.0]);
        let json = serde_json::to_value(Telemetry::Modern(frame)).unwrap();
        assert_eq!(json["schema"], "modern");
        assert_eq!(
            json["telemetry"]["sensor_data"]["gps_data"],
            serde_json::Value::Null
        );
    }

    #[test]
    fn legacy_preserves_timestamp_and_xyzw_quaternion() {
        let frame = LegacyDroneTelemetry {
            timestamp_us: u64::MAX,
            orientation: [0.0, 0.0, 0.0, 1.0],
            ..Default::default()
        };
        let packet = postcard::to_stdvec_cobs(&frame).unwrap();
        let Telemetry::Legacy(actual) = decode_datagram(&packet, Schema::Legacy).unwrap() else {
            panic!("wrong schema")
        };
        assert_eq!(actual.timestamp_us, frame.timestamp_us);
        assert_eq!(actual.orientation, frame.orientation);
    }

    #[test]
    fn rejects_empty_oversized_unterminated_and_joined_frames() {
        assert!(matches!(
            decode_datagram(&[], Schema::Modern),
            Err(DecodeError::EmptyFrame)
        ));
        assert!(matches!(
            decode_datagram(&[0], Schema::Modern),
            Err(DecodeError::EmptyFrame)
        ));
        assert!(matches!(
            decode_datagram(&[1; 1025], Schema::Modern),
            Err(DecodeError::OversizedFrame)
        ));
        let packet = modern_packet();
        assert!(matches!(
            decode_datagram(&packet[..packet.len() - 1], Schema::Modern),
            Err(DecodeError::MissingDelimiter)
        ));
        let joined = [packet.clone(), packet].concat();
        assert!(matches!(
            decode_datagram(&joined, Schema::Modern),
            Err(DecodeError::MultipleFrames)
        ));
    }

    #[test]
    fn rejects_bad_cobs_wrong_schema_and_trailing_serialized_bytes() {
        assert!(matches!(
            decode_datagram(&[255, 1, 0], Schema::Modern),
            Err(DecodeError::InvalidCobs)
        ));
        assert!(decode_datagram(&modern_packet(), Schema::Legacy).is_err());
        let mut payload = postcard::to_stdvec(&TelemetryFrame::default()).unwrap();
        payload.push(99);
        let mut packet = vec![0; payload.len() + payload.len() / 254 + 2];
        let used = cobs::encode(&payload, &mut packet);
        packet.truncate(used + 1);
        packet[used] = 0;
        assert!(matches!(
            decode_datagram(&packet, Schema::Modern),
            Err(DecodeError::TrailingPayload)
        ));
        assert!(decode_datagram(&modern_packet(), Schema::Modern).is_ok());
    }

    #[test]
    fn udp_loopback_preserves_frame_and_decodes_without_reply() {
        use std::{net::UdpSocket, time::Duration};
        let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
        receiver
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        let packet = modern_packet();
        sender
            .send_to(&packet, receiver.local_addr().unwrap())
            .unwrap();
        let mut buffer = [0; 65535];
        let (len, source) = receiver.recv_from(&mut buffer).unwrap();
        assert_eq!(source, sender.local_addr().unwrap());
        assert_eq!(&buffer[..len], packet);
        assert!(matches!(
            decode_datagram(&buffer[..len], Schema::Modern),
            Ok(Telemetry::Modern(_))
        ));
    }
}
