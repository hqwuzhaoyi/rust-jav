use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    fs,
    io::{Read, Write},
    os::fd::{AsRawFd, FromRawFd},
    os::unix::ffi::OsStrExt,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak},
};

use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};

use crate::{
    artwork_image::{self, ValidatedImage},
    image_blocking::{self, ImageBlockingBudget},
    jellyfin::{JellyfinClient, JellyfinImageRef},
};

const CACHE_DIRECTORY: &str = "jellyfin-images";
const MAX_MEMORY_BYTES: usize = 8 * 1024 * 1024;
const MAX_DISK_BYTES: u64 = 64 * 1024 * 1024;
const MAX_CONCURRENT_FETCHES: usize = 4;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("Jellyfin image request failed: {0}")]
    Upstream(#[from] crate::jellyfin::Error),
    #[error("Jellyfin returned unusable artwork: {0}")]
    Invalid(#[from] artwork_image::ValidationError),
    #[error("Jellyfin image cache storage failed: {0}")]
    Storage(#[from] std::io::Error),
    #[error("Jellyfin image cache worker failed")]
    Worker,
    #[error("Jellyfin image cache is busy")]
    Busy,
    #[error("Jellyfin image cache lock was poisoned")]
    Poisoned,
}

#[derive(Clone)]
pub(crate) struct JellyfinImageCache(Arc<Inner>);

struct Inner {
    root: Option<PathBuf>,
    memory: Mutex<MemoryCache>,
    key_locks: Mutex<HashMap<String, Weak<tokio::sync::Mutex<()>>>>,
    fetch_limit: tokio::sync::Semaphore,
    blocking: ImageBlockingBudget,
}

struct MemoryEntry {
    image: Arc<ValidatedImage>,
    last_used: u64,
}

struct MemoryCache {
    entries: HashMap<String, MemoryEntry>,
    total_bytes: usize,
    clock: u64,
}

impl MemoryCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            total_bytes: 0,
            clock: 0,
        }
    }

    fn get(&mut self, key: &str) -> Option<Arc<ValidatedImage>> {
        self.clock = self.clock.wrapping_add(1);
        let entry = self.entries.get_mut(key)?;
        entry.last_used = self.clock;
        Some(entry.image.clone())
    }

    fn insert(&mut self, key: String, image: Arc<ValidatedImage>) {
        let size = image.bytes.len();
        if let Some(previous) = self.entries.remove(&key) {
            self.total_bytes = self.total_bytes.saturating_sub(previous.image.bytes.len());
        }
        if size > MAX_MEMORY_BYTES {
            return;
        }
        while self.total_bytes.saturating_add(size) > MAX_MEMORY_BYTES {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            if let Some(evicted) = self.entries.remove(&oldest) {
                self.total_bytes = self.total_bytes.saturating_sub(evicted.image.bytes.len());
            }
        }
        self.clock = self.clock.wrapping_add(1);
        self.total_bytes = self.total_bytes.saturating_add(size);
        self.entries.insert(
            key,
            MemoryEntry {
                image,
                last_used: self.clock,
            },
        );
    }
}

impl JellyfinImageCache {
    pub(crate) fn new(root: Option<PathBuf>, blocking: ImageBlockingBudget) -> Self {
        Self(Arc::new(Inner {
            root,
            memory: Mutex::new(MemoryCache::new()),
            key_locks: Mutex::new(HashMap::new()),
            fetch_limit: tokio::sync::Semaphore::new(MAX_CONCURRENT_FETCHES),
            blocking,
        }))
    }

