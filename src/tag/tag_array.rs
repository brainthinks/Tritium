use super::Tag;

#[derive(Clone)]
/// A tag array contains the tags that make up a Halo map.
pub struct TagArray {
    principal_tag : Option<usize>,
    tags : Vec<Tag>
}
impl TagArray {
    /// Creates a tag array from a vector of tags, consuming the vector.
    pub fn new(tags : Vec<Tag>, principal_tag : Option<usize>) -> TagArray {
        TagArray { tags : tags, principal_tag : principal_tag }
    }

    /// Get the principal tag of the tag array.
    ///
    /// This function returns `None` if there is no principal tag.
    pub fn principal_tag(&self) -> Option<usize> {
        self.principal_tag
    }

    /// Get an immutable reference to the tag array.
    pub fn tags(&self) -> &[Tag] {
        &self.tags
    }

    /// Get a mutable reference to the tag array.
    pub fn tags_mut(&mut self) -> &mut [Tag] {
        &mut self.tags
    }

    /// Search for the first tag index in this tag array with a path and a class.
    pub fn find_tag(&self, tag_path : &str, tag_class : u32) -> Option<usize> {
        let tag_array = self.tags();
        for i in 0..tag_array.len() {
            let tag = &tag_array[i];
            if tag.tag_path == tag_path && tag.tag_class.0 == tag_class {
                return Some(i);
            }
        }
        None
    }

    /// Search for every tag index in this tag array with a path and a class, optionally omitting either.
    ///
    /// Not specifying either a tag path or a tag class will return every tag index.
    pub fn find_tags(&self, tag_path : Option<&str>, tag_class : Option<u32>) -> Option<Vec<usize>> {
        let mut returned = Vec::new();
        let tag_array = self.tags();
        for i in 0..tag_array.len() {
            let tag = &tag_array[i];
            match tag_path {
                Some(n) => {
                    if tag.tag_path != n {
                        continue;
                    }
                },
                None => ()
            }
            match tag_class {
                Some(n) => {
                    if tag.tag_class.0 != n {
                        continue;
                    }
                },
                None => ()
            }
            returned.push(i);
        }
        if returned.len() == 0 {
            None
        }
        else {
            Some(returned)
        }
    }

    /// Insert a tag into this tag array.
    ///
    /// If this tag array contains the necessary tags from the origin tag array, the function will
    /// return `Ok` along with the index of the new tag. Otherwise, this function will return `Err`
    /// without any changes to the array.
    ///
    /// This function will panic if the tag array exceeds 65535 objects.
    pub fn insert(&mut self, origin_tag_array : &TagArray, origin_tag_index : usize) -> Result<usize,&'static str> {
        let mut tag = (&origin_tag_array.tags()[origin_tag_index]).to_owned();
        for i in self.tags() {
            if i.tag_class == tag.tag_class && tag.tag_path == i.tag_path {
                return Err("tag already exists")
            }
        }

        for i in &mut tag.references(origin_tag_array) {
            let origin_tag = &origin_tag_array.tags()[i.tag_index];
            match self.find_tag(&origin_tag.tag_path, origin_tag.tag_class.0) {
                Some(n) => {
                    i.tag_index = n;
                    tag.set_reference(&i);
                },
                None => return Err("tag array is missing a tag")
            }
        }

