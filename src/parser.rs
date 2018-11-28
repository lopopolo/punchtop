extern crate nom;

use nom::alphanumeric;
use nom::types::CompleteByteSlice;

use std::str;
use std::collections::HashMap;

fn complete_byte_slice_to_str(s: CompleteByteSlice) -> Result<&str, str::Utf8Error> {
  str::from_utf8(s.0)
}

named!(key_value<CompleteByteSlice, (&str, &str)>,
  do_parse!(
      key: map_res!(alphanumeric, complete_byte_slice_to_str)
  >>       char!('=')
  >>  val: map_res!(
           take_while!(call!(|_| true)),
           complete_byte_slice_to_str
         )
  >>     (key, val)
  )
);

// TXT records are given as a Vec of key=value pairs
pub fn dns_txt(vec: &[&str]) -> HashMap<String, String> {
    let mut collect: HashMap<String, String> = HashMap::new();
    for txt in vec.iter() {
        match key_value(CompleteByteSlice(txt.as_bytes())) {
            Ok((_, (key, value))) => collect.insert(key.to_owned(), value.to_owned()),
            _ => None,
        };
    }
    collect
}
