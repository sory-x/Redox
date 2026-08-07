use std::{
    cell::RefCell,
    fs::File,
    io::{PipeWriter, Read, Write},
    path::{Path, PathBuf},
    process::Command,
    rc::Rc,
    time::Duration,
};

use crate::cook::fs;
use pkg::{
    callback::{Callback, PlainCallback, SilentCallback},
    net_backend::{CurlBackend, DownloadBackend, DownloadBackendWriter},
    PackageName, RemotePackage, RepoManager, Repository,
};

#[derive(Clone, Debug, serde::Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub blake3: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct ReleasePackage {
    pub pkgar: ReleaseAsset,
    pub metadata: ReleaseAsset,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct ReleaseSignature {
    url: String,
    public_key_url: String,
    runtime_public_key_url: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct ReleaseDocument {
    schema: u32,
    repository: String,
    target: String,
    release: ReleaseInfo,
    packages: std::collections::BTreeMap<String, ReleasePackage>,
    pkgar_public_key: Option<ReleaseAsset>,
    signature: ReleaseSignature,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct ReleaseInfo {
    tag: String,
    immutable: bool,
}

// TODO: This is a workaround, but as long as whole
// fetch operation is in single thread, this is ok
thread_local! {
static BINARY_REPO: RefCell<Option<(RepoManager, Repository)>> = RefCell::new(None);
static RELEASE_INDEX: RefCell<Option<Option<ReleaseDocument>>> = RefCell::new(None);
}

fn release_index_url() -> Option<String> {
    std::env::var("SORYOS_RELEASE_INDEX_URL")
        .ok()
        .filter(|url| !url.trim().is_empty())
}

pub fn release_index_configured() -> bool {
    release_index_url().is_some()
}

fn download_bytes(url: &str) -> crate::Result<Vec<u8>> {
    let callback = Rc::new(RefCell::new(SilentCallback::new()));
    let backend = CurlBackend::new().map_err(|error| {
        crate::Error::Other(format!("creating Release download backend: {error}"))
    })?;
    let mut writer = DownloadBackendWriter::ToBuf(Vec::new());
    backend
        .download(url, None, &mut writer, callback)
        .map_err(|error| {
            crate::Error::Other(format!("downloading Release asset {url}: {error}"))
        })?;
    Ok(writer.to_inner_buf())
}

fn write_bytes(path: &Path, bytes: &[u8]) -> crate::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            crate::Error::from_io_error(error, "creating Release index cache directory")
        })?;
    }
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, bytes)
        .map_err(|error| crate::Error::from_io_error(error, "writing Release index cache"))?;
    std::fs::rename(&temporary, path)
        .map_err(|error| crate::Error::from_io_error(error, "installing Release index cache"))?;
    Ok(())
}

fn verify_release_signature(
    index_path: &Path,
    signature_path: &Path,
    public_key_path: &Path,
) -> crate::Result<()> {
    let status = Command::new("openssl")
        .args(["pkeyutl", "-verify", "-rawin", "-pubin", "-inkey"])
        .arg(public_key_path)
        .args(["-in"])
        .arg(index_path)
        .args(["-sigfile"])
        .arg(signature_path)
        .status()
        .map_err(|error| {
            crate::Error::from_io_error(error, "starting OpenSSL Release signature verification")
        })?;
    if !status.success() {
        return Err(crate::Error::Other(
            "Release index signature verification failed".to_string(),
        ));
    }
    Ok(())
}

