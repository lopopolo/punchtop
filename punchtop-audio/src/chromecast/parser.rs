//! Parser for Chromecast TXT records.
//!
//! Each Chromecast TXT record is a `key=value` pair that specifies some
//! metadata about the device. There are [several key-value pairs in the record](https://github.com/azasypkin/rust-cast#dns-txt-record-description).
//! The most relevant ones are:
//!
//! - `md` - Model Name
//! - `fn` - Friendly Name
use nom::types::CompleteStr;
use nom::{alphanumeric, char, do_parse, named, take_while};

use std::collections::HashMap;
use std::str;

named!(
    key_value<CompleteStr, (CompleteStr, CompleteStr)>,
    do_parse!(
        key: alphanumeric >>
        char!('=') >>
        val: take_while!(|_| true) >>
        (key, val)
    )
);

/// Extract key-value pairs out of a TXT record and collect them into
/// a `HashMap`.
pub fn dns_txt<T: AsRef<str>>(vec: &[T]) -> HashMap<String, String> {
    let mut collect = HashMap::new();
    for txt in vec.iter() {
        if let Ok((_, (key, value))) = key_value(CompleteStr(txt.as_ref())) {
            collect.insert(key.as_ref().to_owned(), value.as_ref().to_owned());
        }
    }
    collect
}

#[cfg(test)]
mod tests {
    #[test]
    fn parse_dns_txt() {
        let parsed = super::dns_txt(&["fn=Device Name=Bob's", "md=Chromecast"]);
        let name = &parsed["fn"];
        let model = &parsed["md"];
        assert_eq!("Device Name=Bob's", name);
        assert_eq!("Chromecast", model);
        assert_eq!(None, parsed.get("none"));
    }
}
