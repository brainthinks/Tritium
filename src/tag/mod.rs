//! Module for handling tags and asset data

extern crate byteorder;
use self::byteorder::{ByteOrder,LittleEndian};

mod tag_array;
pub use self::tag_array::*;

const BITM : u32 = 0x6269746D;
const SND : u32 = 0x736E6421;
const OBJE : u32 = 0x6F626A65;
const SBSP : u32 = 0x73627370;
const SCNR : u32 = 0x73636E72;
const EFFE : u32 = 0x65666665;
const ANTR : u32 = 0x616E7472;

const JPT : u32 = 0x6A707421;

#[derive(Clone)]
/// Tags can vary on how they reference other tags.
pub enum TagReferenceType {
    /// TagIDs are really just 32-bit integers that refer to another tag.
    TagID,

    /// Dependencies are a set of four 32-bit integers that refer to another tag.
    Dependency
}

#[derive(Clone)]
/// Tag references are used to have tags link to other tags.
pub struct TagReference {
    pub tag_index : usize,
    pub offset : usize,
    pub tag_class : u32,
    pub reference_type : TagReferenceType
}

#[derive(Clone)]
/// Halo tags contain data, including the tag data itself as well as any assets it may contain.
pub struct Tag {
    /// Tag names are used to identify the tag and use a Windows path (backslashes).
    pub tag_path : String,

    /// Tag classes are used to identify what kind of a tag the tag is.
    ///
    /// There are three classes. The primary class is used by the engine and for references.
    pub tag_class : (u32,u32,u32),

    /// Tag data is used for information about the tag. Some tags do not have data stored in the
    /// map file.
    pub data : Option<Vec<u8>>,

    /// Asset data is raw data used by models, internalized bitmaps, and internalized sounds.
    pub asset_data : Option<Vec<u8>>,

    /// This tag is an implicit reference.
    pub implicit : bool,

    /// Some tags use an index to a resource located in bitmaps.map or loc.map, rather than store
    /// data in the map file.
    pub resource_index : Option<u32>,

    /// This is an address used by Halo.
    pub memory_address : Option<u32>
}
impl Tag {
    /// Create a new Tag. This consumes all of the data used.
    pub fn new(path : String, classes : (u32,u32,u32), data : Option<Vec<u8>>, asset_data : Option<Vec<u8>>, implicit: bool, resource_index : Option<u32>, memory_address : Option<u32>) -> Tag {
        Tag {
            tag_path : path,
            tag_class : classes,
            data : data,
            asset_data : asset_data,
            implicit : implicit,
            resource_index : resource_index,
            memory_address : memory_address
        }
    }

    /// Convert an offset to a memory address.
    ///
    /// Returns `None` if the offset is outside of the tag data.
    ///
    /// Panics if there is no memory address used by this tag.
    pub fn memory_address_from_offset(&self, offset : usize) -> Option<u32> {
        if self.data.as_ref().unwrap().len() < offset {
            None
        }
        else {
            Some(self.memory_address.as_ref().unwrap() + offset as u32)
        }
    }

    /// Convert a memory address to an offset.
    ///
    /// Returns `None` if the address is outside of the tag data.
    ///
    /// Panics if there is no memory address used by this tag.
    pub fn offset_from_memory_address(&self, address : u32) -> Option<usize> {
        let memory_address = *self.memory_address.as_ref().unwrap();
        if memory_address > address {
            None
        }
        else {
            let offset = (address - memory_address) as usize;
            if offset > self.data.as_ref().unwrap().len() {
                None
            }
            else {
                Some(offset)
            }
        }
    }

    /// Change the memory address to something else.
    ///
    /// Panics if the address given cannot be used, if there is no memory address used by this tag,
    /// or if there is no tag data used by this tag.
    pub fn set_memory_address(&mut self, new_address : u32) {
        if new_address > (0x7FFFFFFF - self.data.as_mut().unwrap().len() as u32) {
            panic!("attempted to set an invalid memory address")
        }
        let memory_address = *self.memory_address.as_ref().unwrap();

        if new_address > memory_address {
            self.offset_pointers(0,new_address - memory_address,false)
        }
        else {
            self.offset_pointers(0,memory_address - new_address,true)
        }

        self.memory_address = Some(new_address);
    }

