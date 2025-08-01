// Copyright (c) 2022, Matteo Bernacchia <dev@kikijiki.com>. All rights reserved.
// This project is dual licensed under the Apache License 2.0 and the MIT license.
// See the LICENSE files in the project root for details.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use time::OffsetDateTime;

use crate::{
    api::{NtfsAttributeType, ROOT_RECORD, ntfs_to_unix_time},
    file::NtfsFile,
    mft::Mft,
};

pub trait FileInfoCache<'a> {
    fn get(&self, number: u64) -> Option<&Path>;
    fn insert(&mut self, number: u64, path: PathBuf);
}

#[derive(Default)]
pub struct HashMapCache(pub HashMap<u64, PathBuf>);
impl FileInfoCache<'_> for HashMapCache {
    fn get(&self, number: u64) -> Option<&Path> {
        if let Some(p) = self.0.get(&number) {
            Some(p)
        } else {
            None
        }
    }

    fn insert(&mut self, number: u64, path: PathBuf) {
        self.0.insert(number, path);
    }
}

#[derive(Default)]
pub struct VecCache(pub Vec<Option<PathBuf>>);
impl FileInfoCache<'_> for VecCache {
    fn get(&self, number: u64) -> Option<&Path> {
        if self.0.len() > number as usize {
            if let Some(p) = &self.0[number as usize] {
                return Some(p);
            }
        }
        None
    }

    fn insert(&mut self, number: u64, path: PathBuf) {
        if self.0.len() <= number as usize {
            self.0.resize(number as usize + 1, None);
        }
        self.0[number as usize] = Some(path);
    }
}

#[derive(Debug)]
pub struct FileInfo {
    pub file_id: u64, // MFT记录号，表示文件的唯一标识符
    pub name: String,
    pub path: PathBuf,
    pub is_directory: bool,
    pub size: u64,
    pub created: Option<OffsetDateTime>,
    pub accessed: Option<OffsetDateTime>,
    pub modified: Option<OffsetDateTime>,
}

impl FileInfo {
    pub fn new(mft: &Mft, file: &NtfsFile) -> Self {
        let mut info = Self::_new(file);
        info._compute_path(mft, file);
        info
    }

    pub fn with_cache<C: for<'a> FileInfoCache<'a>>(
        mft: &Mft,
        file: &NtfsFile,
        cache: &mut C,
    ) -> Self {
        let mut info = Self::_new(file);
        info._compute_path_with_cache(mft, file, cache);
        info
    }

    fn _new(file: &NtfsFile) -> Self {
        let mut accessed = None;
        let mut created = None;
        let mut modified = None;
        let mut size = 0u64;

        file.attributes(|att| {
            if att.header.type_id == NtfsAttributeType::StandardInformation as u32 {
                let stdinfo = att.as_standard_info();

                accessed = Some(ntfs_to_unix_time(stdinfo.access_time));
                created = Some(ntfs_to_unix_time(stdinfo.creation_time));
                modified = Some(ntfs_to_unix_time(stdinfo.modification_time));
            }

            if att.header.type_id == NtfsAttributeType::Data as u32 {
                if att.header.is_non_resident == 0 {
                    size = att.header_res.value_length as u64;
                } else {
                    size = att.header_nonres.data_size;
                }
            }
        });

        // 构造64位文件ID：低48位是MFT记录号，高16位是序列号
        let file_id = Self::create_file_id(file.number(), file.header.sequence_value);

        FileInfo {
            file_id,
            name: String::new(),
            path: PathBuf::new(),
            is_directory: file.is_directory(),
            size,
            created,
            accessed,
            modified,
        }
    }

    /// 创建64位文件ID：将MFT记录号(48位)和序列号(16位)组合
    /// 这与USN日志中的文件ID格式一致
    fn create_file_id(mft_record_number: u64, sequence_number: u16) -> u64 {
        // 确保MFT记录号只使用低48位
        let mft_number = mft_record_number & 0x0000_FFFF_FFFF_FFFF;
        // 将序列号放在高16位
        let sequence = (sequence_number as u64) << 48;
        // 组合成完整的64位文件ID
        mft_number | sequence
    }

    fn _compute_path(&mut self, mft: &Mft, file: &NtfsFile) {
        let mut next_parent;

        if let Some(name) = file.get_best_file_name(mft) {
            self.name = name.to_string();
            next_parent = name.parent();
        } else {
            //warn!("No name for file {}", file.number);
            return;
        }

        let mut components = Vec::new();
        loop {
            if next_parent == ROOT_RECORD {
                break;
            }

            let cur_file = mft.get_record(next_parent);
            if cur_file.is_none() {
                return;
            }
            let cur_file = cur_file.unwrap();

            if let Some(cur_name_att) = cur_file.get_best_file_name(mft) {
                let cur_name = cur_name_att.to_string();
                components.push((cur_file.number(), PathBuf::from(cur_name)));
                next_parent = cur_name_att.parent();
            } else {
                return;
            }
        }

        let mut path = mft.volume.path.clone();
        for (_, comp) in components.iter().rev() {
            path.push(comp);
        }
        path.push(&self.name);

        self.path = path;
    }

    fn _compute_path_with_cache<C: for<'a> FileInfoCache<'a>>(
        &mut self,
        mft: &Mft,
        file: &NtfsFile,
        cache: &mut C,
    ) {
        let mut next_parent;

        if let Some(name) = file.get_best_file_name(mft) {
            self.name = name.to_string();
            next_parent = name.parent();
        } else {
            return;
        }

        let mut components = Vec::new();
        let mut cached_path = None;
        loop {
            if next_parent == ROOT_RECORD {
                break;
            }

            // Cache hit?
            if let Some(cur_path) = cache.get(next_parent) {
                cached_path = Some(cur_path);
                break;
            }

            let cur_file = mft.get_record(next_parent);
            if cur_file.is_none() {
                return;
            }
            let cur_file = cur_file.unwrap();

            if let Some(cur_name_att) = cur_file.get_best_file_name(mft) {
                let cur_name = cur_name_att.to_string();
                components.push((cur_file.number(), PathBuf::from(cur_name)));
                next_parent = cur_name_att.parent();
            } else {
                return;
            }
        }

        let mut path = PathBuf::from(cached_path.unwrap_or(&mft.volume.path));

        for (number, comp) in components.iter().rev() {
            path.push(comp);
            cache.insert(*number, path.clone());
        }

        path.push(&self.name);
        cache.insert(file.number, path.clone());

        self.path = path;
    }
}