fn load_release_index() -> crate::Result<Option<ReleaseDocument>> {
    RELEASE_INDEX.with(|cell| {
        let mut cached = cell.borrow_mut();
        if let Some(index) = cached.as_ref() {
            return Ok(index.clone());
        }

        let Some(index_url) = release_index_url() else {
            *cached = Some(None);
            return Ok(None);
        };

        let cache_dir = PathBuf::from("build/remotes/soryos-release");
        let index_path = cache_dir.join("index.json");
        let index_bytes = if crate::config::get_config().cook.offline {
            std::fs::read(&index_path).map_err(|error| {
                crate::Error::from_io_error(error, "reading cached SoryOS Release index")
            })?
        } else {
            let bytes = download_bytes(&index_url)?;
            write_bytes(&index_path, &bytes)?;
            bytes
        };

        let document: ReleaseDocument = serde_json::from_slice(&index_bytes).map_err(|error| {
            crate::Error::Other(format!("invalid SoryOS Release index: {error}"))
        })?;
        if document.schema != 1 || !document.release.immutable {
            return Err(crate::Error::Other(
                "SoryOS Release index is not an immutable schema 1 document".to_string(),
            ));
        }
        let expected_repository = std::env::var("SORYOS_RELEASE_REPOSITORY")
            .unwrap_or_else(|_| "sory-x/soryos-apt".to_string());
        if document.repository != expected_repository {
            return Err(crate::Error::Other(format!(
                "SoryOS Release index repository mismatch: expected {expected_repository}, got {}",
                document.repository
            )));
        }
        let expected_target =
            std::env::var("TARGET").unwrap_or_else(|_| "x86_64-unknown-redox".to_string());
        if document.target != expected_target {
            return Err(crate::Error::Other(format!(
                "SoryOS Release index target mismatch: expected {expected_target}, got {}",
                document.target
            )));
        }
        let Some(release_base) = index_url.strip_suffix("/index.json") else {
            return Err(crate::Error::Other(
                "SoryOS Release index URL must end with /index.json".to_string(),
            ));
        };
        let expected_base = format!(
            "https://github.com/{}/releases/download/{}",
            document.repository, document.release.tag
        );
        if release_base != expected_base
            || document.signature.url != format!("{release_base}/index.json.sig")
            || document.signature.public_key_url
                != format!("{release_base}/index-signing-key.pub.pem")
            || document.signature.runtime_public_key_url
                != format!("{release_base}/index-signing-key.pub.hex")
        {
            return Err(crate::Error::Other(
                "SoryOS Release index URLs do not match its immutable repository and tag"
                    .to_string(),
            ));
        }

        let (signature_path, public_key_path) = (
            cache_dir.join("index.json.sig"),
            cache_dir.join("index-signing-key.pub.pem"),
        );
        if crate::config::get_config().cook.offline {
            if !signature_path.is_file() || !public_key_path.is_file() {
                return Err(crate::Error::Other(
                    "offline SoryOS Release index is missing its signature or public key"
                        .to_string(),
                ));
            }
        } else {
            write_bytes(&signature_path, &download_bytes(&document.signature.url)?)?;
            write_bytes(
                &public_key_path,
                &download_bytes(&document.signature.public_key_url)?,
            )?;
        }
        verify_release_signature(&index_path, &signature_path, &public_key_path)?;

        let result = Some(document);
        *cached = Some(result.clone());
        Ok(result)
    })
}

pub fn get_release_package(name: &str) -> crate::Result<Option<ReleasePackage>> {
    Ok(load_release_index()?.and_then(|index| index.packages.get(name).cloned()))
}

pub fn get_release_pubkey() -> crate::Result<Option<PathBuf>> {
    let Some(index) = load_release_index()? else {
        return Ok(None);
    };
    let Some(asset) = index.pkgar_public_key else {
        return Err(crate::Error::Other(
            "SoryOS Release index has no PKGAR public key".to_string(),
        ));
    };
    let path = PathBuf::from("build/remotes/soryos-release").join(&asset.name);
    let _ = download_release_asset(&asset, &path)?;
    Ok(Some(path))
}

pub fn download_release_asset(asset: &ReleaseAsset, destination: &Path) -> crate::Result<bool> {
    if destination.is_file() {
        if verify_release_asset(asset, destination).is_ok() {
            return Ok(false);
        }
        std::fs::remove_file(destination).map_err(|error| {
            crate::Error::from_io_error(error, "removing invalid Release asset")
        })?;
    }

    let temporary = destination.with_added_extension("release-tmp");
    if temporary.exists() {
        std::fs::remove_file(&temporary).map_err(|error| {
            crate::Error::from_io_error(error, "removing stale Release download")
        })?;
    }
    let file = File::create(&temporary)
        .map_err(|error| crate::Error::from_io_error(error, "creating Release asset"))?;
    let callback = Rc::new(RefCell::new(SilentCallback::new()));
    let backend = CurlBackend::new().map_err(|error| {
        crate::Error::Other(format!("creating Release download backend: {error}"))
    })?;
    let mut writer = DownloadBackendWriter::ToFile(file);
    backend
        .download(&asset.url, Some(asset.size), &mut writer, callback)
        .map_err(|error| {
            crate::Error::Other(format!("downloading Release asset {}: {error}", asset.name))
        })?;
    drop(writer);

    let actual_size = std::fs::metadata(&temporary)
        .map_err(|error| crate::Error::from_io_error(error, "checking Release asset size"))?
        .len();
    let actual_blake3 = file_blake3(&temporary)?;
    if actual_size != asset.size || actual_blake3 != asset.blake3 {
        let _ = std::fs::remove_file(&temporary);
        return Err(crate::Error::Other(format!(
            "Release asset {} failed verification (size {actual_size}, BLAKE3 {actual_blake3})",
            asset.name
        )));
    }
    std::fs::rename(&temporary, destination)
        .map_err(|error| crate::Error::from_io_error(error, "installing verified Release asset"))?;
    Ok(true)
}

