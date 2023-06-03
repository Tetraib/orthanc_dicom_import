use walkdir::{WalkDir, DirEntry};
use rayon::prelude::*;
use reqwest::Client;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use std::path::PathBuf;
use std::env;

fn is_dicom(entry: &DirEntry) -> bool {
    entry.file_name()
        .to_str()
        .map(|s| s.ends_with(".dcm"))
        .unwrap_or(false)
}

async fn upload_to_orthanc(file: &PathBuf, orthanc_url: &str) {
    let mut buffer = Vec::new();
    File::open(file).await.unwrap().read_to_end(&mut buffer).await.unwrap();

    let client = Client::new();
    let res = client.post(orthanc_url)
        .body(buffer)
        .send()
        .await;

    match res {
        Ok(_) => println!("Successfully uploaded: {:?}", file),
        Err(e) => println!("Failed to upload: {:?}, due to error: {}", file, e),
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("Please provide the directory path and Orthanc server URL as arguments");
        return;
    }

    let folder_path = &args[1];
    let orthanc_url = &args[2];
    let dicom_files: Vec<_> = WalkDir::new(folder_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| is_dicom(e))
        .collect();

    dicom_files.par_iter().for_each(|file| {
        // We need to use tokio's block_in_place because we're in a rayon parallel context
        // and we want to do async IO operations.
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(upload_to_orthanc(&file.path().to_path_buf(), orthanc_url));
        });
    });
}
