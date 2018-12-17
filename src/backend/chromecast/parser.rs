///! Parser for Chromecast TXT records.
///!
///! Each Chromecast TXT record is a `key=value` pair that specifies some
///! metadata about the device. There are [several key-value pairs in the record](https://github.com/azasypkin/rust-cast#dns-txt-record-description).
///! The most relevant ones are:
///!
///! - `md` - Model Name
///! - `fn` - Friendly Name
extern crate nom;

use nom::alphanumeric;
use nom::types::CompleteStr;

use std::collections::HashMap;
use std::str;

named!(key_value<CompleteStr, (CompleteStr, CompleteStr)>,
do_parse!(
    key: alphanumeric >>
    char!('=') >>
    val: take_while!(call!(|_| true)) >>
    (key, val)
)
);

/// Extract key-value pairs out of a TXT record and collect them into
/// a `HashMap`.
pub fn dns_txt<T: AsRef<str>>(vec: &[T]) -> HashMap<String, String> {
    let mut collect: HashMap<String, String> = HashMap::new();
    for txt in vec.iter() {
        if let Ok((_, (key, value))) = key_value(CompleteStr(txt.as_ref())) {
            collect.insert(key.as_ref().to_owned(), value.as_ref().to_owned());
        }
    }
    collect
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_dns_txt() {
        let parsed = dns_txt(&vec!["fn=Device Name=Bob's", "md=Chromecast"]);
        let name = parsed.get("fn").unwrap();
        let model = parsed.get("md").unwrap();
        assert_eq!("Device Name=Bob's", name);
        assert_eq!("Chromecast", model);
        assert_eq!(None, parsed.get("none"));
    }
}
