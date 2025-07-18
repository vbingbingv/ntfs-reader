// Copyright (c) 2022, Matteo Bernacchia <dev@kikijiki.com>. All rights reserved.
// This project is dual licensed under the Apache License 2.0 and the MIT license.
// See the LICENSE files in the project root for details.

pub mod aligned_reader;
pub mod api;
pub mod attribute;
pub mod errors;
pub mod file;
pub mod file_info;
pub mod journal;
pub mod mft;
pub mod volume;

use std::path::{Path, PathBuf};
use usn_journal_rs::path::PathResolver;
use file_info::{FileInfo, VecCache};
use mft::Mft as NTFSMft;
use volume::Volume as NTFSVolume;

fn usn_test() -> Result<(), Box<dyn std::error::Error>> {
    use usn_journal_rs::{journal::UsnJournal, volume::Volume};

    let drive_letter = 'D';
    let volume = Volume::from_drive_letter(drive_letter)?;
    let journal = UsnJournal::new(&volume);
    let mut path_resolver = PathResolver::new_with_cache(&volume);
    // @TIP 这是个监听!!!
    for entry in journal.iter()? {
        let full_path = path_resolver.resolve_path(&entry);
        let full_path = full_path
            .unwrap_or(PathBuf::from(Path::new("")));
        if entry.file_name.to_string_lossy().starts_with("mmp-test") {
            println!("USN entry: {:?}, path: {}, file id: {}", entry, full_path.to_str().unwrap_or(""), entry.fid);
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start_time = std::time::Instant::now();
    let volume = NTFSVolume::new("\\\\.\\D:")?;
    let mft = NTFSMft::new(volume)?;
    //Test with Vec cache
    let mut vec_cache = VecCache::default();
    vec_cache.0.resize(mft.max_record as usize, None);
    mft.iterate_files(|file| {
        let file_info = FileInfo::new(&mft, file);
        if file_info.name.starts_with("mmp-test") {
            println!("{:?}", file_info);
        }
    });
    let end = start_time.elapsed().as_secs_f32();
    println!("mft test Elapsed time: {:.2} [s]", end);

    // 搜索mmp-test相关的USN事件
    usn_test()?;

    Ok(())
}
