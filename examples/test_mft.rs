use ntfs_reader::file_info::FileInfo;

use ntfs_reader::mft::Mft as NTFSMft;

use std::path::{Path, PathBuf};

use usn_journal_rs::path::PathResolver;

use usn_journal_rs::{journal::UsnJournal, volume::Volume};

use ntfs_reader::volume::Volume as NTFSVolume;

fn usn_test() -> Result<(), Box<dyn std::error::Error>> {
    let drive_letter = 'C';
    let volume = Volume::from_drive_letter(drive_letter)?;
    let journal = UsnJournal::new(&volume);
    let mut path_resolver = PathResolver::new_with_cache(&volume);
    // @TIP 这是个监听!!!
    for entry in journal.iter()? {
        let full_path = path_resolver.resolve_path(&entry);
        let full_path = full_path.unwrap_or(PathBuf::from(Path::new("")));
        if entry.file_name.to_string_lossy().starts_with("mmp-test") {
            println!(
                "USN entry: {:?}, path: {}, file id: {}",
                entry,
                full_path.to_str().unwrap_or(""),
                entry.fid
            );
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start_time = std::time::Instant::now();
    let volume = NTFSVolume::new("\\\\.\\C:")?;
    let mft = NTFSMft::new(volume)?;
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
