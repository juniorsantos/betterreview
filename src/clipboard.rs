use std::io::{self, Write};

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub(crate) fn copy(content: &str) -> io::Result<()> {
    let stdout = io::stdout();
    write_osc52(&mut stdout.lock(), content)
}

fn write_osc52(writer: &mut impl Write, content: &str) -> io::Result<()> {
    writer.write_all(osc52_sequence(content).as_bytes())?;
    writer.flush()
}

fn osc52_sequence(content: &str) -> String {
    format!("\u{1b}]52;c;{}\u{7}", encode_base64(content.as_bytes()))
}

fn encode_base64(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);

        encoded.push(BASE64[(first >> 2) as usize] as char);
        encoded.push(BASE64[(((first & 0x03) << 4) | (second >> 4)) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            BASE64[(((second & 0x0f) << 2) | (third >> 6)) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            BASE64[(third & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osc52_sequence_base64_encodes_unicode_code() {
        assert_eq!(osc52_sequence("ação"), "\u{1b}]52;c;YcOnw6Nv\u{7}");
    }

    #[test]
    fn writer_receives_the_complete_osc52_sequence() {
        let mut output = Vec::new();

        write_osc52(&mut output, "hello").unwrap();

        assert_eq!(output, b"\x1b]52;c;aGVsbG8=\x07");
    }
}
