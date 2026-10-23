//! JSON is console input only; serialize the shared command itself on the wire.
use monoprop_common::{ControlCommand, MAX_TVC_GIMBAL_RAD, TextCommand};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    content = "command",
    rename_all = "lowercase",
    deny_unknown_fields
)]
enum ConsoleCommand {
    Text(TextCommand),
    Control(ControlCommand),
}

pub fn encode_command(json: &str) -> Result<Vec<u8>, String> {
    let command: ConsoleCommand = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let bytes = match command {
        ConsoleCommand::Text(command) => postcard::to_stdvec_cobs(&command),
        ConsoleCommand::Control(command) => {
            if !command.is_valid()
                || !(0.0..=1.0).contains(&command.actuator_command.thrust_norm)
                || command.actuator_command.gimbal_rad.iter().any(|v| v.abs() > MAX_TVC_GIMBAL_RAD)
            {
                return Err("invalid control: require finite values, throttle 0..1 and gimbals within the shared TVC limits".into());
            }
            postcard::to_stdvec_cobs(&command)
        }
    }.map_err(|e| e.to_string())?;
    if bytes.len() > crate::MAX_FRAME_BYTES {
        return Err("command exceeds the ESP32's 1024-byte limit".into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_the_shared_enum_not_the_json_envelope() {
        let mut bytes = encode_command(r#"{"kind":"text","command":"Disarm"}"#).unwrap();
        assert_eq!(bytes.last(), Some(&0));
        assert_eq!(
            postcard::from_bytes_cobs::<TextCommand>(&mut bytes).unwrap(),
            TextCommand::Disarm
        );
    }

    #[test]
    fn control_round_trip_and_invalid_throttle() {
        let command = ControlCommand {
            sequence: 5,
            ..Default::default()
        };
        let mut input = serde_json::json!({"kind": "control", "command": command});
        let mut bytes = encode_command(&input.to_string()).unwrap();
        let actual: ControlCommand = postcard::from_bytes_cobs(&mut bytes).unwrap();
        assert_eq!(actual.sequence, 5);
        assert_eq!(actual.actuator_command.thrust_norm, 0.0);
        input["command"]["actuator_command"]["thrust_norm"] = serde_json::json!(1.1);
        assert!(encode_command(&input.to_string()).is_err());
        assert!(encode_command("hello").is_err());
        assert!(encode_command(r#"{"kind":"text","command":"unknown"}"#).is_err());
    }
}
