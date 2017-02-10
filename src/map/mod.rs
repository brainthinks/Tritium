//! Module for handling maps and cache files
extern crate encoding;
use self::encoding::{Encoding, DecoderTrap, EncoderTrap};
use self::encoding::all::ISO_8859_1;

extern crate byteorder;
use self::byteorder::{ByteOrder,LittleEndian};
use super::tag::*;


#[derive(PartialEq,Clone)]
/// The game can vary from map to map, using a different version number for each game. Maps from
/// one game will not work on maps from another game.
pub enum Game {
    /// The retail boxed version of the game is installed from a disc. This version is available
    /// for both Mac and Windows.
    HaloCombatEvolved,

    /// Halo Custom Edition is obtained through a download and can be installed alongside the
    /// retail version of the game. It supports indexed tags, reducing the size of the map and
    /// removes the need to localize the map for a particular language. Multiplayer requires that
    /// the CRC32 checksum of the client's map to match the server's in order to join. This version
    /// is exclusively available for Windows, however.
    HaloCustomEdition,

    /// Other iterations of the engine use different version numbers.
    Unknown(u32)
}
impl Game {
    /// Convert a 32-bit unsigned integer into a Game.
    pub fn from_u32(map_game : u32) -> Game {
        match map_game {
            0x7 => Game::HaloCombatEvolved,
            0x261 => Game::HaloCustomEdition,
            n => Game::Unknown(n)
        }
    }

    /// Convert a Game to its equivalent 32-bit integer.
    pub fn as_u32(&self) -> u32 {
        match *self {
            Game::HaloCombatEvolved => 0x7,
            Game::HaloCustomEdition => 0x261,
            Game::Unknown(n) => n
        }
    }
}

#[derive(PartialEq,Clone)]
/// The type of map determines whether or not it's singleplayer, multiplayer, or a user interface.
pub enum MapType {
    /// This type of map is used for network play.
    Multiplayer,

    /// This type of map is used in singleplayer or co-operative play (Xbox only).
    Singleplayer,

    /// This type of map is loaded by default. On the client, it also serves as the main menu and
    /// allows the user to choose what to do.
    UserInterface,

    /// Other map types may use different numbers.
    Unknown(u32)
}
impl MapType {
    /// Convert a 32-bit unsigned integer into a MapType.
    pub fn from_u32(map_type : u32) -> MapType {
        match map_type {
            0x0 => MapType::Singleplayer,
            0x1 => MapType::Multiplayer,
            0x2 => MapType::UserInterface,
            n => MapType::Unknown(n)
        }
    }

    /// Convert a MapType to its equivalent 32-bit integer.
    pub fn as_u32(&self) -> u32 {
        match *self {
            MapType::Singleplayer => 0x0,
            MapType::Multiplayer => 0x1,
            MapType::UserInterface => 0x2,
            MapType::Unknown(n) => n
        }
    }
}

#[derive(Clone)]
/// Map structs can be created from other cache files to be parsed and can be created into map
/// files.
pub struct Map {
    /// This determines the game and the type of map.
    pub kind : (Game,MapType),

    /// This is the name of the map file. Tool.exe uses the name of the scenario tag when building
    /// a map. It must not exceed 31 characters.
    pub name : String,

    /// This is the build of the map file. This is read by map editors. It must not exceed 31
    /// characters.
    pub build : String,

    /// Maps contain an array of tags which make up the map's resources for gameplay.
    pub tag_array : TagArray
}
impl Map {
    /// This function attempts to parse a cache file.
    ///
    /// If the cache file is invalid or an error occurs, `Err` is returned, instead.
    pub fn from_cache_file(cache_file : &[u8]) -> Result<Map,&'static str> {
        // A cache file header is 2048 bytes, so a cache file must be at least 2048 bytes.
        if cache_file.len() < 0x800 {
            return Err("invalid cache file");
        }

        // Check the "head" and "foot" markers in the beginning and ending of header, respectively.
        if LittleEndian::read_u32(&cache_file[0x0..]) != 0x68656164 || LittleEndian::read_u32(&cache_file[0x7FC..]) != 0x666F6F74 {
            return Err("head/foot in cache file header corrupt")
        }

        // It is valid if the buffer size is bigger than the file size in the map header. It isn't
        // if it's less, however.
        let file_size = LittleEndian::read_u32(&cache_file[0x8..]) as usize;
        if file_size > cache_file.len() || file_size > 0x7FFFFFFF {
            return Err("file size in header is invalid")
        }

        // Get the file name of the cache file.
        let name = match string_from_slice(&cache_file[0x20..]) {
            Ok(n) => n,
            Err(_) => return Err("name could not be parsed")
        };

        // Get the build number of the cache file.
        let build = match string_from_slice(&cache_file[0x40..]) {
            Ok(n) => n,
            Err(_) => return Err("name could not be parsed")
        };

        // Get the meta data of the cache file.
        let meta_offset = LittleEndian::read_u32(&cache_file[0x10..]) as usize;
        let meta_length = LittleEndian::read_u32(&cache_file[0x14..]) as usize;
        let meta_end = match meta_offset.checked_add(meta_length) {
            Some(n) => n as usize,
            None => return Err("invalid meta data range")
        };
        if meta_end > file_size {
            return Err("invalid meta data range")
        }
        let base_address : u32 = 0x40440000;
        let meta_data = &cache_file[meta_offset .. meta_offset + meta_length];