pub fn verify_release_asset(asset: &ReleaseAsset, path: &Path) -> crate::Result<()> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| crate::Error::from_io_error(error, "reading cached Release asset"))?;
    let actual_blake3 = file_blake3(path)?;
    if metadata.len() != asset.size || actual_blake3 != asset.blake3 {
        return Err(crate::Error::Other(format!(
            "cached Release asset {} failed verification (size {}, BLAKE3 {})",
            asset.name,
            metadata.len(),
            actual_blake3
        )));
    }
    Ok(())
}

fn file_blake3(path: &Path) -> crate::Result<String> {
    let mut file = File::open(path).map_err(|error| {
        crate::Error::from_io_error(error, "opening file for BLAKE3 verification")
    })?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            crate::Error::from_io_error(error, "reading file for BLAKE3 verification")
        })?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn load_cached_repo(path: &Path) -> Option<Repository> {
    let refresh = std::env::var("REPO_BINARY_REFRESH")
        .map(|value| matches!(value.as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);
    if refresh {
        return None;
    }

    let metadata = std::fs::metadata(path).ok()?;

    if !crate::config::get_config().cook.offline {
        let stale_time = std::time::SystemTime::now().checked_sub(Duration::from_secs(8 * 3600))?;
        if metadata.modified().ok()? < stale_time {
            // stale cache
            let _ = std::fs::remove_file(path);
            return None;
        }
    }

    let toml_str = std::fs::read_to_string(path).ok()?;
    Repository::from_toml(&toml_str).ok()
}

fn init_binary_repo() -> (RepoManager, Repository) {
    let callback = Rc::new(RefCell::new(SilentCallback::new()));
    let download_backend = CurlBackend::new().expect("Curl not found");
    let mut repo = RepoManager::new(callback, Box::new(download_backend));
    let target = redoxer::target();
    let remote_source = crate::config::translate_mirror(crate::REMOTE_PKG_SOURCE);
    repo.add_remote(&remote_source, target)
        .expect("Unable to add remote");

    let repo_path = PathBuf::from("build/remotes");
    repo.set_download_path(repo_path.clone());
    repo.sync_keys().expect("Unable to sync keys");

    let repo_toml =
        load_cached_repo(&repo_path.join(format!("repo_{}_{target}.toml", repo.remotes[0])))
            .unwrap_or_else(|| {
                let repo = download_repo(&repo, repo_path)
                    .map_err(|e| {
                        eprintln!(
                        "Unable to load server repo.toml, all recipes will build from source: {e}"
                    );
                        e
                    })
                    .unwrap_or_default();
                repo
            });
    // reset here to not clobber pty
    repo.callback = Rc::new(RefCell::new(PlainCallback::new()));
    (repo, repo_toml)
}

fn download_repo(repo: &RepoManager, repo_path: PathBuf) -> crate::Result<Repository> {
    let (toml_str, _) = repo.get_package_toml(&PackageName::new("repo").unwrap())?;
    let repo = Repository::from_toml(&toml_str)?;
    let target = redoxer::target();
    fs::serialize_and_write(&repo_path.join(format!("{target}_repo.toml")), &repo)?;
    Ok(repo)
}

pub fn get_binary_repo() -> (RepoManager, Repository) {
    BINARY_REPO.with(|cell| {
        let mut opt = cell.borrow_mut();
        if opt.is_none() {
            *opt = Some(init_binary_repo());
        }
        let (repo, repo_toml) = opt.as_ref().unwrap();
        ((*repo).clone(), repo_toml.clone())
    })
}
pub fn get_binary_pubkey() -> PathBuf {
    BINARY_REPO.with(|cell| {
        let mut opt = cell.borrow_mut();
        if opt.is_none() {
            *opt = Some(init_binary_repo());
        }
        let (repo, _) = opt.as_ref().unwrap();
        let repo_path = PathBuf::from("build/remotes");
        repo_path.join(format!("pub_key_{}.toml", repo.remotes[0]))
    })
}

pub struct PlainPtyCallback {
    size: u64,
    unknown_size: bool,
    pos: u64,
    fetch_processed: usize,
    fetch_total: usize,
    interactive: bool,
    download_file: Option<String>,
    pty: PipeWriter,
}

impl PlainPtyCallback {
    pub fn new(pty: PipeWriter) -> Self {
        Self {
            size: 0,
            unknown_size: false,
            pos: 0,
            fetch_processed: 0,
            fetch_total: 0,
            interactive: false,
            download_file: None,
            pty,
        }
    }

    /// Set if user require to agree on terminal
    pub fn set_interactive(&mut self, enabled: bool) {
        self.interactive = enabled;
    }

    fn flush(&self) {
        let _ = std::io::stderr().flush();
    }

    pub fn format_size(bytes: u64) -> String {
        if bytes == 0 {
            return "0 B".to_string();
        }
        const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
        let i = (bytes as f64).log(1024.0).floor() as usize;
        let size = bytes as f64 / 1024.0_f64.powi(i as i32);
        format!("{:.2} {}", size, UNITS[i])
    }

    fn downloading_str(&self) -> &'static str {
        "Downloading"
    }
}

