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
            byte if byte < 0x20 || byte == 0x7f => index += 1,
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }

    Ok(output)
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