        // Convert a meta data address into an offset.
        let address_to_offset = |address : u32| -> Option<usize> {
            match address.checked_sub(base_address) {
                Some(n) => {
                    let offset = n as usize;
                    if offset < meta_data.len() {
                        Some(offset)
                    }
                    else {
                        None
                    }
                },
                None => None
            }
        };

        // Get model data stuff.
        let model_data_offset = LittleEndian::read_u32(&meta_data[0x14..]) as usize;
        let model_data_size = LittleEndian::read_u32(&meta_data[0x20..]) as usize;
        let model_data_end = model_data_size + model_data_offset;

        if model_data_end > cache_file.len()  {
            return Err("invalid model data offset/size")
        }

        let index_data_offset = LittleEndian::read_u32(&meta_data[0x1C..]) as usize + model_data_offset;
        if index_data_offset > cache_file.len() || index_data_offset > model_data_end {
            return Err("invalid index data offset")
        }

        let vertices = &cache_file[model_data_offset..index_data_offset];
        let indices = &cache_file[index_data_offset..model_data_end];

        // Begin adding tags.
        let mut tags = Vec::new();
        let tag_count = LittleEndian::read_u32(&meta_data[0xC..]) as usize;
        tags.reserve_exact(tag_count);

        // Go through the tag array.
        let tag_array_address = LittleEndian::read_u32(&meta_data[0x0..]);
        let tag_array_start = match address_to_offset(tag_array_address) {
            Some(n) => n,
            None => return Err("could not find tag array")
        };
        let tag_array_end = tag_array_start + 0x20 * tag_count;
        if tag_array_end > meta_data.len() {
            return Err("tag array ends outside of the meta data")
        }

        // Let's see if we have a scenario tag.
        let scenario_tag = {
            let tag_id = LittleEndian::read_u32(&meta_data[0x4..]);
            if tag_id == 0xFFFFFFFF {
                None
            }
            else {
                let index = LittleEndian::read_u32(&meta_data[0x4..]) as usize & 0xFFFF;
                if index > tag_count {
                    return Err("scenario tag outside of tag array!")
                }
                else {
                    Some(index)
                }
            }
        };

        let tag_array = &meta_data[tag_array_start .. tag_array_end];

        // Go through the SBSPs in the scenario tag. We only need to do this once. The value is:
        // Vec<(Tag ID, Memory Address, File Offset, Data Size)>
        let sbsps : Vec<(usize,u32,usize,usize)> = if scenario_tag.is_some() {
            let mut sbsps = Vec::new();

            let scenario_tag_index = scenario_tag.as_ref().unwrap();
            let principal_scenario_tag = &tag_array[scenario_tag_index * 0x20 .. (scenario_tag_index + 1) * 0x20];
            let principal_scenario_tag_data = match address_to_offset(LittleEndian::read_u32(&principal_scenario_tag[0x14..])) {
                Some(n) => if n + 0x5B0 > meta_data.len() {
                        return Err("scenario tag invalid")
                    }
                    else {
                        &meta_data[n..n+0x5B0]
                    },
                None => return Err("scenario tag invalid")
            };

            let sbsp_reflexive = match Reflexive::serialize(&principal_scenario_tag_data[0x5A4..],base_address,base_address + meta_data.len() as u32,32) {
                Ok(n) => n,
                Err(_) => return Err("scenario tag sbsp pointer is invalid")
            };
            let sbsp_count = sbsp_reflexive.count;
            if sbsp_count > 0 {
                let sbsp_offset = address_to_offset(sbsp_reflexive.address).unwrap();
                let sbsp_size = sbsp_count * 32;
                let sbsp_data = &meta_data[sbsp_offset .. sbsp_offset + sbsp_size];
                for i in 0..sbsp_count {
                    let sbsp = &sbsp_data[i*32 .. (i+1)*32];
                    let tag_index = LittleEndian::read_u32(&sbsp[0x1C..]) as usize & 0xFFFF;
                    let tag_memory_address = LittleEndian::read_u32(&sbsp[0x8..]);
                    let tag_file_offset = LittleEndian::read_u32(&sbsp[0x0..]) as usize;
                    let tag_size = LittleEndian::read_u32(&sbsp[0x4..]) as usize;
                    if tag_file_offset + tag_size > file_size {
                        return Err("invalid sbsp tag")
                    }
                    sbsps.push((
                        tag_index,
                        tag_memory_address,
                        tag_file_offset,
                        tag_size
                    ));
                }
            }
            sbsps
        }
        // If there is no scenario tag, then expect no SBSPs.
        else {
            Vec::new()
        };

