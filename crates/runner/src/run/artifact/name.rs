const RESERVED: [&str; 4] = ["con", "prn", "aux", "nul"];
const PREFIXES: [&str; 2] = ["com", "lpt"];

pub(super) fn id(subject: &str) -> bool {
    bound(subject, 128) && lower(subject) && atom(subject)
}

pub(super) fn file(name: &str) -> bool {
    bound(name, 255) && edges(name) && portable(name)
}

fn bound(text: &str, limit: usize) -> bool {
    !text.is_empty() && text.len() <= limit
}

fn lower(word: &str) -> bool {
    matches!(word.as_bytes().first(), Some(b'a'..=b'z'))
}

fn atom(token: &str) -> bool {
    let bytes = token.as_bytes();
    bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        && !bytes.windows(2).any(|pair| pair == b"--")
}

fn edges(base: &str) -> bool {
    let bytes = base.as_bytes();
    bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
}

fn portable(filename: &str) -> bool {
    let bytes = filename.as_bytes();
    bytes.iter().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
    }) && !bytes.windows(2).any(|pair| pair == b"..")
        && !reserved(filename)
}

fn reserved(path: &str) -> bool {
    let stem = path.split('.').next().unwrap_or_default();
    if RESERVED.contains(&stem) {
        return true;
    }
    let bytes = stem.as_bytes();
    bytes.len() == 4 && PREFIXES.contains(&&stem[..3]) && matches!(bytes[3], b'1'..=b'9')
}
