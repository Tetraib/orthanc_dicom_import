use rayon::prelude::*;
use reqwest::Client;
use std::env;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use walkdir::{DirEntry, WalkDir};
use indicatif::{ProgressBar, ProgressStyle};

fn is_dicom(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.ends_with(".dcm"))
        .unwrap_or(false)
}

async fn upload_to_orthanc(file: &PathBuf, orthanc_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = Vec::new();
    File::open(file)
        .await?
        .read_to_end(&mut buffer)
        .await?;

    let client = Client::new();
    let res = client.post(orthanc_url).body(buffer).send().await;

    match res {
        Ok(_) => Ok(()),
        Err(e) => {
            let mut file = OpenOptions::new().append(true).open("errors.txt").await?;
            file.write_all(format!("Failed to upload: {:?}, due to error: {}\n", file, e).as_bytes()).await?;
            Err(e.into())
        }
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

    let pb = ProgressBar::new(dicom_files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
            )
            .unwrap()
            .progress_chars("#>-"),
    );

    dicom_files.par_iter().for_each(|file| {
        let file_path = file.path().to_path_buf();
        let orthanc_url = orthanc_url.clone();

        // We need to use tokio-rayon's spawn_async because we're in a rayon parallel context
        // and we want to do async IO operations.
        let _ = tokio_rayon::spawn_async(move || {
            tokio::runtime::Handle::current().block_on(upload_to_orthanc(&file_path, &orthanc_url))
        }).await.unwrap_or_else(|e| {
            // Handle the error from spawn_async
            println!("Failed to upload: {:?}, due to spawn_async error: {:?}", file_path, e);
        });

        pb.inc(1);
    });

    pb.finish_with_message("All files processed.");
}