        // Go through all of the tags.
        for i in 0..tag_count {
            let tag = &tag_array[i * 0x20 .. (i+1) * 0x20];
            let tag_name = match address_to_offset(LittleEndian::read_u32(&tag[0x10..])) {
                Some(n) => {
                    match string_from_slice(&meta_data[n..]) {
                        Ok(n) => n,
                        Err(_) => return Err("name of one of the tags is invalid")
                    }
                }
                None => return Err("name of one of the tags is invalid")
            };

            let classes = (LittleEndian::read_u32(&tag[0x0..]),LittleEndian::read_u32(&tag[0x4..]),LittleEndian::read_u32(&tag[0x8..]));
            let memory_address;
            let data;
            let asset_data;
            let resource_index;
            let implicit = LittleEndian::read_u32(&tag[0x18 ..]) & 1 == 1;

            // This is the memory address read, but it may not necessarily be a memory address. For
            // tags that exist outside of the map file, it may be the case that this is an index
            // for a resource located in a resource map file, such as bitmaps.map, sounds.map, and
            // loc.map.
            let memory_address_read = LittleEndian::read_u32(&tag[0x14..]);

            // Tags that aren't located in the map are located in the resource map files. That
            // means we don't need to do very much.
            if implicit && classes.0 != 560230003 {
                data = None;
                resource_index = Some(memory_address_read);
                asset_data = None;
                memory_address = None;
            }
            // If it's one of the SBSP tags...
            else if classes.0 == 0x73627370 {
                asset_data = None;
                resource_index = None;
                let mut sbsp = None;
                for s in &sbsps {
                    if s.0 == i {
                        sbsp = Some(s)
                    }
                }

                // An SBSP tag that isn't referenced in the scenario tag is invalid.
                if sbsp.is_none() {
                    return Err("orphaned sbsp tag")
                }

                let sbsp_metadata = sbsp.unwrap();
                data = Some(cache_file[sbsp_metadata.2 .. sbsp_metadata.2 + sbsp_metadata.3].to_owned());
                memory_address = Some(sbsp_metadata.1);
            }
            // Everything else...
            else {
                resource_index = None;
                memory_address = Some(LittleEndian::read_u32(&tag[0x14..]));
                let offset = match address_to_offset(*memory_address.as_ref().unwrap()) {
                    Some(n) => n,
                    None => return Err("tag location out of bounds")
                };

                let mut potential_size;
                if offset < tag_array_start {
                    potential_size = tag_array_start - offset
                }
                else {
                    potential_size = meta_data.len() - offset
                };

                for i in 0..tag_count {
                    let tag = &tag_array[i * 0x20 .. (i+1) * 0x20];

                    // Don't check if it can't be checked.
                    if LittleEndian::read_u32(&tag[0x18..]) & 1 == 1 || LittleEndian::read_u32(&tag[0x0..]) == 0x73627370 {
                        continue;
                    }

                    let potential_offset = match address_to_offset(LittleEndian::read_u32(&tag[0x14..])) {
                        Some(n) => n,
                        None => return Err("tag location invalid")
                    };

                    if potential_offset <= offset {
                        continue;
                    }

                    let potential_new_size = potential_offset - offset;
                    if potential_new_size < potential_size {
                        potential_size = potential_new_size;
                    }
                }

                let mut tag_data = meta_data[offset .. offset + potential_size].to_owned();

                // Deal with asset data here per class...
                asset_data = match classes.0 {
                    // bitm (bitmaps)
                    0x6269746D => {
                        let mut asset_data = None;

                        if tag_data.len() < 0x60 + 0xC {
                            return Err("bitmap tag is too small");
                        }

                        let memory_address = *memory_address.as_ref().unwrap();
                        let bitmaps_reflexive = match Reflexive::serialize(&tag_data[0x60..],memory_address,memory_address + tag_data.len() as u32, 0x30) {
                            Ok(n) => n,
                            Err(_) => return Err("invalid address on bitmap reflexive")
                        };

                        if bitmaps_reflexive.count > 0 {
                            let offset = (bitmaps_reflexive.address - memory_address) as usize;
                            let mut bitmaps = &mut tag_data[offset .. offset + bitmaps_reflexive.count * 0x30];

                            let mut asset_data_len = 0;

                            // Get asset data size.
                            for i in 0..bitmaps_reflexive.count {
                                let bitmap = &mut bitmaps[i * 0x30 .. (i+1)*0x30];
                                // Check if internalized...
                                if bitmap[0xF] & 1 == 0 {
                                    asset_data_len += LittleEndian::read_u32(&bitmap[0x18..]);
                                }
                            }

                            if asset_data_len != 0 {
                                let mut asset_data_vec = Vec::new();
                                asset_data_vec.reserve_exact(asset_data_len as usize);

                                for i in 0..bitmaps_reflexive.count {
                                    let mut bitmap = &mut bitmaps[i * 0x30 .. (i+1)*0x30];
                                    if bitmap[0xF] & 1 == 0 {
                                        let data_offset = LittleEndian::read_u32(&bitmap[0x18..]) as usize;
                                        let data_size = LittleEndian::read_u32(&bitmap[0x1C..]) as usize;
                                        let data = &cache_file[data_offset .. data_offset + data_size];
                                        LittleEndian::write_u32(&mut bitmap[0x18..], asset_data_vec.len() as u32);
                                        asset_data_vec.extend_from_slice(data);
                                    }
                                }
                                asset_data = Some(asset_data_vec);
                            }
                        }

                        asset_data
                    },
                    // snd! (sound)
                    0x736E6421 => {
                        if implicit {
                            None
                        }
                        else {
                            let mut asset_data = None;

                            if potential_size < 0x98 + 0xC {
                                return Err("sound tag is too small");
                            }

                            let memory_address = *memory_address.as_ref().unwrap();
                            let ranges_reflexive = match Reflexive::serialize(&tag_data[0x98..],memory_address,memory_address + potential_size as u32, 0x48) {
                                Ok(n) => n,
                                Err(_) => return Err("invalid address on sound range reflexive")
                            };

                            if ranges_reflexive.count > 0 {
                                let offset = (ranges_reflexive.address - memory_address) as usize;
                                let ranges = tag_data[offset .. offset + ranges_reflexive.count * 0x48].to_owned();
                                let mut asset_data_len = 0;

                                for i in 0..ranges_reflexive.count as usize {
                                    let range = &ranges[i * 0x48 .. (i+1)* 0x48];
                                    let permutations_reflexive = match Reflexive::serialize(&range[0x3C..],memory_address,memory_address + potential_size as u32, 0x7C) {
                                        Ok(n) => n,
                                        Err(_) => return Err("invalid address on sound permutation reflexive")
                                    };

                                    if permutations_reflexive.count == 0 {
                                        continue;
                                    }

                                    let offset = (permutations_reflexive.address - memory_address) as usize;
                                    let permutations = &tag_data[offset .. offset + permutations_reflexive.count * 0x7C];

                                    for p in 0..permutations_reflexive.count {
                                        let sound = &permutations[p * 0x7C .. (p+1) * 0x7C];

                                        // Check if internalized...
                                        if sound[0x44] & 1 == 0 {
                                            let data_offset = LittleEndian::read_u32(&sound[0x48..]) as usize;
                                            let data_size = LittleEndian::read_u32(&sound[0x40..]) as usize;
                                            if data_offset + data_size > cache_file.len() {
                                                return Err("sound points to invalid data")
                                            }
                                            asset_data_len += data_size;
                                        }
                                    }
                                }

                                if asset_data_len != 0 {
                                    let mut asset_data_vec = Vec::new();
                                    asset_data_vec.reserve_exact(asset_data_len as usize);

                                    for i in 0..ranges_reflexive.count as usize {
                                        let range = &ranges[i * 0x48 .. (i+1)* 0x48];
                                        let permutations_reflexive = Reflexive::serialize(&range[0x3C..],memory_address,memory_address + potential_size as u32, 0x7C).unwrap();

                                        if permutations_reflexive.count == 0 {
                                            continue;
                                        }

                                        let offset = (permutations_reflexive.address - memory_address) as usize;
                                        let mut permutations = &mut tag_data[offset .. offset + permutations_reflexive.count * 0x7C];

                                        for p in 0..permutations_reflexive.count {
                                            let mut sound = &mut permutations[p * 0x7C .. (p+1) * 0x7C];

                                            // Check if internalized...
                                            if sound[0x44] & 1 == 0 {
                                                let data_offset = LittleEndian::read_u32(&sound[0x48..]) as usize;
                                                let data_size = LittleEndian::read_u32(&sound[0x40..]) as usize;

                                                let data = &cache_file[data_offset .. data_offset + data_size];

                                                LittleEndian::write_u32(&mut sound[0x48..], asset_data_vec.len() as u32);
                                                asset_data_vec.extend_from_slice(data);
                                            }
                                        }
                                    }

                                    asset_data = Some(asset_data_vec);
                                }
                            }
                            asset_data
                        }
                    },
                    // mod2 (PC models)
                    0x6D6F6432 => {
                        let mut asset_data = None;
                        if potential_size < 0xD0 + 0xC {
                            return Err("mod2 tag is too small");
                        }

                        let memory_address = *memory_address.as_ref().unwrap();
                        let geometries_reflexive = match Reflexive::serialize(&tag_data[0xD0..],memory_address,memory_address + potential_size as u32, 0x30) {
                            Ok(n) => n,
                            Err(_) => return Err("invalid address on model geometry reflexive")
                        };

                        if geometries_reflexive.count > 0 {
                            let offset = (geometries_reflexive.address - memory_address) as usize;
                            let geometries = tag_data[offset .. offset + geometries_reflexive.count * 0x30].to_owned();
                            let mut asset_data_len = 0;

                            for i in 0..geometries_reflexive.count as usize {
                                let geometry = &geometries[i * 0x30 .. (i+1)* 0x30];
                                let parts_reflexive = match Reflexive::serialize(&geometry[0x24..],memory_address,memory_address + potential_size as u32, 0x84) {
                                    Ok(n) => n,
                                    Err(_) => return Err("invalid address on model part reflexive")
                                };

                                if parts_reflexive.count == 0 {
                                    continue;
                                }

                                let offset = (parts_reflexive.address - memory_address) as usize;
                                let parts = &tag_data[offset .. offset + parts_reflexive.count * 0x84];

                                for p in 0..parts_reflexive.count {
                                    let part = &parts[p * 0x84 .. (p+1) * 0x84];
                                    let index_count = LittleEndian::read_u32(&part[0x48 + 0x0..]) as usize;
                                    let index_offset = LittleEndian::read_u32(&part[0x48 + 0x4..]) as usize;
                                    if LittleEndian::read_u32(&part[0x48 + 0x8..]) as usize != index_offset {
                                        return Err("invalid model index offset");
                                    }

                                    let index_size = index_count * 0x2 + 4;
                                    let index_end = index_size + index_offset as usize;
                                    if index_end > indices.len() {
                                        return Err("invalid model index offset/size");
                                    }

                                    let vertex_count = LittleEndian::read_u32(&part[0x58 + 0x0..]) as usize;
                                    let vertex_offset = LittleEndian::read_u32(&part[0x58 + 0xC..]) as usize;
                                    let vertex_size = vertex_count * 0x44;
                                    let vertex_end = vertex_offset + vertex_size;
                                    if vertex_end > vertices.len() {
                                        return Err("invalid model vertex offset/size");
                                    }

                                    asset_data_len += vertex_size + index_size;
                                }
                            }

                            if asset_data_len != 0 {
                                let mut asset_data_vec = Vec::new();
                                asset_data_vec.reserve_exact(asset_data_len as usize);

                                for i in 0..geometries_reflexive.count as usize {
                                    let geometry = &geometries[i * 0x30 .. (i+1)* 0x30];
                                    let parts_reflexive = Reflexive::serialize(&geometry[0x24..],memory_address,memory_address + potential_size as u32, 0x84).unwrap();

                                    if parts_reflexive.count == 0 {
                                        continue;
                                    }

                                    let offset = (parts_reflexive.address - memory_address) as usize;
                                    let mut parts = &mut tag_data[offset .. offset + parts_reflexive.count * 0x84];

                                    for p in 0..parts_reflexive.count {
                                        let mut part = &mut parts[p * 0x84 .. (p+1) * 0x84];
                                        let index_count = LittleEndian::read_u32(&part[0x48 + 0x0..]) as usize;
                                        let index_offset = LittleEndian::read_u32(&part[0x48 + 0x4..]) as usize;

                                        let index_size = index_count * 0x2 + 4;
                                        let index_end = index_size + index_offset as usize;

                                        let vertex_count = LittleEndian::read_u32(&part[0x58 + 0x0..]) as usize;
                                        let vertex_offset = LittleEndian::read_u32(&part[0x58 + 0xC..]) as usize;
                                        let vertex_size = vertex_count * 0x44;
                                        let vertex_end = vertex_offset + vertex_size;

                                        let asset_data_len = asset_data_vec.len() as u32;

                                        // Write vertex offset.
                                        LittleEndian::write_u32(&mut part[0x58 + 0xC..], asset_data_len);

                                        // Write index offset.
                                        LittleEndian::write_u32(&mut part[0x48 + 0x4..], asset_data_len + vertex_size as u32);
                                        LittleEndian::write_u32(&mut part[0x48 + 0x8..], asset_data_len + vertex_size as u32);

                                        asset_data_vec.extend_from_slice(&vertices[vertex_offset .. vertex_end]);
                                        asset_data_vec.extend_from_slice(&indices[index_offset .. index_end]);
                                    }
                                }
                                asset_data = Some(asset_data_vec);
                            }
                        }
                        asset_data
                    },
                    // All other tag classes don't have asset data.
                    _ => None
                };
                data = Some(tag_data);
            }

            // Success!
            tags.push(Tag::new(
                tag_name,
                classes,
                data,
                asset_data,
                implicit,
                resource_index,
                memory_address,
            ));
        }

