use serde_json::Value;

pub(super) fn bytes(frames: &[Value], stream: &str) -> Vec<u8> {
    frames
        .iter()
        .filter(|frame| frame["kind"] == "output" && frame["stream"] == stream)
        .flat_map(|frame| {
            frame["bytes"]
                .as_array()
                .expect("output bytes")
                .iter()
                .map(|byte| byte.as_u64().expect("byte") as u8)
        })
        .collect()
}

pub(super) fn code(frames: &[Value]) -> Option<i64> {
    frames
        .last()
        .and_then(|frame| frame["process"]["code"].as_i64())
}
