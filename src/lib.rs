//! This crate provides a library for Halo: Combat Evolved cache file parsing and manipulation.
pub mod tag;
pub mod map;

#[cfg(test)]
mod tests {
    use map::*;
    use test::Bencher;
    use std::io::{Write,Read};
    use std::fs::File;

    #[test]
    pub fn test() {
        let mut file = File::open("bloodgulch.map").unwrap();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();

        let passes = 5;

        let mut map = match Map::from_cache_file(&data) {
            Ok(n) => n,
            Err(e) => panic!("{}",e)
        };

        for pass in 0..passes {
            let older_data_len = data.len();

            println!("PASS {}: Parsing...", pass);
            let mut map = match Map::from_cache_file(&data) {
                Ok(n) => n,
                Err(e) => panic!("{}",e)
            };

            map.tag_array.remove_dead_tags();

            println!("PASS {}: Rebuilding...", pass);
            data = match map.as_cache_file() {
                Ok(n) => n,
                Err(e) => panic!("{}",e)
            };

            if pass + 1 == passes {
                let mut new_file = File::create(&format!("pass_{}.map", pass)).unwrap();
                new_file.write_all(&data);
            }
        }
    }
}
