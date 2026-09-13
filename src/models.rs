//! The same local Whisper model downloader is used on every desktop platform.
use anyhow::{Result, bail, ensure};
use std::{
    io::{Read, Write},
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
const MODEL_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin";
async fn cancelled(cancel: &AtomicBool) {
    while !cancel.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
pub fn download(cancel: &AtomicBool, progress: impl Fn(f32)) -> Result<PathBuf> {
    let dir = crate::preferences::Preferences::directory()?.join("whisper");
    std::fs::create_dir_all(&dir)?;
    let destination = dir.join("ggml-small.bin");
    let mut temp = tempfile::NamedTempFile::new_in(&dir)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async{
        let client=reqwest::Client::builder().https_only(true).connect_timeout(Duration::from_secs(20)).timeout(Duration::from_secs(1800)).build()?;
        let mut response=tokio::select!{r=client.get(MODEL_URL).send()=>r?.error_for_status()?,_=cancelled(cancel)=>bail!("Model download cancelled")};
        let total=response.content_length();let mut written=0u64;let mut last_percent=-1;
        loop{
            let chunk=tokio::select!{chunk=response.chunk()=>chunk?,_=cancelled(cancel)=>bail!("Model download cancelled")};
            let Some(chunk)=chunk else{break};written+=chunk.len() as u64;
            ensure!(written<=1024*1024*1024,"Model exceeds the 1 GiB limit");temp.write_all(&chunk)?;
            if let Some(total)=total{let fraction=written as f32/total.max(1) as f32;let percent=(fraction*100.) as i32;if percent!=last_percent{progress(fraction);last_percent=percent;}}
        }
        ensure!(written>10*1024*1024,"Downloaded model is too small");
        if let Some(total)=total{ensure!(written==total,"Model download is incomplete");}
        Ok::<(),anyhow::Error>(())
    })?;
    let mut magic = [0u8; 4];
    temp.reopen()?.read_exact(&mut magic)?;
    ensure!(
        magic == [0x6c, 0x6d, 0x67, 0x67],
        "Downloaded file is not a Whisper GGML model"
    );
    ensure!(!cancel.load(Ordering::Relaxed), "Model download cancelled");
    temp.as_file().sync_all()?;
    temp.persist(&destination).map_err(|e| e.error)?;
    progress(1.);
    Ok(destination)
}
