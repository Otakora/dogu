use std::{collections::{BTreeMap, HashMap}, fs::File, io::Read, path::Path, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}, time::Instant};
use anyhow::{anyhow, Result};
use serde::Serialize;
use sha2::Digest;
use tauri::{ipc::Channel, State};
use crate::remote;

pub struct HashRegistry(pub Mutex<HashMap<String, Arc<AtomicBool>>>);
impl HashRegistry { pub fn new() -> Self { Self(Mutex::new(HashMap::new())) } }
#[derive(Clone, Serialize)]
#[serde(rename_all="camelCase")]
pub struct HashProgress { pub path: String, pub index: usize, pub count: usize, pub bytes: u64, pub total: u64 }
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct HashResult { pub path: String, pub hashes: BTreeMap<String, String>, pub error: Option<String> }

pub fn hash_reader<R: Read + ?Sized>(reader: &mut R, algorithms: &[String], cancel: &AtomicBool, mut progress: impl FnMut(u64)) -> Result<BTreeMap<String, String>> {
    let has = |s: &str| algorithms.iter().any(|a| a == s);
    let mut md5 = has("MD5").then(md5::Md5::new);
    let mut sha1 = has("SHA-1").then(sha1::Sha1::new);
    let mut sha256 = has("SHA-256").then(sha2::Sha256::new);
    let mut sha512 = has("SHA-512").then(sha2::Sha512::new);
    let mut crc = has("CRC32").then(crc32fast::Hasher::new);
    let mut blake = has("BLAKE3").then(blake3::Hasher::new);
    let mut buffer = vec![0; 1024 * 1024];
    let mut total = 0u64;
    let mut last = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) { return Err(anyhow!("[HASH_CANCELLED]")); }
        let n = reader.read(&mut buffer)?;
        if n == 0 { break; }
        let bytes = &buffer[..n];
        if let Some(v) = &mut md5 { v.update(bytes); }
        if let Some(v) = &mut sha1 { v.update(bytes); }
        if let Some(v) = &mut sha256 { v.update(bytes); }
        if let Some(v) = &mut sha512 { v.update(bytes); }
        if let Some(v) = &mut crc { v.update(bytes); }
        if let Some(v) = &mut blake { v.update(bytes); }
        total += n as u64;
        if last.elapsed().as_millis() >= 150 { progress(total); last = Instant::now(); }
    }
    if cancel.load(Ordering::Relaxed) { return Err(anyhow!("[HASH_CANCELLED]")); }
    progress(total);
    let mut hashes = BTreeMap::new();
    if let Some(v) = md5 { hashes.insert("MD5".into(), format!("{:x}", v.finalize())); }
    if let Some(v) = sha1 { hashes.insert("SHA-1".into(), format!("{:x}", v.finalize())); }
    if let Some(v) = sha256 { hashes.insert("SHA-256".into(), format!("{:x}", v.finalize())); }
    if let Some(v) = sha512 { hashes.insert("SHA-512".into(), format!("{:x}", v.finalize())); }
    if let Some(v) = crc { hashes.insert("CRC32".into(), format!("{:08x}", v.finalize())); }
    if let Some(v) = blake { hashes.insert("BLAKE3".into(), v.finalize().to_hex().to_string()); }
    Ok(hashes)
}

#[tauri::command]
pub fn cancel_hashes(registry: State<'_, HashRegistry>, request_id: String) -> Result<(), String> {
    if let Some(flag) = registry.0.lock().map_err(|e| e.to_string())?.get(&request_id) { flag.store(true, Ordering::Relaxed); }
    Ok(())
}