        let new_index = self.tags.len();
        if new_index > 65535 {
            panic!("tag array exceeds 65535 objects")
        }
        self.tags.push(tag);
        Ok(new_index)
    }

    /// Recursively insert a tag into this tag array, inserting any necessary tags along the way.
    ///
    /// The index of the new tag is returned.
    ///
    /// This function will panic if the tag array exceeds 65535 objects.
    pub fn insert_recursive(&mut self, origin_tag_array : &TagArray, origin_tag_index : usize) -> Result<usize,&'static str> {
        let tag = (&origin_tag_array.tags()[origin_tag_index]).to_owned();
        for i in self.tags() {
            if i.tag_class == tag.tag_class && tag.tag_path == i.tag_path {
                return Err("tag already exists")
            }
        }
        Ok(self.p_insert_recursive(origin_tag_array,origin_tag_index,&mut Vec::new()))
    }

    /// Remove a specific tag from the tag array and returns it.
    ///
    /// This function will panic if the tag does not already exist.
    pub fn remove(&mut self, tag : usize) -> Tag {
        let tag_count = self.tags.len();
        assert!(tag < tag_count,"tag out of bounds");
        for t in 0..tag_count {
            if tag == t {
                continue;
            }
            let references = self.tags[t].references(&self);
            for mut r in references {
                if r.tag_index > tag {
                    r.tag_index -= 1;
                }
                else if r.tag_index == tag {
                    r.tag_index = 0xFFFFFFFF;
                }
                self.tags[t].set_reference(&r);
            }
        }
        self.tags.remove(tag)
    }

    /// Remove all tags not referenced (recursively) by tagc tags, matg tags, and the principal scenario tag, as well as essential tags.
    pub fn remove_dead_tags(&mut self) {
        let mut keep_list = Vec::new();
        let tag_count = self.tags.len();
        keep_list.resize(tag_count, false);

        for i in 0..tag_count {
            {
                let tag = unsafe { self.tags.get_unchecked(i) };
                let class = tag.tag_class.0;
                let is_principal_tag = {
                    match self.principal_tag {
                        Some(n) => i == n,
                        None => false
                    }
                };
                if match class {
                    0x6269746D => {
                        match &tag.tag_path as &str {
                            "ui\\shell\\bitmaps\\background" => false,
                            "ui\\shell\\bitmaps\\trouble_brewing" => false,
                            _ => true
                        }
                    },
                    0x736E6421 => {
                        match &tag.tag_path as &str {
                            "sound\\sfx\\ui\\cursor" => false,
                            "sound\\sfx\\ui\\forward" => false,
                            "sound\\sfx\\ui\\back" => false,
                            _ => true
                        }
                    },
                    0x75737472 => {
                        match &tag.tag_path as &str {
                            "ui\\shell\\strings\\loading" => false,
                            "ui\\shell\\main_menu\\mp_map_list" => false,
                            _ => true
                        }
                    },
                    0x6D617467 => {
                        match &tag.tag_path as &str {
                            "globals\\globals" => false,
                            _ => true
                        }
                    },
                    0x74616763 => false,
                    _ => !is_principal_tag
                } {
                    if !is_principal_tag {
                        continue;
                    }
                }
            }

            self.p_save_tag_recursive(i, &mut keep_list);
        }

        for i in (0..tag_count).rev() {
            if keep_list[i] {
                continue;
            }
            println!("Removing tag {} - {}.{}",i,self.tags[i].tag_path,self.tags[i].tag_class.0);
            self.remove(i);
        }
    }

    fn p_save_tag_recursive(&mut self, tag_index : usize, keep_list : &mut [bool]) {
        if keep_list[tag_index] {
            return;
        }
        keep_list[tag_index] = true;

        let references = self.tags[tag_index].references(&self);
        for i in references {
            if tag_index == i.tag_index {
                continue;
            }
            self.p_save_tag_recursive(i.tag_index, keep_list);
        }
    }

    fn p_insert_recursive(&mut self, origin_tag_array : &TagArray, origin_tag_index : usize, tags_to_be_imported : &mut Vec<usize>) -> usize {
        let mut tag = (&origin_tag_array.tags()[origin_tag_index]).to_owned();
        if tags_to_be_imported.contains(&origin_tag_index) {
            // Cyclical tag reference.
            return origin_tag_index;
        }
        tags_to_be_imported.push(origin_tag_index);

        let mut referencing_self = Vec::new();
        for i in &mut tag.references(origin_tag_array) {
            let origin_tag = &origin_tag_array.tags()[i.tag_index];
            i.tag_index = if i.tag_index == origin_tag_index {
                referencing_self.push(i.to_owned());
                continue;
            }
            else {
                match self.find_tag(&origin_tag.tag_path, origin_tag.tag_class.0) {
                    Some(n) => n,
                    None => self.p_insert_recursive(origin_tag_array, i.tag_index, tags_to_be_imported)
                }
            };
            tag.set_reference(&i);
        }

        let new_index = self.tags.len();

        if new_index > 65535 {
            panic!("tag array exceeds 65535 objects")
        }

        // Handle tags that are referencing themselves.
        for mut i in referencing_self {
            i.tag_index = new_index;
            tag.set_reference(&i);
        }

        self.tags.push(tag);
        new_index
    }
}

/// Convert a tag index into a 32-bit tag ID for older map editors.
pub fn tag_index_to_tag_id(index : usize) -> u32 {
    if index == 0xFFFFFFFF {
        return 0xFFFFFFFF;
    }
    let tag_index = index & 0xFFFF;
    let secondary_index = (index + 0xE174) * 0x10000;
    (tag_index + secondary_index) as u32
}