    /// Calculate all of the references in this tag and return an index of them.
    pub fn references(&self, tag_array : &TagArray) -> Vec<TagReference> {
        if self.data.is_none() {
            return Vec::new();
        }
        let mut references = Vec::new();
        let data = self.data.as_ref().unwrap();

        let add_predicted_resources = |offset : usize| {
            let mut p_references = Vec::new();
            let data = self.data.as_ref().unwrap();
            let count = LittleEndian::read_u32(&data[offset ..]) as usize;
            if count == 0 {
                return p_references;
            }
            let resource_offset = match self.offset_from_memory_address(LittleEndian::read_u32(&data[offset + 4..])) {
                Some(n) => n,
                None => panic!("invalid tag when trying to find predicted resources")
            };
            let resource_data = &data[resource_offset .. resource_offset + 8 * count];
            let tag_array_tags = tag_array.tags();
            let tag_count = tag_array_tags.len();
            for i in 0..count {
                let resource = &resource_data[i * 8 .. (i + 1) * 8];
                let tag_type = LittleEndian::read_u16(&resource[0..]);
                let tag_identity = LittleEndian::read_u32(&resource[4..]);
                if tag_identity == 0xFFFFFFFF {
                    continue;
                }
                let tag_index = tag_identity as usize & 0xFFFF;
                assert!(tag_index < tag_count,"invalid predicted resource");
                let tag = &tag_array_tags[tag_index];
                let tag_class = tag.tag_class.0;
                assert!((tag_type == 0 && tag_class == BITM) || (tag_type == 1 && tag_class == SND),"tag_type {}; tag_class : {}", tag_type, tag_class);
                p_references.push(TagReference {
                    tag_index : tag_index,
                    offset : resource_offset + i * 0x8 + 4,
                    tag_class : tag_class,
                    reference_type : TagReferenceType::TagID
                });
            }
            p_references
        };

        match self.tag_class.0 {
            ANTR => {
                let sounds_count = LittleEndian::read_u32(&data[0x54..]) as usize;
                if sounds_count > 0 {
                    let sounds_offset = match self.offset_from_memory_address(LittleEndian::read_u32(&data[0x54 + 4..])) {
                        Some(n) => n,
                        None => panic!("invalid animation tag")
                    };
                    let sounds = &data[sounds_offset .. sounds_count * 20 + sounds_offset];
                    for i in 0..sounds_count {
                        let sound = &sounds[i*20 .. (i+1)*20];
                        let identity = LittleEndian::read_u32(&sound[0x0 + 0xC..]);
                        if identity == 0xFFFFFFFF {
                            continue;
                        }
                        references.push(TagReference {
                            tag_index : identity as usize & 0xFFFF,
                            offset : sounds_offset + i * 20,
                            tag_class : LittleEndian::read_u32(&sound[0x0..]),
                            reference_type : TagReferenceType::Dependency
                        })
                    }
                }
            },
            BITM => {
                let bitmaps_count = LittleEndian::read_u32(&data[0x60..]) as usize;
                let bitmaps_address = LittleEndian::read_u32(&data[0x64..]);

                let bitmaps_offset = match self.offset_from_memory_address(bitmaps_address) {
                    Some(n) => n,
                    None => return references
                };

                if bitmaps_offset + bitmaps_count * 0x30 > data.len() {
                    return references;
                }

                let bitmaps = &data[bitmaps_offset .. bitmaps_offset + bitmaps_count * 0x30];

                for bitmap in 0..bitmaps_count {
                    let bitmap_data = &bitmaps[bitmap * 0x30 .. (bitmap + 1) * 0x30];
                    let identity = LittleEndian::read_u32(&bitmap_data[0x20..]);
                    if identity == 0xFFFFFFFF {
                        continue;
                    }
                    references.push(TagReference {
                        tag_index : identity as usize & 0xFFFF,
                        offset : bitmaps_offset + bitmap * 0x30 + 0x20,
                        tag_class : 0x6269746D,
                        reference_type : TagReferenceType::TagID
                    })
                }
            },
            EFFE => {
                let event_count = LittleEndian::read_u32(&data[0x34..]) as usize;
                if event_count > 0 {
                    let event_offset = match self.offset_from_memory_address(LittleEndian::read_u32(&data[0x34 + 4..])) {
                        Some(n) => n,
                        None => panic!("invalid effe tag")
                    };
                    let events = &data[event_offset .. event_offset + event_count * 68];
                    for e in 0..event_count {
                        let event = &events[e * 68 .. (e+1) * 68];
                        let part_count = LittleEndian::read_u32(&event[0x2C..]) as usize;

                        if part_count > 0 {
                            let part_offset = match self.offset_from_memory_address(LittleEndian::read_u32(&event[0x2C + 4..])) {
                                Some(n) => n,
                                None => panic!("invalid effe tag")
                            };

                            let parts = &data[part_offset .. part_offset + part_count * 104];
                            for p in 0..part_count {
                                let part = &parts[p * 104 .. (p+1) * 104];
                                let identity = LittleEndian::read_u32(&part[0x18 + 0xC..]);
                                if identity == 0xFFFFFFFF {
                                    continue;
                                }
                                let id = identity as usize & 0xFFFF;
                                assert!(id < tag_array.tags().len(), "{} < {}", id, tag_array.tags().len());
                                references.push(TagReference {
                                    tag_index : id,
                                    offset : part_offset + p * 104 + 0x18,
                                    tag_class : LittleEndian::read_u32(&part[0x18..]),
                                    reference_type : TagReferenceType::Dependency
                                });
                            }
                        }

                        let particle_count = LittleEndian::read_u32(&event[0x38..]) as usize;
                        if particle_count > 0 {
                            let particle_offset = match self.offset_from_memory_address(LittleEndian::read_u32(&event[0x38 + 4..])) {
                                Some(n) => n,
                                None => panic!("invalid effe tag")
                            };
                            let particles = &data[particle_offset .. particle_offset + particle_count * 232];
                            for p in 0..particle_count {
                                let particle = &particles[p * 232 .. (p+1) * 232];
                                let identity = LittleEndian::read_u32(&particle[0x54 + 0xC..]);
                                if identity == 0xFFFFFFFF {
                                    continue;
                                }
                                let id = identity as usize & 0xFFFF;
                                assert!(id < tag_array.tags().len(), "{} < {}", id, tag_array.tags().len());
                                references.push(TagReference {
                                    tag_index : id,
                                    offset : particle_offset + p * 232 + 0x54,
                                    tag_class : LittleEndian::read_u32(&particle[0x54..]),
                                    reference_type : TagReferenceType::Dependency
                                });
                            }
                        }
                    }
                }
            },
            JPT => {
                let identity = LittleEndian::read_u32(&data[0x114 + 0xC..]);
                if identity != 0xFFFFFFFF {
                    references.push(TagReference {
                        tag_index : identity as usize & 0xFFFF,
                        offset : 0x114,
                        tag_class : LittleEndian::read_u32(&data[0x114..]),
                        reference_type : TagReferenceType::Dependency
                    });
                }
            },
            SND => {
                let promo_sound_id = LittleEndian::read_u32(&data[0x70 + 0xC..]) as usize;
                if promo_sound_id != 0xFFFFFFFF {
                    assert!(promo_sound_id & 0xFFFF < tag_array.tags().len());
                    references.push(TagReference {
                        tag_index : promo_sound_id & 0xFFFF,
                        offset : 0x70,
                        tag_class : SND,
                        reference_type : TagReferenceType::Dependency
                    });
                }
                let count = LittleEndian::read_u32(&data[0x98..]) as usize;
                let offset = match self.offset_from_memory_address(LittleEndian::read_u32(&data[0x98 + 4..])) {
                    Some(n) => n,
                    None => panic!("invalid snd! tag")
                };
                let ranges = &data[offset .. offset + count * 0x48].to_owned();
                for i in 0..count {
                    let range = &ranges[i * 0x48 .. (i+1)* 0x48];
                    let permutations_count = LittleEndian::read_u32(&range[0x3C..]) as usize;
                    let permutations_offset = match self.offset_from_memory_address(LittleEndian::read_u32(&range[0x3C+4..])) {
                        Some(n) => n,
                        None => panic!("invalid snd! range")
                    };
                    let permutations = &data[permutations_offset .. permutations_offset + permutations_count * 124];
                    for p in 0..permutations_count {
                        let permutation = &permutations[p * 124 .. (p+1) * 124];
                        for k in 0..2 {
                            let identity = LittleEndian::read_u32(&permutation[0x34 + k * 8..]);
                            if identity == 0xFFFFFFFF {
                                continue;
                            }
                            let id = identity as usize & 0xFFFF;
                            assert!(id < tag_array.tags().len(), "{} < {}", id, tag_array.tags().len());
                            references.push(TagReference {
                                tag_index : id,
                                offset : p * 124 + k * 8 + 0x34 + permutations_offset,
                                tag_class : SND,
                                reference_type : TagReferenceType::TagID
                            });
                        }

                    }
                }
            },
            // Everything else!
            _ => {
                let data_length = data.len();
                if data_length < 16 {
                    return references;
                }
                let tag_array_tag_length = tag_array.tags().len();

                let mut i = 0;
                let iterator = 4;
                loop {
                    if i + 16 - 1 >= data_length {
                        break;
                    }
                    let data = &data[i..i+0x10];
                    let tag_identity = LittleEndian::read_u32(&data[0xC..]);
                    let tag_index = tag_identity as usize & 0xFFFF;
                    if tag_array_tag_length <= tag_index || tag_identity == 0xFFFFFFFF {
                        i += iterator;
                        continue;
                    }

                    let tag_class = LittleEndian::read_u32(&data[0x0..]);
                    if unsafe { tag_array.tags().get_unchecked(tag_index).tag_class.0 } == tag_class {
                        references.push(TagReference {
                            tag_index : tag_index,
                            offset : i,
                            tag_class : tag_class,
                            reference_type : TagReferenceType::Dependency
                        });
                        i += 16;
                    }
                    else {
                        i += iterator;
                    }
                }
            }
        }
        if self.tag_class.0 == OBJE || self.tag_class.1 == OBJE || self.tag_class.2 == OBJE {
            for i in add_predicted_resources(0x170) {
                references.push(i);
            }
        }
        if self.tag_class.0 == SCNR {
            for i in add_predicted_resources(0xEC) {
                references.push(i);
            }
        }
        if self.tag_class.0 == SBSP {
            let clusters_count = LittleEndian::read_u32(&data[0x14C..]) as usize;
            if clusters_count > 0 {
                let clusters_offset = match self.offset_from_memory_address(LittleEndian::read_u32(&data[0x14C + 4..])) {
                    Some(n) => n,
                    None => panic!("invalid sbsp tag when trying to find predicted resources")
                };
                for i in 0..clusters_count {
                    for i in add_predicted_resources(clusters_offset + i * 104 + 0x28) {
                        references.push(i);
                    }
                }
            }
        }

        references
    }