        Ok(Map {
            kind : (Game::from_u32(LittleEndian::read_u32(&cache_file[0x4..])),MapType::from_u32(LittleEndian::read_u32(&cache_file[0x60..]))),
            name : name,
            build : build,
            tag_array : TagArray::new(tags,scenario_tag)
        })
    }

    /// This function creates a cache file from the Map struct.
    ///
    /// If the cache file is over 2 GiB or an error occurs, this function will result in an `Err`.
    pub fn as_cache_file(&self) -> Result<Vec<u8>,&'static str> {
        let mut header = [0u8 ; 0x800];
        LittleEndian::write_u32(&mut header[0x0..],0x68656164);
        LittleEndian::write_u32(&mut header[0x7FC..],0x666F6F74);
        LittleEndian::write_u32(&mut header[0x4..], self.kind.0.as_u32());
        LittleEndian::write_u32(&mut header[0x60..], self.kind.1.as_u32());
        let name_latin1 = try!(encode_latin1_string(&self.name));
        if name_latin1.len() > 0x1F {
            return Err("map name exceeds 31 characters");
        }
        let build_latin1 = try!(encode_latin1_string(&self.build));
        if build_latin1.len() > 0x1F {
            return Err("build exceeds 31 characters");
        }
        let write_bytes = |destination : &mut [u8], source : &[u8]| {
            assert!(source.len() < destination.len());
            for i in 0..source.len() {
                unsafe { *destination.get_unchecked_mut(i) = *source.get_unchecked(i) };
            }
        };
        write_bytes(&mut header[0x20..], &name_latin1);
        write_bytes(&mut header[0x40..], &build_latin1);

        let mut sbsp_data : Vec<u8> = Vec::new();
        let mut resource_data : Vec<u8> = Vec::new();

        let mut model_vertex_data : Vec<u8> = Vec::new();
        let mut model_index_data : Vec<u8> = Vec::new();

        let mut sbsp_length = 0;
        let mut sbsp_count = 0;

        let mut resource_length = 0;

        let mut new_tag_array = self.tag_array.tags().to_owned();

        let mut tag_paths_length = 0;

        // First pass: Get data and tag paths length.
        for tag in &new_tag_array {
            tag_paths_length += try!(encode_latin1_string(&tag.tag_path)).len() + 1;
            if tag.data.is_none() {
                continue;
            }
            match tag.tag_class.0 {
                // Bitmaps
                0x6269746D => resource_length += {
                    match tag.asset_data.as_ref() {
                        Some(n) => n.len(),
                        None => 0
                    }
                },
                // Sounds
                0x736E6421 => resource_length += {
                    match tag.asset_data.as_ref() {
                        Some(n) => n.len(),
                        None => 0
                    }
                },
                // SBSPs
                0x73627370 => {
                    sbsp_length += tag.data.as_ref().unwrap().len();
                    sbsp_count += 1;
                },
                _ => continue
            }
        }

        let padded_sbsp_length = pad_32(sbsp_length);
        let padded_resource_data_length = pad_32(resource_length);

        sbsp_data.reserve_exact(padded_sbsp_length);
        resource_data.reserve_exact(padded_resource_data_length);

        // Tag ID, File offset
        let mut sbsps : Vec<(usize, usize)> = Vec::new();
        sbsps.reserve_exact(sbsp_count);

        match self.tag_array.principal_tag() {
            Some(n) => if n > new_tag_array.len() {
                return Err("invalid principal scenario tag")
            },
            None => if sbsp_count != 0 {
                return Err("orphaned sbsp tags");
            }
        }

        let sbsp_file_offset = header.len();
        let resource_file_offset = sbsp_file_offset + sbsp_length;

        let mut part_count = 0;

        let mut cached_tag_array = Vec::new();
        cached_tag_array.resize(0x20 * new_tag_array.len(),0);
        let cached_tag_array_len = cached_tag_array.len();

        let mut tag_paths : Vec<u8> = Vec::new();
        let padded_tag_paths_length = pad_32(tag_paths_length);
        tag_paths.reserve_exact(padded_tag_paths_length);

        let tag_header_address = 0x40440000u32;

        let mut total_tag_data = 0;

        // Second pass: Write data and work on the tag array.
        for tag_index in 0..new_tag_array.len() {
            let mut tag = unsafe { new_tag_array.get_unchecked_mut(tag_index) };
            let mut tag_array_tag = &mut cached_tag_array[tag_index * 0x20 .. (tag_index + 1) * 0x20];

            LittleEndian::write_u32(&mut tag_array_tag[0x10..],tag_header_address + 0x28 + (cached_tag_array_len + tag_paths.len()) as u32);
            tag_paths.extend({
                let mut x = try!(encode_latin1_string(&tag.tag_path));
                x.push(0);
                x
            });

            match tag.resource_index.as_ref() {
                Some(n) => {
                    if tag.data.is_some() {
                        return Err("tag has both data and a reference index")
                    }
                    LittleEndian::write_u32(&mut tag_array_tag[0x14..],*n);
                },
                None => ()
            }
            if tag.implicit {
                LittleEndian::write_u32(&mut tag_array_tag[0x18..],1);
            }

            LittleEndian::write_u32(&mut tag_array_tag[0x0..],tag.tag_class.0);
            LittleEndian::write_u32(&mut tag_array_tag[0x4..],tag.tag_class.1);
            LittleEndian::write_u32(&mut tag_array_tag[0x8..],tag.tag_class.2);
            LittleEndian::write_u32(&mut tag_array_tag[0xC..],tag_index_to_tag_id(tag_index));

            if tag.data.is_none() {
                continue;
            }
            else {
                let references = tag.references(&self.tag_array);
                for i in references {
                    tag.set_reference(&i);
                }
            }

            let memory_address = *tag.memory_address_from_offset(0).as_ref().unwrap();

            match tag.tag_class.0 {
                // Get internalized bitmaps...
                0x6269746D => {
                    let asset_data = match tag.asset_data.as_mut() {
                        Some(n) => n,
                        None => continue
                    };

                    let mut tag_data = tag.data.as_mut().unwrap();

                    if tag_data.len() < 0x60 + 0xC {
                        return Err("bitmap tag is too small");
                    }

                    let bitmaps_reflexive = match Reflexive::serialize(&tag_data[0x60..],memory_address,memory_address + tag_data.len() as u32, 0x30) {
                        Ok(n) => n,
                        Err(_) => return Err("invalid address on bitmap reflexive")
                    };

                    if bitmaps_reflexive.count == 0 {
                        continue;
                    }

                    let offset = (bitmaps_reflexive.address - memory_address) as usize;
                    let mut bitmaps = &mut tag_data[offset .. offset + bitmaps_reflexive.count * 0x30];

                    for i in 0..bitmaps_reflexive.count {
                        let mut bitmap = &mut bitmaps[i * 0x30 .. (i+1)*0x30];
                        if bitmap[0xF] & 1 == 0 {
                            let data_offset = LittleEndian::read_u32(&bitmap[0x18..]) as usize;
                            let data_size = LittleEndian::read_u32(&bitmap[0x1C..]) as usize;

                            if data_offset + data_size > asset_data.len() {
                                return Err("invalid data offset on bitmap");
                            }

                            LittleEndian::write_u32(&mut bitmap[0x18..], (resource_file_offset + resource_data.len()) as u32);
                            resource_data.extend_from_slice(&asset_data[data_offset .. data_offset + data_size]);
                        }
                    }
                },
                // Get internalized sounds...
                0x736E6421 => {
                    let asset_data = match tag.asset_data.as_mut() {
                        Some(n) => n,
                        None => continue
                    };

                    let mut tag_data = tag.data.as_mut().unwrap();
                    let tag_data_len = tag_data.len();

                    if tag_data_len < 0x98 + 0xC {
                        return Err("sound tag is too small");
                    }

                    let ranges_reflexive = match Reflexive::serialize(&tag_data[0x98..],memory_address,memory_address + tag_data_len as u32, 0x48) {
                        Ok(n) => n,
                        Err(_) => return Err("invalid address on sound range reflexive")
                    };

                    if ranges_reflexive.count == 0 {
                        continue;
                    }

                    let offset = (ranges_reflexive.address - memory_address) as usize;
                    let ranges = tag_data[offset .. offset + ranges_reflexive.count * 0x48].to_owned();

                    for i in 0..ranges_reflexive.count as usize {
                        let range = &ranges[i * 0x48 .. (i+1)* 0x48];
                        let permutations_reflexive = match Reflexive::serialize(&range[0x3C..],memory_address,memory_address + tag_data_len as u32, 0x7C) {
                            Ok(n) => n,
                            Err(_) => return Err("invalid address on sound permutation reflexive")
                        };

                        if permutations_reflexive.count == 0 {
                            continue;
                        }

                        let offset = (permutations_reflexive.address - memory_address) as usize;
                        let mut permutations = &mut tag_data[offset .. offset + permutations_reflexive.count * 0x7C];

                        for p in 0..permutations_reflexive.count {
                            let mut sound = &mut permutations[p * 0x7C .. (p+1) * 0x7C];
                            if sound[0x44] & 1 == 0 {
                                let data_offset = LittleEndian::read_u32(&sound[0x48..]) as usize;
                                let data_size = LittleEndian::read_u32(&sound[0x40..]) as usize;
                                if data_offset + data_size > asset_data.len() {
                                    return Err("sound points to invalid data")
                                }

                                LittleEndian::write_u32(&mut sound[0x48..], (resource_file_offset + resource_data.len()) as u32);
                                resource_data.extend_from_slice(&asset_data[data_offset .. data_offset + data_size]);
                            }
                        }
                    }
                },
                // Get models...
                0x6D6F6432 => {
                    let asset_data = match tag.asset_data.as_mut() {
                        Some(n) => n,
                        None => continue
                    };

                    let mut tag_data = tag.data.as_mut().unwrap();
                    let tag_data_len = tag_data.len();

                    if tag_data_len < 0xD0 + 0xC {
                        return Err("mod2 tag is too small");
                    }

                    let geometries_reflexive = match Reflexive::serialize(&tag_data[0xD0..],memory_address,memory_address + tag_data_len as u32, 0x30) {
                        Ok(n) => n,
                        Err(_) => return Err("invalid address on model geometry reflexive")
                    };

                    if geometries_reflexive.count == 0 {
                        continue;
                    }

                    let offset = (geometries_reflexive.address - memory_address) as usize;
                    let geometries = tag_data[offset .. offset + geometries_reflexive.count * 0x30].to_owned();

                    for i in 0..geometries_reflexive.count as usize {
                        let geometry = &geometries[i * 0x30 .. (i+1)* 0x30];
                        let parts_reflexive = match Reflexive::serialize(&geometry[0x24..],memory_address,memory_address + tag_data_len as u32, 0x84) {
                            Ok(n) => n,
                            Err(_) => return Err("invalid address on model part reflexive")
                        };

                        if parts_reflexive.count == 0 {
                            continue;
                        }

                        let offset = (parts_reflexive.address - memory_address) as usize;
                        let mut parts = &mut tag_data[offset .. offset + parts_reflexive.count * 0x84];

                        for p in 0..parts_reflexive.count {
                            let mut part = &mut parts[p * 0x84 .. (p+1) * 0x84];
                            let index_count = LittleEndian::read_u32(&part[0x48 + 0x0..]) as usize;
                            let index_offset = LittleEndian::read_u32(&part[0x48 + 0x4..]) as usize;
                            if LittleEndian::read_u32(&part[0x48 + 0x8..]) as usize != index_offset {
                                return Err("invalid model index offset");
                            }

                            let index_size = index_count * 0x2 + 4;
                            let index_end = index_size + index_offset as usize;
                            if index_end > asset_data.len() {
                                return Err("invalid model index offset/size");
                            }

                            let vertex_count = LittleEndian::read_u32(&part[0x58 + 0x0..]) as usize;
                            let vertex_offset = LittleEndian::read_u32(&part[0x58 + 0xC..]) as usize;
                            let vertex_size = vertex_count * 0x44;
                            let vertex_end = vertex_offset + vertex_size;
                            if vertex_end > asset_data.len() {
                                return Err("invalid model vertex offset/size");
                            }

                            // Write vertex offset.
                            LittleEndian::write_u32(&mut part[0x58 + 0xC..], model_vertex_data.len() as u32);

                            // Write index offset.
                            LittleEndian::write_u32(&mut part[0x48 + 0x4..], model_index_data.len() as u32);
                            LittleEndian::write_u32(&mut part[0x48 + 0x8..], model_index_data.len() as u32);

                            model_vertex_data.extend_from_slice(&asset_data[vertex_offset .. vertex_end]);
                            model_index_data.extend_from_slice(&asset_data[index_offset .. index_end]);

                            part_count += 1;
                        }
                    }
                },
                // Get sbsp tags...
                0x73627370 => {
                    sbsps.push((tag_index, sbsp_data.len()));
                    sbsp_data.append(&mut tag.data.as_mut().unwrap());
                    tag.data = None;
                },
                _ => continue
            }
            tag.asset_data = None;
            total_tag_data += match tag.data.as_ref() {
                Some(n) => n.len(),
                None => 0
            };
        }

        sbsp_data.resize(padded_sbsp_length,0);
        resource_data.resize(padded_resource_data_length,0);
        assert!(padded_resource_data_length >= resource_length);

        let mut model_data = Vec::new();
        let vertex_size = model_vertex_data.len();
        let mut model_data_length = vertex_size + model_index_data.len();
        model_data_length = pad_32(model_data_length);
        model_data.reserve_exact(model_data_length);
        model_data.append(&mut model_vertex_data);
        model_data.append(&mut model_index_data);
        model_data.resize(model_data_length,0);

        let model_data_offset = padded_sbsp_length + padded_resource_data_length + header.len();
        let meta_offset = model_data_offset + model_data_length;
        LittleEndian::write_u32(&mut header[0x10..], meta_offset as u32);

        // Write tag data header
        let mut tag_data = {
            let mut tag_header = [0u8; 0x28];
            let tag_header_len = tag_header.len();

            // Tag array address
            LittleEndian::write_u32(&mut tag_header[0x0..], tag_header_address + tag_header_len as u32);

            // Principal scenario tag
            LittleEndian::write_u32(&mut tag_header[0x4..], match self.tag_array.principal_tag().as_ref() {
                Some(n) => tag_index_to_tag_id(*n),
                None => 0xFFFFFFFF
            });

            // Random number
            LittleEndian::write_u32(&mut tag_header[0x8..], 0x00010000);

            // Tag count
            LittleEndian::write_u32(&mut tag_header[0xC..], new_tag_array.len() as u32);

            // Part count
            LittleEndian::write_u32(&mut tag_header[0x10..], part_count as u32);
            LittleEndian::write_u32(&mut tag_header[0x18..], part_count as u32);

            // Model offset
            LittleEndian::write_u32(&mut tag_header[0x14..], model_data_offset as u32);

            // Vertex size
            LittleEndian::write_u32(&mut tag_header[0x1C..], vertex_size as u32);

            // Model size
            LittleEndian::write_u32(&mut tag_header[0x20..], model_data_length as u32);

            // "tags"
            LittleEndian::write_u32(&mut tag_header[0x24..], 0x74616773);

            tag_header.to_owned()
        };

        let first_tag_address = tag_header_address + 0x28 + (cached_tag_array_len + padded_tag_paths_length) as u32;
        tag_paths.resize(padded_tag_paths_length,0);

        let mut tag_meta_data : Vec<u8> = Vec::new();
        tag_meta_data.reserve_exact(total_tag_data);

        // Third pass: Build tag data
        for tag_index in 0..new_tag_array.len() {
            let tag = unsafe { new_tag_array.get_unchecked_mut(tag_index) };
            if tag.data.is_none() {
                continue;
            }
            let new_address = first_tag_address + tag_meta_data.len() as u32;
            tag.set_memory_address(new_address);
            tag_meta_data.extend_from_slice(tag.data.as_ref().unwrap());
            LittleEndian::write_u32(&mut cached_tag_array[tag_index * 0x20 + 0x14..], new_address);
        }

        tag_data.append(&mut cached_tag_array);
        tag_data.append(&mut tag_paths);
        tag_data.append(&mut tag_meta_data);

        let tag_data_length = tag_data.len();

        let file_size = meta_offset + tag_data.len();

        let mut new_cache_file = Vec::new();
        new_cache_file.reserve_exact(file_size);
        new_cache_file.append(&mut header.to_owned());
        new_cache_file.append(&mut sbsp_data);
        new_cache_file.append(&mut resource_data);
        new_cache_file.append(&mut model_data);
        new_cache_file.append(&mut tag_data);

        let new_cache_file_len = new_cache_file.len();

        if new_cache_file_len > 0x7FFFFFFF {
            return Err("cache file too big")
        }

        LittleEndian::write_u32(&mut new_cache_file[0x8..], new_cache_file_len as u32);
        LittleEndian::write_u32(&mut new_cache_file[0x14..], tag_data_length as u32);

        Ok(new_cache_file)
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