    pub(crate) async fn get(
        &self,
        client: &JellyfinClient,
        image: &JellyfinImageRef,
        max_width: u32,
    ) -> Result<Arc<ValidatedImage>, Error> {
        let key = cache_key(client, image, max_width);
        if let Some(image) = self.memory_get(&key)? {
            return Ok(image);
        }

        let key_lock = self.key_lock(&key)?;
        let _key_guard = key_lock.lock().await;
        if let Some(image) = self.memory_get(&key)? {
            return Ok(image);
        }

        if let Some(root) = self.0.root.clone() {
            let disk_key = key.clone();
            let cached = self
                .0
                .blocking
                .run(artwork_image::MAX_ARTWORK_BYTES as usize, move || {
                    read_cached(&root, &disk_key)
                })
                .await
                .map_err(blocking_error)??;
            if let Some(image) = cached {
                let image = Arc::new(image);
                self.memory_insert(key, image.clone())?;
                return Ok(image);
            }
        }

        let _fetch_permit = self
            .0
            .fetch_limit
            .acquire()
            .await
            .map_err(|_| Error::Worker)?;
        let downloaded = client.primary_image_ref(image, max_width).await?;
        let downloaded_bytes = downloaded.bytes.len();
        let validated = Arc::new(
            self.0
                .blocking
                .run(downloaded_bytes, move || {
                    artwork_image::validate(downloaded.bytes)
                })
                .await
                .map_err(blocking_error)??,
        );

        if let Some(root) = self.0.root.clone() {
            let disk_key = key.clone();
            let disk_image = validated.clone();
            self.0
                .blocking
                .run(disk_image.bytes.len(), move || {
                    write_cached(&root, &disk_key, &disk_image.bytes)
                })
                .await
                .map_err(blocking_error)??;
        }

        self.memory_insert(key, validated.clone())?;
        Ok(validated)
    }

    fn memory_get(&self, key: &str) -> Result<Option<Arc<ValidatedImage>>, Error> {
        Ok(self.0.memory.lock().map_err(|_| Error::Poisoned)?.get(key))
    }

    fn memory_insert(&self, key: String, image: Arc<ValidatedImage>) -> Result<(), Error> {
        self.0
            .memory
            .lock()
            .map_err(|_| Error::Poisoned)?
            .insert(key, image);
        Ok(())
    }

    fn key_lock(&self, key: &str) -> Result<Arc<tokio::sync::Mutex<()>>, Error> {
        let mut locks = self.0.key_locks.lock().map_err(|_| Error::Poisoned)?;
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(key).and_then(Weak::upgrade) {
            return Ok(lock);
        }
        let lock = Arc::new(tokio::sync::Mutex::new(()));
        locks.insert(key.to_owned(), Arc::downgrade(&lock));
        Ok(lock)
    }
}

fn blocking_error(error: image_blocking::Error) -> Error {
    match error {
        image_blocking::Error::Busy => Error::Busy,
        image_blocking::Error::Worker => Error::Worker,
    }
}

fn cache_key(client: &JellyfinClient, image: &JellyfinImageRef, max_width: u32) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"rust-jav-jellyfin-image-cache-v1\0");
    hasher.update(client.cache_fingerprint());
    hash_field(&mut hasher, image.item_id.as_bytes());
    hash_field(&mut hasher, image.image_tag.as_bytes());
    hasher.update(max_width.to_be_bytes());
    format!("{:x}", hasher.finalize())
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn read_cached(root: &Path, key: &str) -> Result<Option<ValidatedImage>, std::io::Error> {
    let Some(directory) = cache_directory(root, false)? else {
        return Ok(None);
    };
    let name = cache_name(key)?;
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
        )
    };
    if fd < 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::NotFound {
            return Ok(None);
        }
        return Err(error);
    }
    let mut file = unsafe { fs::File::from_raw_fd(fd) };
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file()
        || metadata.len() == 0
        || metadata.len() > artwork_image::MAX_ARTWORK_BYTES
    {
        return Ok(None);
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    Read::by_ref(&mut file)
        .take(artwork_image::MAX_ARTWORK_BYTES + 1)
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    if bytes.len() as u64 != metadata.len() || !same_file_identity(&metadata, &after) {
        return Ok(None);
    }
    let validated = match artwork_image::validate(bytes) {
        Ok(validated) => validated,
        Err(_) => return Ok(None),
    };
    if unsafe { libc::futimens(file.as_raw_fd(), std::ptr::null()) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(Some(validated))
}

fn same_file_identity(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    before.file_type().is_file()
        && after.file_type().is_file()
        && before.dev() == after.dev()
        && before.ino() == after.ino()
        && before.len() == after.len()
        && before.mtime() == after.mtime()
        && before.mtime_nsec() == after.mtime_nsec()
        && before.ctime() == after.ctime()
        && before.ctime_nsec() == after.ctime_nsec()
}

fn write_cached(root: &Path, key: &str, bytes: &[u8]) -> Result<(), std::io::Error> {
    let directory = cache_directory(root, true)?.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "cache directory could not be created",
        )
    })?;
    let _directory_lock = DirectoryLock::acquire(&directory)?;
    let final_name = cache_name(key)?;
    enforce_disk_quota(&directory, &final_name, bytes.len() as u64)?;
    let mut random = [0u8; 16];
    OsRng.fill_bytes(&mut random);
    let temporary_name = CString::new(format!(".{key}.{:x}.tmp", u128::from_be_bytes(random)))
        .expect("cache key and random suffix contain no NUL");
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            temporary_name.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            0o600,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut file = unsafe { fs::File::from_raw_fd(fd) };
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        let renamed = unsafe {
            libc::renameat(
                directory.as_raw_fd(),
                temporary_name.as_ptr(),
                directory.as_raw_fd(),
                final_name.as_ptr(),
            )
        };
        if renamed < 0 {
            return Err(std::io::Error::last_os_error());
        }
        directory.sync_all()
    })();
    if result.is_err() {
        unsafe {
            libc::unlinkat(directory.as_raw_fd(), temporary_name.as_ptr(), 0);
        }
    }
    result
}

