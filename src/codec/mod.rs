//! Private codecs: byte representations to text or unscaled integer values.

mod cp037;
mod numeric;

pub(crate) use cp037::{decode_cp037, utf8_length};
pub(crate) use numeric::{decode_binary, decode_display, decode_packed};