// This function will create an ISO 8859-1 vec from a string
fn encode_latin1_string(string : &str) -> Result<Vec<u8>,&'static str> {
    match ISO_8859_1.encode(&string, EncoderTrap::Strict) {
        Ok(n) => Ok(n),
        Err(_) => Err("failed to encode string")
    }
}

// Convenience for reading a tag reflexive.
struct Reflexive {
    pub count : usize,
    pub address : u32,
    pub unused : u32
}

impl Reflexive {
    pub fn serialize(data : &[u8], min_address : u32, max_address : u32, reflexive_size : usize) -> Result<Reflexive,&'static str> {
        if data.len() < 0xC {
            Err("data too small")
        }
        else {
            let address = LittleEndian::read_u32(&data[4..]);
            let count = LittleEndian::read_u32(&data[0..]) as usize;

            if count > 0 && (address >= max_address || address < min_address || count * reflexive_size + (address as usize) > (max_address as usize)) {
                Err("data exceeds address range")
            }
            else {
                Ok(Reflexive {
                    count : LittleEndian::read_u32(&data[0..]) as usize,
                    address : LittleEndian::read_u32(&data[4..]),
                    unused : LittleEndian::read_u32(&data[8..])
                })
            }
        }
    }
}

/// Add padding for 32-bit word alignment.
pub fn pad_32(length : usize) -> usize {
    length + (4 - (length % 4)) % 4
}
