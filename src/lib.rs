//! This crate provides a library for Halo: Combat Evolved cache file parsing and manipulation.
pub mod tag;
pub mod map;
extern crate encoding;
use self::encoding::{Encoding, DecoderTrap, EncoderTrap};
use self::encoding::all::ISO_8859_1;

// This function will create an ISO 8859-1 vec from a string
fn encode_latin1_string(string : &str) -> Result<Vec<u8>,&'static str> {
    match ISO_8859_1.encode(&string, EncoderTrap::Strict) {
        Ok(n) => Ok(n),
        Err(_) => Err("failed to encode string")
    }
}

// This function will create a string from an ISO 8859-1 string in a slice.
fn string_from_slice(slice : &[u8]) -> Result<String,&'static str> {
    match slice.iter().position(|&x| x == 0) {
        Some(n) => match ISO_8859_1.decode(&slice[..n], DecoderTrap::Strict) {
            Ok(n) => Ok(n),
            Err(_) => Err("invalid latin1 string")
        },
        None => Err("string had no null-termination")
    }
}

// Add padding for 32-bit word alignment.
fn pad_32(length : usize) -> usize {
    length + (4 - (length % 4)) % 4
}