    /// Apply a tag reference to this tag.
    ///
    /// This function may panic if the offset is invalid or if the tag does not have any data.
    pub fn set_reference(&mut self, reference : &TagReference) {
        let mut tag_data = self.data.as_mut().unwrap();
        match reference.reference_type {
            TagReferenceType::TagID => {
                LittleEndian::write_u32(&mut tag_data[reference.offset..], tag_index_to_tag_id(reference.tag_index));
            }
            TagReferenceType::Dependency => {
                LittleEndian::write_u32(&mut tag_data[reference.offset..], reference.tag_class as u32);
                LittleEndian::write_u32(&mut tag_data[reference.offset + 0xC..], tag_index_to_tag_id(reference.tag_index));
            }
        }
    }

    /// Insert bytes into a section of the tag data while also adjusting memory pointers that use
    /// any data after it. This may be useful when inserting structures into the tag data.
    ///
    /// This function will panic if there is no tag data or memory address used by the tag.
    pub fn create_data(&mut self, offset : usize, size : usize, value : u8) {
        let mut p = Vec::new();
        p.resize(size,value);
        self.insert_data(offset,&p);
    }

    /// Insert bytes into a section of the tag data while also adjusting memory pointers that use
    /// any data at that location.
    ///
    /// This function will panic if there is no tag data or memory address used by the tag.
    pub fn insert_data(&mut self, offset : usize, data : &[u8]) {
        self.offset_pointers(offset,data.len() as u32,false);
        self.data = Some({
            let mut tag_data = self.data.as_mut().unwrap();
            let mut a = tag_data[0..offset].to_owned();
            a.reserve(tag_data.len() + data.len());
            a.append(&mut data.to_owned());
            a.append(&mut tag_data[offset..].to_owned());
            a
        });
    }

