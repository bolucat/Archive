//! Bounded log previews of byte and text payloads.
//!
//! A payload such as an image, a profile or a script is logged as its length,
//! its CRC-32 and its first and last bytes, so a log line stays small while
//! two payloads can still be told apart:
//!
//! ```
//! use nyanpasu_log_preview::{bytes, text};
//!
//! let png = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, b'I', b'H', b'D', b'R', 0];
//! assert_eq!(
//!     bytes(&png).edge(4).to_string(),
//!     "17 bytes, crc32 67d38fb2, 89504e47..48445200",
//! );
//! assert_eq!(
//!     text("function main(config) { return config; }").to_string(),
//!     r#"40 bytes, crc32 fa887ff7, "function".."onfig; }""#,
//! );
//! ```
//!
//! [`Preview`] formats the same through `Display` and `Debug`, so it fits both
//! `%` and `?` fields of `tracing` macros.

use std::fmt;

/// The number of bytes shown from each end when the caller does not set one.
pub const DEFAULT_EDGE: usize = 8;

/// Previews binary data: the shown bytes are written in hex.
pub fn bytes(data: &[u8]) -> Preview<'_> {
    Preview {
        data,
        edge: DEFAULT_EDGE,
        kind: Kind::Bytes,
    }
}

/// Previews text: the shown ends are written as escaped strings and never
/// split a character.
pub fn text(data: &str) -> Preview<'_> {
    Preview {
        data: data.as_bytes(),
        edge: DEFAULT_EDGE,
        kind: Kind::Text,
    }
}

/// A payload formatted as its length, CRC-32 and both ends.
///
/// A payload no longer than both ends together is shown whole.
#[derive(Clone, Copy)]
pub struct Preview<'a> {
    data: &'a [u8],
    edge: usize,
    kind: Kind,
}

#[derive(Clone, Copy)]
enum Kind {
    Bytes,
    /// `data` is valid UTF-8.
    Text,
}

impl Preview<'_> {
    /// Sets how many bytes are shown from each end. Text ends shrink to the
    /// nearest character boundary.
    pub fn edge(mut self, bytes: usize) -> Self {
        self.edge = bytes;
        self
    }
}

impl fmt::Display for Preview<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let len = self.data.len();
        write!(
            f,
            "{len} bytes, crc32 {:08x}, ",
            crc32fast::hash(self.data)
        )?;
        let whole = len <= self.edge.saturating_mul(2);
        match self.kind {
            Kind::Bytes if whole => write_hex(f, self.data),
            Kind::Bytes => {
                write_hex(f, &self.data[..self.edge])?;
                f.write_str("..")?;
                write_hex(f, &self.data[len - self.edge..])
            }
            Kind::Text => {
                // Only `text` builds a text preview, from a `str`.
                let text = std::str::from_utf8(self.data).expect("text previews hold UTF-8");
                if whole {
                    return write!(f, "{text:?}");
                }
                let mut head = self.edge;
                while !text.is_char_boundary(head) {
                    head -= 1;
                }
                let mut tail = len - self.edge;
                while !text.is_char_boundary(tail) {
                    tail += 1;
                }
                write!(f, "{:?}..{:?}", &text[..head], &text[tail..])
            }
        }
    }
}

impl fmt::Debug for Preview<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

fn write_hex(f: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    bytes.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_payloads_are_shown_whole() {
        assert_eq!(bytes(&[]).to_string(), "0 bytes, crc32 00000000, ");
        assert_eq!(
            bytes(b"abc").to_string(),
            format!("3 bytes, crc32 {:08x}, 616263", crc32fast::hash(b"abc"))
        );
        assert_eq!(
            text("a\"b\n").to_string(),
            format!(r#"4 bytes, crc32 {:08x}, "a\"b\n""#, crc32fast::hash(b"a\"b\n"))
        );
    }

    #[test]
    fn long_payloads_keep_only_their_ends() {
        let data: Vec<u8> = (0..=255).collect();
        let preview = bytes(&data).edge(2).to_string();
        assert!(preview.starts_with("256 bytes, crc32 "));
        assert!(preview.ends_with(", 0001..feff"));
    }

    #[test]
    fn edge_of_zero_shows_no_bytes() {
        assert!(bytes(b"abc").edge(0).to_string().ends_with(", .."));
        assert!(text("abc").edge(0).to_string().ends_with(r#", "".."""#));
        assert!(text("").edge(0).to_string().ends_with(r#", """#));
    }

    #[test]
    fn payload_of_both_edges_is_shown_whole() {
        assert!(bytes(b"abcd").edge(2).to_string().ends_with(", 61626364"));
        assert!(bytes(b"abcde").edge(2).to_string().ends_with(", 6162..6465"));
        assert!(bytes(b"abc").edge(usize::MAX).to_string().ends_with(", 616263"));
    }

    #[test]
    fn text_ends_do_not_split_characters() {
        // Each "猫" is three bytes; a two-byte edge cannot hold one.
        let preview = text("猫a猫猫b猫").edge(4).to_string();
        assert!(preview.ends_with(r#", "猫a".."b猫""#), "{preview}");
        assert!(text("猫猫猫").edge(1).to_string().ends_with(r#", "".."""#));
    }

    #[test]
    fn checksum_tells_payloads_apart() {
        let a = bytes(b"0123456789abcdefXY0123456789abcdef").to_string();
        let b = bytes(b"0123456789abcdefZW0123456789abcdef").to_string();
        assert_ne!(a, b);
    }

    #[test]
    fn debug_matches_display() {
        let preview = text("payload");
        assert_eq!(format!("{preview:?}"), preview.to_string());
    }
}
