//! Module for handling resource map files
extern crate byteorder;
use self::byteorder::{ByteOrder,LittleEndian};

use super::{encode_latin1_string, string_from_slice};

#[derive(PartialEq,Clone)]
/// There are a few different types of resource maps that can be used by Halo.
pub enum ResourceMapType {
    /// This is a bitmaps.map map. It stores bitmaps.
    Bitmap,
    /// This is a sounds.map map. It stores sounds.
    Sound,
    /// This is a loc.map map which is exclusive to Halo Custom Edition. It stores localization
    /// data tags such as unicode string tags.
    Loc,
    /// The type isn't known.
    Unknown(u32)
}
impl ResourceMapType {
    /// Create a ResourceMapType from a u32.
    pub fn from_u32(int32 : u32) -> ResourceMapType {
        match int32 {
            1 => ResourceMapType::Bitmap,
            2 => ResourceMapType::Sound,
            3 => ResourceMapType::Loc,
            n => ResourceMapType::Unknown(n)
        }
    }
    /// Convert a ResourceMapType to a u32.
    pub fn as_u32(&self) -> u32 {
        match *self {
            ResourceMapType::Bitmap => 1,
            ResourceMapType::Sound => 2,
            ResourceMapType::Loc => 3,
            ResourceMapType::Unknown(n) => n
        }
    }
}

#[derive(PartialEq,Clone)]
/// This defines a resource.
pub struct Resource {
    /// This is the name of the resource, which is typically a tag path.
    pub name : String,
    /// This is the data for the resource.
    pub data : Vec<u8>
}

#[derive(PartialEq,Clone)]
/// Resource maps are used by Halo for storing assets such as bitmaps and sounds. On Halo Custom
/// Edition, it also stores tag data.
pub struct ResourceMap {
    /// This defines the type of resource file.
    pub map_type : ResourceMapType,
    /// This is the array of resources.
    pub resources : Vec<Resource>
}
impl ResourceMap {
    /// This parses a resource map from a slice.
    pub fn from_resource_map(data : &[u8]) -> Result<ResourceMap,&'static str> {
        if data.len() < 0x10 {
            return Err("invalid resource map");
        }
        let names_offset = LittleEndian::read_u32(&data[0x4..]) as usize;
        if names_offset > data.len() {
            return Err("invalid names offset");
        }
        let resource_index_offset = LittleEndian::read_u32(&data[0x8..]) as usize;
        let resource_count = LittleEndian::read_u32(&data[0xC..]) as usize;
        if resource_count * 0xC + resource_index_offset > data.len() {
            return Err("invalid resource index offset/count");
        }

        let names = &data[names_offset ..];
        let resources_data = &data[resource_index_offset .. resource_index_offset + resource_count * 0xC];

        let mut resources = Vec::with_capacity(resource_count);

        for i in 0..resource_count {
            let resource = &resources_data[i * 0xC .. (i + 1) * 0xC];
            resources.push(Resource {
                name : {
                    let name_offset = LittleEndian::read_u32(&resource[0x0..]) as usize;
                    if name_offset > names.len() {
                        return Err("invalid resource name offset");
                    }
                    try!(string_from_slice(&names[name_offset..]))
                },
                data : {
                    let data_size = LittleEndian::read_u32(&resource[0x4..]) as usize;
                    let data_offset = LittleEndian::read_u32(&resource[0x8..]) as usize;
                    if data_size + data_offset > data.len() {
                        return Err("invalid resource data offset/size");
                    }
                    data[data_offset .. data_size + data_offset].to_owned()
                }
            });
        }

        Ok(ResourceMap {
            map_type : ResourceMapType::from_u32(LittleEndian::read_u32(&data[0x0..])),
            resources : resources
        })
    }
    /// This converts a resource map to a vector containing data that can be used by Halo.
    pub fn as_resource_map(&self) -> Vec<u8> {
        let mut header = [0u8 ; 0x10];
        let header_len = header.len();
        LittleEndian::write_u32(&mut header[0x0..], self.map_type.as_u32());

        let mut data = Vec::new();
        let mut names_data = Vec::new();
        let resources_len = self.resources.len();
        let mut resources = Vec::with_capacity(0xC * resources_len);
        for i in 0..resources_len {
            let mut resource = [0u8 ; 0xC];
            LittleEndian::write_u32(&mut resource[0x0..], names_data.len() as u32);
            names_data.append(&mut encode_latin1_string(&self.resources[i].name).unwrap());
            names_data.push(0);
            LittleEndian::write_u32(&mut resource[0x4..], self.resources[i].data.len() as u32);
            LittleEndian::write_u32(&mut resource[0x8..], header_len as u32 + data.len() as u32);
            data.extend_from_slice(&self.resources[i].data[..]);
            resources.extend_from_slice(&resource)
        }

        let mut v = Vec::new();
        LittleEndian::write_u32(&mut header[0x4..],(header_len + data.len()) as u32);
        LittleEndian::write_u32(&mut header[0x8..],(header_len + data.len() + names_data.len()) as u32);
        LittleEndian::write_u32(&mut header[0xC..], resources_len as u32);
        v.extend_from_slice(&header);
        v.append(&mut data);
        v.append(&mut names_data);
        v.append(&mut resources);

        v
    }
}