    /// Delete bytes into a section of the tag data while also adjusting memory pointers that use
    /// any data after the chunk. This may be useful when destroying structures into the tag data.
    ///
    /// This function will panic if there is no tag data or memory address used by the tag.
    pub fn delete_data(&mut self, offset : usize, size : usize) {
        self.offset_pointers(offset+size,size as u32,true);
        let mut tag_data = self.data.as_mut().unwrap();
        for _ in 0..size {
            tag_data.remove(offset);
        }
    }

    /// Offset pointers that point to the offset or after without adding or removing any data.
    /// Setting `subtract` to true will decrease the pointers instead of increasing them.
    ///
    /// Pointers that end up pointing outside of the data may no longer be pattern-matched.
    ///
    /// This function will panic if there is no memory address or data used by the tag.
    pub fn offset_pointers(&mut self, offset : usize, size : u32, subtract : bool) {
        let min_memory_address = *self.memory_address.as_ref().unwrap() + offset as u32;
        let pointers = self.p_pointers();
        let mut tag_data = self.data.as_mut().unwrap();
        for i in pointers {
            let address = LittleEndian::read_u32(&tag_data[i..]);
            if address >= min_memory_address {
                LittleEndian::write_u32(
                    &mut tag_data[i..],
                    if subtract {
                        address - size
                    }
                    else {
                        address + size
                    }
                );
            }
        }
    }

