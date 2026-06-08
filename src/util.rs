//! Small shared helpers.

use std::fmt::Write as _;

/// Encode a byte slice as a lowercase hex string.
///
/// Replaces the `{:x}` formatting that digest outputs supported before the
/// `sha2` 0.11 upgrade, whose `Output` type no longer implements `LowerHex`.
#[must_use]
pub fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        // Writing a formatted byte into a String is infallible.
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    #[rstest]
    #[case(&[], "")]
    #[case(&[0x00], "00")]
    #[case(&[0x0f, 0xa0, 0xff], "0fa0ff")]
    #[case(&[0xde, 0xad, 0xbe, 0xef], "deadbeef")]
    fn hex_encode_lowercase_zero_padded(#[case] bytes: &[u8], #[case] expected: &str) {
        assert_that!(hex_encode(bytes), eq(expected));
    }
}