struct DirectoryLock<'a>(&'a fs::File);

impl<'a> DirectoryLock<'a> {
    fn acquire(directory: &'a fs::File) -> Result<Self, std::io::Error> {
        if unsafe { libc::flock(directory.as_raw_fd(), libc::LOCK_EX) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self(directory))
    }
}

impl Drop for DirectoryLock<'_> {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

struct DiskEntry {
    name: CString,
    size: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    evictable: bool,
    temporary: bool,
}

fn enforce_disk_quota(
    directory: &fs::File,
    final_name: &CStr,
    incoming_bytes: u64,
) -> Result<(), std::io::Error> {
    if incoming_bytes > MAX_DISK_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::FileTooLarge,
            "image is larger than the Jellyfin disk cache quota",
        ));
    }
    let mut entries = list_disk_entries(directory)?;
    let mut total = entries.iter().map(|entry| entry.size).sum::<u64>();
    for entry in entries.iter().filter(|entry| entry.temporary) {
        if unsafe { libc::unlinkat(directory.as_raw_fd(), entry.name.as_ptr(), 0) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        total = total.saturating_sub(entry.size);
    }
    entries.retain(|entry| !entry.temporary);
    entries.sort_by(|left, right| {
        (
            left.modified_seconds,
            left.modified_nanoseconds,
            left.name.as_bytes(),
        )
            .cmp(&(
                right.modified_seconds,
                right.modified_nanoseconds,
                right.name.as_bytes(),
            ))
    });
    for entry in entries {
        if total.saturating_add(incoming_bytes) <= MAX_DISK_BYTES {
            break;
        }
        if !entry.evictable || entry.name.as_c_str() == final_name {
            continue;
        }
        if unsafe { libc::unlinkat(directory.as_raw_fd(), entry.name.as_ptr(), 0) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        total = total.saturating_sub(entry.size);
    }
    if total.saturating_add(incoming_bytes) > MAX_DISK_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::StorageFull,
            "Jellyfin disk cache quota cannot be satisfied safely",
        ));
    }
    Ok(())
}