    /// Find all of the pointers in the tag and return the offsets to them. Pattern matching will
    /// only find reflexives that point to data within the tag.
    ///
    /// This function will panic if there is no memory address or data used by the tag.
    fn p_pointers(&self) -> Vec<usize> {
        let tag_data = self.data.as_ref().unwrap();
        let memory_address = *self.memory_address.as_ref().unwrap();
        let memory_address_end = memory_address + tag_data.len() as u32;
        let mut pointers = Vec::new();

        match self.tag_class.0 {
            BITM => {
                let sequences_count = LittleEndian::read_u32(&tag_data[0x54..]) as usize;
                if sequences_count > 0 {
                    pointers.push(0x58);
                    let sequences_offset = self.offset_from_memory_address(LittleEndian::read_u32(&tag_data[0x58..])).unwrap();
                    let sequences = &tag_data[sequences_offset .. sequences_offset + sequences_count * 64];
                    for i in 0..sequences_count {
                        let sequence = &sequences[i * 64 .. (i+1)*64];
                        let seq_count = LittleEndian::read_u32(&sequence[0x34..]);
                        if seq_count > 0 {
                            pointers.push(i * 64 + sequences_offset + 0x38);
                            self.offset_from_memory_address(LittleEndian::read_u32(&sequence[0x38..])).unwrap();
                        }
                    }
                }
                let bitmaps_count = LittleEndian::read_u32(&tag_data[0x60..]);
                if bitmaps_count > 0 {
                    self.offset_from_memory_address(LittleEndian::read_u32(&tag_data[0x64..])).unwrap();
                    pointers.push(0x64);
                }
            },
            EFFE => {
                let location_count = LittleEndian::read_u32(&tag_data[0x28..]) as usize;
                if location_count > 0 {
                    match self.offset_from_memory_address(LittleEndian::read_u32(&tag_data[0x28 + 4..])) {
                        Some(_) => pointers.push(0x28 + 4),
                        None => panic!("invalid effe tag")
                    };
                }
                let event_count = LittleEndian::read_u32(&tag_data[0x34..]) as usize;
                if event_count > 0 {
                    let event_offset = match self.offset_from_memory_address(LittleEndian::read_u32(&tag_data[0x34 + 4..])) {
                        Some(n) => n,
                        None => panic!("invalid effe tag")
                    };
                    pointers.push(0x34 + 4);
                    let events = &tag_data[event_offset .. event_offset + event_count * 68];
                    for e in 0..event_count {
                        let event = &events[e * 68 .. (e+1) * 68];
                        let part_count = LittleEndian::read_u32(&event[0x2C..]) as usize;
                        if part_count > 0 {
                            match self.offset_from_memory_address(LittleEndian::read_u32(&event[0x2C + 4..])) {
                                Some(_) => pointers.push(event_offset + e * 68 + 0x2C + 4),
                                None => panic!("invalid effe tag {}",LittleEndian::read_u32(&event[0x2C + 4..]))
                            };
                        }

                        let particle_count = LittleEndian::read_u32(&event[0x38..]) as usize;
                        if particle_count > 0 {
                            match self.offset_from_memory_address(LittleEndian::read_u32(&event[0x38 + 4..])) {
                                Some(_) => pointers.push(event_offset + e * 68 + 0x38 + 4),
                                None => panic!("invalid effe tag")
                            };
                        }
                    }
                }
            },
            JPT => (),
            SCNR => {
                let mut maybe_add_pointer = |offset : usize| {
                    let x = LittleEndian::read_u32(&tag_data[offset ..]);
                    if x != 0 {
                        pointers.push(offset);
                    }
                };
                maybe_add_pointer(0x30 + 4);
                maybe_add_pointer(0x40 + 4);
                maybe_add_pointer(0xEC + 4);
                maybe_add_pointer(0xF8 + 4);
                maybe_add_pointer(0x110);
                match self.offset_from_memory_address(LittleEndian::read_u32(&tag_data[0x118 + 4..])) {
                    Some(n) => {
                        let comments_count = LittleEndian::read_u32(&tag_data[0x118..]) as usize;
                        for i in 0..comments_count {
                            maybe_add_pointer(n + i * 48 + 0x24 + 4);
                        }
                        maybe_add_pointer(0x118 + 4);
                    },
                    None => ()
                };
                maybe_add_pointer(0x204 + 4);
                maybe_add_pointer(0x210 + 4);
                maybe_add_pointer(0x21C + 4);
                maybe_add_pointer(0x228 + 4);
                maybe_add_pointer(0x234 + 4);
                maybe_add_pointer(0x240 + 4);
                maybe_add_pointer(0x24C + 4);
                maybe_add_pointer(0x258 + 4);
                maybe_add_pointer(0x264 + 4);
                maybe_add_pointer(0x270 + 4);
                maybe_add_pointer(0x27C + 4);
                maybe_add_pointer(0x288 + 4);
                maybe_add_pointer(0x294 + 4);
                maybe_add_pointer(0x2A0 + 4);
                maybe_add_pointer(0x2AC + 4);
                maybe_add_pointer(0x2B8 + 4);
                maybe_add_pointer(0x2C4 + 4);
                maybe_add_pointer(0x2D0 + 4);
                maybe_add_pointer(0x2DC + 4);
                maybe_add_pointer(0x2E8 + 4);
                maybe_add_pointer(0x348 + 4);
                maybe_add_pointer(0x354 + 4);
                maybe_add_pointer(0x360 + 4);
                match self.offset_from_memory_address(LittleEndian::read_u32(&tag_data[0x36C + 4..])) {
                    Some(n) => {
                        let recorded_animations_count = LittleEndian::read_u32(&tag_data[0x36C..]) as usize;
                        for i in 0..recorded_animations_count {
                            maybe_add_pointer(n + i * 64 + 0x38);
                        }
                        maybe_add_pointer(0x36C + 4);
                    },
                    None => ()
                };
                maybe_add_pointer(0x378 + 4);
                maybe_add_pointer(0x384 + 4);
                maybe_add_pointer(0x390 + 4);
                maybe_add_pointer(0x39C + 4);
                maybe_add_pointer(0x3A8 + 4);
                maybe_add_pointer(0x3B4 + 4);
                maybe_add_pointer(0x3C0 + 4);
                maybe_add_pointer(0x420 + 4);
                match self.offset_from_memory_address(LittleEndian::read_u32(&tag_data[0x42C + 4..])) {
                    Some(n) => {
                        let encounters_count = LittleEndian::read_u32(&tag_data[0x42C..]) as usize;
                        let data = &tag_data[n..];
                        for i in 0..encounters_count {
                            let block = &data[i * 176 .. (i+1) * 176];
                            match self.offset_from_memory_address(LittleEndian::read_u32(&block[0x80 + 4..])) {
                                Some(m) => {
                                    let squad_count = LittleEndian::read_u32(&block[0x80..]) as usize;
                                    for i in 0..squad_count {
                                        maybe_add_pointer(m + i * 232 + 0xC4 + 4);
                                        maybe_add_pointer(m + i * 232 + 0xD0 + 4);
                                    }
                                    maybe_add_pointer(n + i * 176 + 0x80 + 4);
                                },
                                None => ()
                            }
                            maybe_add_pointer(n + i * 176 + 0x8C + 4);
                            maybe_add_pointer(n + i * 176 + 0x98 + 4);
                            maybe_add_pointer(n + i * 176 + 0xA4 + 4);
                        }
                        maybe_add_pointer(0x42C + 4);
                    },
                    None => ()
                };
                match self.offset_from_memory_address(LittleEndian::read_u32(&tag_data[0x438 + 4..])) {
                    Some(n) => {
                        let commands_count = LittleEndian::read_u32(&tag_data[0x438..]) as usize;
                        for i in 0..commands_count {
                            maybe_add_pointer(n + i * 96 + 0x30 + 4);
                            maybe_add_pointer(n + i * 96 + 0x3C + 4);
                        }
                        maybe_add_pointer(0x438 + 4);
                    },
                    None => ()
                };
                maybe_add_pointer(0x444 + 4);
                maybe_add_pointer(0x450 + 4);
                maybe_add_pointer(0x45C + 4);
                match self.offset_from_memory_address(LittleEndian::read_u32(&tag_data[0x468 + 4..])) {
                    Some(n) => {
                        let conversations_count = LittleEndian::read_u32(&tag_data[0x468..]) as usize;
                        for i in 0..conversations_count {
                            maybe_add_pointer(n + i * 116 + 0x50 + 4);
                            maybe_add_pointer(n + i * 116 + 0x5C + 4);
                        }
                        maybe_add_pointer(0x468 + 4);
                    },
                    None => ()
                };
                maybe_add_pointer(0x480);
                maybe_add_pointer(0x494);
                maybe_add_pointer(0x49C + 4);
                maybe_add_pointer(0x4A8 + 4);
                maybe_add_pointer(0x4B4 + 4);
                maybe_add_pointer(0x4E4 + 4);
                maybe_add_pointer(0x4F0 + 4);
                maybe_add_pointer(0x4FC + 4);
                maybe_add_pointer(0x5A4 + 4);
            },
            _ => {
                let mut i = 0;
                if tag_data.len() >= 12 {
                    while i < tag_data.len()-12+2 {
                        let count = LittleEndian::read_u32(&tag_data[i..]);
                        let address = LittleEndian::read_u32(&tag_data[i + 4..]);
                        let zero = LittleEndian::read_u32(&tag_data[i + 8..]);
                        if count > 0 && zero == 0 && address >= memory_address as u32 && address < memory_address_end {
                            pointers.push(i + 4);
                            i += 0xC;
                        }
                        else {
                            i += 2;
                        }
                    }
                }
            }
        }


        pointers
    }
}