const RESET_LINE: &str = "\r\x1b[2K";

impl Callback for PlainPtyCallback {
    fn fetch_start(&mut self, initial_count: usize) {
        self.fetch_total = 0;
        self.fetch_processed = 0;
        self.fetch_package_increment(0, initial_count);
    }

    fn fetch_package_name(&mut self, pkg_name: &PackageName) {
        // resuming after fetch_package_increment
        let _ = write!(&self.pty, " {}", pkg_name.as_str());
        self.flush();
    }

    fn fetch_package_increment(&mut self, added_processed: usize, added_count: usize) {
        self.fetch_processed += added_processed;
        self.fetch_total += added_count;

        let _ = write!(
            &self.pty,
            "{RESET_LINE}Fetching: [{}/{}]",
            self.fetch_processed, self.fetch_total
        );
        self.flush();
    }

    fn fetch_end(&mut self) {
        if self.fetch_processed == self.fetch_total {
            let _ = writeln!(&self.pty, "{RESET_LINE}Fetch complete.");
        } else {
            let _ = writeln!(&self.pty, "{RESET_LINE}Fetch incomplete.");
        }
    }

    fn download_start(&mut self, length: u64, file: &str) {
        self.size = length;
        self.unknown_size = length == 0;
        self.pos = 0;
        if !self.unknown_size {
            let _ = write!(&self.pty, "{RESET_LINE}{} {file}", self.downloading_str());
            self.download_file = Some(file.to_string());
            self.flush();
        }
    }

    fn download_increment(&mut self, downloaded: u64) {
        self.pos += downloaded;
        if self.unknown_size {
            self.size += downloaded;
        }
        if self.unknown_size {
            return;
        }

        // keep using MB for consistency
        let pos_mb = self.pos as f64 / 1_048_576.0;
        let size_mb = self.size as f64 / 1_048_576.0;
        let file_name = self
            .download_file
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("");
        let _ = write!(
            &self.pty,
            "{RESET_LINE}{} {} [{:.2} MB / {:.2} MB]",
            self.downloading_str(),
            file_name,
            pos_mb,
            size_mb
        );
        self.flush();
    }

    fn download_end(&mut self) {
        if !self.unknown_size {
            let _ = writeln!(&self.pty, "");
            self.download_file = None;
        }
    }

    fn install_extract(&mut self, remote_pkg: &RemotePackage) {
        let _ = writeln!(&self.pty, "Extracting {}...", remote_pkg.package.name);
        self.flush();
    }
}