#[tauri::command]
pub async fn calculate_hashes(registry: State<'_, HashRegistry>, remote_state: State<'_, remote::RemoteState>, request_id: String, paths: Vec<String>, algorithms: Vec<String>, progress: Channel<HashProgress>) -> Result<Vec<HashResult>, String> {
    let allowed = ["MD5", "SHA-1", "SHA-256", "SHA-512", "CRC32", "BLAKE3"];
    if algorithms.is_empty() || algorithms.iter().any(|a| !allowed.contains(&a.as_str())) { return Err("Invalid hash algorithms".into()); }
    let flag = Arc::new(AtomicBool::new(false));
    { let mut requests = registry.0.lock().map_err(|e| e.to_string())?; if requests.contains_key(&request_id) { return Err("Duplicate hash request".into()); } requests.insert(request_id.clone(), flag.clone()); }
    let manager = remote_state.inner.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut results = Vec::new();
        for (index, path) in paths.iter().enumerate() {
            if flag.load(Ordering::Relaxed) { break; }
            let send = |bytes, total| { let _ = progress.send(HashProgress { path: path.clone(), index, count: paths.len(), bytes, total }); };
            send(0, 0);
            let result = if path.starts_with("remote://") {
                manager.hash_remote_file(path, &algorithms, &flag, &send)
            } else {
                (|| -> Result<_> {
                    if !std::fs::metadata(path)?.is_file() { return Err(anyhow!("[HASH_NOT_FILE]")); }
                    let mut file = File::open(Path::new(path))?;
                    let before = file.metadata()?;
                    if !before.is_file() { return Err(anyhow!("[HASH_NOT_FILE]")); }
                    let hashes = hash_reader(&mut file, &algorithms, &flag, |bytes| send(bytes, before.len()))?;
                    let after = file.metadata()?;
                    let current = std::fs::metadata(path)?;
                    if before.len() != after.len() || before.modified().ok() != after.modified().ok() || before.len() != current.len() || before.modified().ok() != current.modified().ok() { return Err(anyhow!("[HASH_FILE_CHANGED]")); }
                    Ok(hashes)
                })()
            };
            match result { Ok(hashes) => results.push(HashResult { path: path.clone(), hashes, error: None }), Err(e) => { if flag.load(Ordering::Relaxed) { break; } results.push(HashResult { path: path.clone(), hashes: BTreeMap::new(), error: Some(format!("{e:#}")) }); } }
        }
        (results, flag.load(Ordering::Relaxed))
    }).await.map_err(|e| e.to_string());
    if let Ok(mut requests) = registry.0.lock() { requests.remove(&request_id); }
    let (results, cancelled) = result?;
    if cancelled { Err("[HASH_CANCELLED]".into()) } else { Ok(results) }
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn standard_vectors_and_cancellation() {
        let alg = ["MD5","SHA-1","SHA-256","SHA-512","CRC32","BLAKE3"].map(String::from);
        let cancel = AtomicBool::new(false);
        let values = hash_reader(&mut &b"abc"[..], &alg, &cancel, |_| {}).unwrap();
        assert_eq!(values["MD5"], "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(values["SHA-1"], "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(values["SHA-256"], "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(values["SHA-512"], "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f");
        assert_eq!(values["CRC32"], "352441c2");
        assert_eq!(values["BLAKE3"], "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85");
        cancel.store(true, Ordering::Relaxed); assert!(hash_reader(&mut &b"abc"[..], &alg, &cancel, |_| {}).is_err());
    }
    #[test] fn hashes_empty_and_multi_block_streams() {
        let algorithms = ["SHA-256".to_string(), "CRC32".to_string()];
        let flag = AtomicBool::new(false);
        let empty = hash_reader(&mut &b""[..], &algorithms, &flag, |_| {}).unwrap();
        assert_eq!(empty["SHA-256"], "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(empty["CRC32"], "00000000");
        let bytes = vec![42u8; 2 * 1024 * 1024 + 1];
        let mut count = 0;
        let actual = hash_reader(&mut bytes.as_slice(), &algorithms, &flag, |n| count = n).unwrap();
        assert_eq!(count, bytes.len() as u64);
        assert_eq!(actual["SHA-256"], format!("{:x}", sha2::Sha256::digest(&bytes)));
        assert_eq!(actual["CRC32"], format!("{:08x}", crc32fast::hash(&bytes)));
    }
}