fn list_disk_entries(directory: &fs::File) -> Result<Vec<DiskEntry>, std::io::Error> {
    let duplicated = unsafe { libc::dup(directory.as_raw_fd()) };
    if duplicated < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let stream = unsafe { libc::fdopendir(duplicated) };
    if stream.is_null() {
        unsafe { libc::close(duplicated) };
        return Err(std::io::Error::last_os_error());
    }
    let mut entries = Vec::new();
    loop {
        set_errno(0);
        let raw = unsafe { libc::readdir(stream) };
        if raw.is_null() {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(0) {
                unsafe { libc::closedir(stream) };
                return Err(error);
            }
            break;
        }
        let name = unsafe { CStr::from_ptr((*raw).d_name.as_ptr()) };
        if matches!(name.to_bytes(), b"." | b"..") {
            continue;
        }
        let Ok(name) = CString::new(name.to_bytes()) else {
            continue;
        };
        let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
        if unsafe {
            libc::fstatat(
                directory.as_raw_fd(),
                name.as_ptr(),
                metadata.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } < 0
        {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::NotFound {
                continue;
            }
            unsafe { libc::closedir(stream) };
            return Err(error);
        }
        let metadata = unsafe { metadata.assume_init() };
        if metadata.st_mode & libc::S_IFMT != libc::S_IFREG {
            continue;
        }
        entries.push(DiskEntry {
            evictable: is_cache_entry_name(name.to_bytes()),
            temporary: is_cache_temporary_name(name.to_bytes()),
            name,
            size: metadata.st_size.max(0) as u64,
            modified_seconds: stat_modified_seconds(&metadata),
            modified_nanoseconds: stat_modified_nanoseconds(&metadata),
        });
    }
    if unsafe { libc::closedir(stream) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(entries)
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn set_errno(value: i32) {
    unsafe { *libc::__error() = value }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn set_errno(value: i32) {
    unsafe { *libc::__errno_location() = value }
}

fn stat_modified_seconds(metadata: &libc::stat) -> i64 {
    metadata.st_mtime
}

fn stat_modified_nanoseconds(metadata: &libc::stat) -> i64 {
    metadata.st_mtime_nsec
}

fn is_cache_entry_name(name: &[u8]) -> bool {
    let Some(digest) = name.strip_suffix(b".image") else {
        return false;
    };
    digest.len() == 64 && digest.iter().all(u8::is_ascii_hexdigit)
}

fn is_cache_temporary_name(name: &[u8]) -> bool {
    let Some(rest) = name.strip_prefix(b".") else {
        return false;
    };
    let Some(rest) = rest.strip_suffix(b".tmp") else {
        return false;
    };
    let Some(separator) = rest.iter().position(|byte| *byte == b'.') else {
        return false;
    };
    let (digest, random_with_separator) = rest.split_at(separator);
    let random = &random_with_separator[1..];
    digest.len() == 64
        && digest.iter().all(u8::is_ascii_hexdigit)
        && !random.is_empty()
        && random.len() <= 32
        && random.iter().all(u8::is_ascii_hexdigit)
}

fn cache_directory(root: &Path, create: bool) -> Result<Option<fs::File>, std::io::Error> {
    if create {
        fs::create_dir_all(root)?;
    }
    let root_name = CString::new(root.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "cache root contains NUL")
    })?;
    let root_fd = unsafe {
        libc::open(
            root_name.as_ptr(),
            libc::O_RDONLY
                | libc::O_DIRECTORY
                | libc::O_CLOEXEC
                | libc::O_NOFOLLOW
                | libc::O_NONBLOCK,
        )
    };
    if root_fd < 0 {
        let error = std::io::Error::last_os_error();
        if !create && error.kind() == std::io::ErrorKind::NotFound {
            return Ok(None);
        }
        return Err(error);
    }
    let root = unsafe { fs::File::from_raw_fd(root_fd) };
    let child = CString::new(CACHE_DIRECTORY).expect("static cache directory contains no NUL");
    if create {
        let result = unsafe { libc::mkdirat(root.as_raw_fd(), child.as_ptr(), 0o700) };
        if result < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(error);
            }
        }
    }
    let child_fd = unsafe {
        libc::openat(
            root.as_raw_fd(),
            child.as_ptr(),
            libc::O_RDONLY
                | libc::O_DIRECTORY
                | libc::O_CLOEXEC
                | libc::O_NOFOLLOW
                | libc::O_NONBLOCK,
        )
    };
    if child_fd < 0 {
        let error = std::io::Error::last_os_error();
        if !create && error.kind() == std::io::ErrorKind::NotFound {
            return Ok(None);
        }
        return Err(error);
    }
    Ok(Some(unsafe { fs::File::from_raw_fd(child_fd) }))
}

fn cache_name(key: &str) -> Result<CString, std::io::Error> {
    CString::new(format!("{key}.image")).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "cache key contains NUL")
    })
}
