use serde::Serialize;

const RECORD_SEPARATOR: &str = "\u{001E}";

pub fn to_json<T>(value: &T) -> Result<String, serde_json::Error>
where
    T: ?Sized + Serialize,
{
    let serialized = serde_json::to_string(value)?;
    Ok(serialized + RECORD_SEPARATOR)
}

pub fn strip_record_separator(input: &str) -> &str {
    input.trim_end_matches(RECORD_SEPARATOR)
}
