use super::DeltaError;

pub fn sanitize_ansi(input: &[u8]) -> Result<Vec<u8>, DeltaError> {
    std::str::from_utf8(input).map_err(|_| DeltaError::InvalidUtf8)?;
    let mut output = Vec::with_capacity(input.len());
    let mut index = 0;

    while index < input.len() {
        match input[index] {
            0x1b => index = consume_escape(input, index, &mut output),
            b'\t' | b'\n' => {
                output.push(input[index]);
                index += 1;
            }
            b'\r' if input.get(index + 1) == Some(&b'\n') => {
                output.push(b'\r');
                index += 1;
            }
            0xc2 if input
                .get(index + 1)
                .is_some_and(|byte| (0x80..=0x9f).contains(byte)) =>
            {
                index += 2;
            }
            byte if byte < 0x20 || byte == 0x7f => index += 1,
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }

    let text = String::from_utf8(output).map_err(|_| DeltaError::InvalidUtf8)?;
    Ok(escape_confusables(&text).into_bytes())
}

fn is_confusable(value: char) -> bool {
    matches!(
        value,
        '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
            | '\u{200b}'..='\u{200f}'
            | '\u{2060}'..='\u{2064}'
            | '\u{feff}'
    )
}

fn escape_confusables(text: &str) -> String {
    if !text.chars().any(is_confusable) {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    for value in text.chars() {
        if is_confusable(value) {
            out.push_str(&format!("<U+{:04X}>", value as u32));
        } else {
            out.push(value);
        }
    }
    out
}

pub fn has_confusables(text: &str) -> bool {
    text.chars().any(is_confusable)
}

fn consume_escape(input: &[u8], start: usize, output: &mut Vec<u8>) -> usize {
    match input.get(start + 1).copied() {
        Some(b'[') => consume_csi(input, start, output),
        Some(b']' | b'P' | b'_' | b'^') => consume_control_string(input, start + 2),
        Some(_) => (start + 2).min(input.len()),
        None => input.len(),
    }
}

fn consume_csi(input: &[u8], start: usize, output: &mut Vec<u8>) -> usize {
    let mut end = start + 2;
    while end < input.len() {
        let byte = input[end];
        if (0x40..=0x7e).contains(&byte) {
            if byte == b'm'
                && input[start + 2..end].iter().all(|value| {
                    value.is_ascii_digit() || matches!(value, b';' | b':' | b'?' | b' ')
                })
            {
                output.extend_from_slice(&input[start..=end]);
            }
            return end + 1;
        }
        end += 1;
    }
    input.len()
}

fn consume_control_string(input: &[u8], mut index: usize) -> usize {
    while index < input.len() {
        if input[index] == 0x07 {
            return index + 1;
        }
        if input[index] == 0x1b && input.get(index + 1) == Some(&b'\\') {
            return index + 2;
        }
        index += 1;
    }
    input.len()
}
