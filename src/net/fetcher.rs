use anyhow::{anyhow, Result};
use bytes::Bytes;
use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Hard upper bound for one downloaded resource (including uncached resources).
const MAX_RESOURCE_BYTES: usize = 32 * 1024 * 1024;
/// Total memory budget for cached assets.
const MAX_CACHE_BYTES: usize = 128 * 1024 * 1024;
/// Upper bound across page and asset network requests for one browser instance.
const MAX_CONCURRENT_REMOTE_REQUESTS: usize = 8;
 
fn read_file_limited(path: &Path, description: &str) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path)
        .map_err(|e| anyhow!("Failed to open {} {}: {}", description, path.display(), e))?;
    let mut limited = file.take(MAX_RESOURCE_BYTES as u64 + 1);
    let mut bytes = Vec::new();
    limited.read_to_end(&mut bytes)
        .map_err(|e| anyhow!("Failed to read {} {}: {}", description, path.display(), e))?;
    if bytes.len() > MAX_RESOURCE_BYTES {
        return Err(anyhow!("{} exceeds the {} MiB limit", description, MAX_RESOURCE_BYTES / (1024 * 1024)));
    }
    Ok(bytes)
}

fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let octets = ip.octets();
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_unspecified()
                || ip.is_multicast()
                || octets[0] == 0
                || (octets[0] == 100 && (64..=127).contains(&octets[1]))
                || (octets[0] == 198 && (18..=19).contains(&octets[1]))
                || (octets[0] == 192 && octets[1] == 0 && (octets[2] == 0 || octets[2] == 2))
                || (octets[0] == 192 && octets[1] == 88 && octets[2] == 99)
                || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
                || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
                || octets[0] >= 240
        }
        IpAddr::V6(ip) => {
            if let Some(mapped) = ip.to_ipv4() {
                return is_blocked_ip(IpAddr::V4(mapped));
            }
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || ip.is_multicast()
                || (ip.segments()[0] == 0x2001 && ip.segments()[1] == 0x0db8)
                || (ip.segments()[0] == 0x0100 && ip.segments()[1] == 0)
                || (ip.segments()[0] == 0x0064 && ip.segments()[1] == 0xff9b)
                || ip.segments()[0] == 0x2002
                || (ip.segments()[0] == 0x2001 && ip.segments()[1] == 0)
        }
    }
}

fn validate_network_target(url: &Url, addresses: &[SocketAddr]) -> Result<()> {
    let host = url.host_str().ok_or_else(|| anyhow!("URL has no host"))?;
    let host_lower = host.trim_end_matches('.').to_ascii_lowercase();
    if host_lower == "localhost"
        || host_lower.ends_with(".localhost")
        || host_lower.ends_with(".local")
        || host_lower.ends_with(".internal")
    {
        return Err(anyhow!("Local and internal hostnames are blocked: {}", host));
    }
    if addresses.is_empty() {
        return Err(anyhow!("No network addresses resolved for {}", host));
    }
    if let Some(address) = addresses.iter().find(|address| is_blocked_ip(address.ip())) {
        return Err(anyhow!("Private or non-public network destination is blocked: {}", address.ip()));
    }
    Ok(())
}

async fn build_pinned_async_client(url: &Url) -> Result<reqwest::Client> {
    let host = url.host_str().ok_or_else(|| anyhow!("URL has no host"))?;
    let port = url.port_or_known_default().ok_or_else(|| anyhow!("URL has no known port"))?;
    let literal_ip = match url.host() {
        Some(Host::Ipv4(ip)) => Some(IpAddr::V4(ip)),
        Some(Host::Ipv6(ip)) => Some(IpAddr::V6(ip)),
        _ => None,
    };
    let addresses: Vec<SocketAddr> = if let Some(ip) = literal_ip {
        vec![SocketAddr::new(ip, port)]
    } else {
        tokio::net::lookup_host((host, port))
            .await
            .map_err(|e| anyhow!("DNS lookup failed for {}: {}", host, e))?
            .collect()
    };
    validate_network_target(url, &addresses)?;
    let mut builder = reqwest::Client::builder()
        .user_agent("Lumra/0.2.0 (Typst-Browser; Native-GUI)")
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy();
    if literal_ip.is_none() {
        builder = builder.resolve_to_addrs(host, &addresses);
    }
    builder.build().map_err(|e| anyhow!("Failed to build safe HTTP client: {}", e))
}

fn build_pinned_blocking_client(url: &Url) -> Result<reqwest::blocking::Client> {
    let host = url.host_str().ok_or_else(|| anyhow!("URL has no host"))?;
    let port = url.port_or_known_default().ok_or_else(|| anyhow!("URL has no known port"))?;
    let literal_ip = match url.host() {
        Some(Host::Ipv4(ip)) => Some(IpAddr::V4(ip)),
        Some(Host::Ipv6(ip)) => Some(IpAddr::V6(ip)),
        _ => None,
    };
    let addresses: Vec<SocketAddr> = if let Some(ip) = literal_ip {
        vec![SocketAddr::new(ip, port)]
    } else {
        (host, port)
            .to_socket_addrs()
            .map_err(|e| anyhow!("DNS lookup failed for {}: {}", host, e))?
            .collect()
    };
    validate_network_target(url, &addresses)?;
    let mut builder = reqwest::blocking::Client::builder()
        .user_agent("Lumra/0.2.0 (Typst-Browser; Native-GUI)")
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy();
    if literal_ip.is_none() {
        builder = builder.resolve_to_addrs(host, &addresses);
    }
    builder.build().map_err(|e| anyhow!("Failed to build safe HTTP client: {}", e))
}
use url::{Host, Url};

struct AssetCache {
    entries: HashMap<String, Bytes>,
    insertion_order: VecDeque<String>,
    total_bytes: usize,
}

impl AssetCache {
    fn new() -> Self {
        Self { entries: HashMap::new(), insertion_order: VecDeque::new(), total_bytes: 0 }
    }

    fn get(&self, key: &str) -> Option<Bytes> {
        self.entries.get(key).cloned()
    }

    fn insert(&mut self, key: String, bytes: Bytes) {
        if bytes.len() > MAX_CACHE_BYTES { return; }
        if let Some(previous) = self.entries.remove(&key) {
            self.total_bytes = self.total_bytes.saturating_sub(previous.len());
            self.insertion_order.retain(|entry| entry != &key);
        }
        while self.total_bytes.saturating_add(bytes.len()) > MAX_CACHE_BYTES {
            let Some(oldest) = self.insertion_order.pop_front() else { break; };
            if let Some(previous) = self.entries.remove(&oldest) {
                self.total_bytes = self.total_bytes.saturating_sub(previous.len());
            }
        }
        self.total_bytes = self.total_bytes.saturating_add(bytes.len());
        self.insertion_order.push_back(key.clone());
        self.entries.insert(key, bytes);
    }
}

pub struct ContentFetcher {
    asset_cache: Mutex<AssetCache>,
    remote_requests: tokio::sync::Semaphore,
}

impl ContentFetcher {
    pub fn new() -> Self {
        Self {
            asset_cache: Mutex::new(AssetCache::new()),
            remote_requests: tokio::sync::Semaphore::new(MAX_CONCURRENT_REMOTE_REQUESTS),
        }
    }

    /// Resolve an asset path against an optional base URL/path
    pub fn resolve_url(&self, base_url: Option<&str>, path_str: &str) -> String {
        let trimmed = path_str.trim();

        // 1. Fix Unix VirtualPath single-slash artifact for protocols
        let normalized = Self::normalize_url(trimmed);

        // 2. Already an absolute URL
        if normalized.starts_with("http://")
            || normalized.starts_with("https://")
            || normalized.starts_with("file://")
            || normalized.starts_with("lumra://")
        {
            return normalized;
        }

        // 3. Resolve relative to base URL
        if let Some(base) = base_url {
            let base_norm = Self::normalize_url(base.trim());
            if let Ok(base_parsed) = Url::parse(&base_norm) {
                if let Ok(joined) = base_parsed.join(&normalized) {
                    return joined.to_string();
                }
            } else if base_norm.starts_with("lumra://") {
                return format!("{}/{}", base_norm.trim_end_matches('/'), normalized.trim_start_matches('/'));
            } else {
                // Base is a local file path
                let base_path = Path::new(&base_norm);
                let dir = if base_path.is_file() {
                    base_path.parent().unwrap_or(Path::new("."))
                } else {
                    base_path
                };
                let joined = dir.join(&normalized);
                return joined.to_string_lossy().to_string();
            }
        }

        normalized
    }

    pub fn normalize_url(raw: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.starts_with("https:/") && !trimmed.starts_with("https://") {
            format!("https://{}", &trimmed["https:/".len()..])
        } else if trimmed.starts_with("http:/") && !trimmed.starts_with("http://") {
            format!("http://{}", &trimmed["http:/".len()..])
        } else if trimmed.starts_with("lumra:/") && !trimmed.starts_with("lumra://") {
            format!("lumra://{}", &trimmed["lumra:/".len()..])
        } else if trimmed.starts_with("file:/") && !trimmed.starts_with("file://") {
            format!("file://{}", &trimmed["file:/".len()..])
        } else {
            trimmed.to_string()
        }
    }

    /// Asynchronous fetch for top-level pages
    pub async fn fetch(&self, url_str: &str) -> Result<String> {
        let trimmed = url_str.trim();

        // 1. Internal lumra:// scheme
        if let Some(path) = trimmed.strip_prefix("lumra://") {
            return self.handle_internal_page(path);
        }

        // 2. Local file:// scheme
        if let Some(path) = trimmed.strip_prefix("file://") {
            let path_buf = PathBuf::from(path);
            let bytes = read_file_limited(&path_buf, "local document")?;
            return String::from_utf8(bytes)
                .map_err(|e| anyhow!("Local document is not valid UTF-8: {}", e));
        }

        // Check if direct local path
        if Path::new(trimmed).is_file() {
            let bytes = read_file_limited(Path::new(trimmed), "local document")?;
            return String::from_utf8(bytes)
                .map_err(|e| anyhow!("Local document is not valid UTF-8: {}", e));
        }

        // 3. Remote HTTP/HTTPS fetch
        let resolved_url = if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
            format!("https://{}", trimmed)
        } else {
            trimmed.to_string()
        };

        // Fail fast before DNS resolution: do not build an unbounded waiter queue.
        let _request_permit = self.remote_requests
            .try_acquire()
            .map_err(|_| anyhow!("Too many concurrent remote resource requests (limit: {})", MAX_CONCURRENT_REMOTE_REQUESTS))?;
        let parsed_url = Url::parse(&resolved_url)
            .map_err(|e| anyhow!("Invalid URL {}: {}", resolved_url, e))?;
        if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
            return Err(anyhow!("Unsupported network scheme: {}", parsed_url.scheme()));
        }
        let client = build_pinned_async_client(&parsed_url).await?;
        let response = client
            .get(&resolved_url)
            .header("Accept", "text/typst, text/plain;q=0.9, */*;q=0.8")
            .send()
            .await
            .map_err(|e| anyhow!("Network error: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!("HTTP Status: {}", response.status()));
        }

        if response.content_length().is_some_and(|len| len > MAX_RESOURCE_BYTES as u64) {
            return Err(anyhow!("Resource exceeds the {} MiB limit", MAX_RESOURCE_BYTES / (1024 * 1024)));
        }
        let mut response = response;
        let mut body = bytes::BytesMut::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| anyhow!("Failed to read response body: {}", e))?
        {
            if body.len().saturating_add(chunk.len()) > MAX_RESOURCE_BYTES {
                return Err(anyhow!("Resource exceeds the {} MiB limit", MAX_RESOURCE_BYTES / (1024 * 1024)));
            }
            body.extend_from_slice(&chunk);
        }
        String::from_utf8(body.to_vec())
            .map_err(|e| anyhow!("Response is not valid UTF-8: {}", e))
    }

    /// Synchronous fetch for text (used by #import and #include)
    pub fn fetch_text_blocking(&self, url_str: &str, local_root: Option<&Path>) -> Result<String> {
        let bytes = self.fetch_bytes_blocking(url_str, local_root)?;
        String::from_utf8(bytes.to_vec())
            .map_err(|e| anyhow!("UTF-8 decode error for {}: {}", url_str, e))
    }

    /// Synchronous fetch for raw bytes (used by #image, #read, etc.)
    pub fn fetch_bytes_blocking(&self, url_str: &str, local_root: Option<&Path>) -> Result<Bytes> {
        let normalized = Self::normalize_url(url_str);
        let trimmed = normalized.trim();

        // Resolve local paths before cache lookup so a cached entry cannot bypass
        // the current document's filesystem capability boundary.
        let local_path = if let Some(path) = trimmed.strip_prefix("file://") {
            Some(PathBuf::from(path))
        } else if !trimmed.contains("://")
            && !trimmed.starts_with("http://")
            && !trimmed.starts_with("https://")
            && !trimmed.starts_with("lumra://")
        {
            Some(PathBuf::from(trimmed))
        } else {
            None
        };

        let local_path = if let Some(path) = local_path {
            let root = local_root.ok_or_else(|| {
                anyhow!("Local resource access is denied for remote documents: {}", trimmed)
            })?;
            let canonical_root = root.canonicalize()
                .map_err(|e| anyhow!("Failed to resolve local document root: {}", e))?;
            let canonical_path = path.canonicalize()
                .map_err(|e| anyhow!("Failed to resolve local resource {}: {}", trimmed, e))?;
            if !canonical_path.starts_with(&canonical_root) {
                return Err(anyhow!("Local resource is outside the opened document directory: {}", trimmed));
            }
            Some(canonical_path)
        } else {
            None
        };

        // 1. Check in-memory asset cache
        {
            let cache = self.asset_cache.lock().unwrap();
            if let Some(cached) = cache.get(&normalized) {
                return Ok(cached);
            }
        }

        // 2. Handle internal lumra:// assets
        if let Some(path) = trimmed.strip_prefix("lumra://") {
            if let Some(bytes) = self.handle_internal_asset(path) {
                self.cache_asset(&normalized, bytes.clone());
                return Ok(bytes);
            }
            return Err(anyhow!("Unknown lumra asset: {}", trimmed));
        }

        // 3. Handle local resources within the explicitly opened document directory.
        if let Some(path) = local_path {
            let data = read_file_limited(&path, "local asset")?;
            let bytes = Bytes::from(data);
            self.cache_asset(&normalized, bytes.clone());
            return Ok(bytes);
        }

        // 4. Remote HTTP/HTTPS fetch
        let resolved_url = if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
            format!("https://{}", trimmed)
        } else {
            trimmed.to_string()
        };

        // Blocking compiler asset fetches fail fast rather than creating an
        // unbounded queue of worker threads waiting for network access.
        let _request_permit = self.remote_requests
            .try_acquire()
            .map_err(|_| anyhow!("Too many concurrent remote resource requests (limit: {})", MAX_CONCURRENT_REMOTE_REQUESTS))?;
        let parsed_url = Url::parse(&resolved_url)
            .map_err(|e| anyhow!("Invalid URL {}: {}", resolved_url, e))?;
        if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
            return Err(anyhow!("Unsupported network scheme: {}", parsed_url.scheme()));
        }
        let client = build_pinned_blocking_client(&parsed_url)?;
        let response = client
            .get(&resolved_url)
            .header("Accept", "image/*, application/octet-stream, text/*, */*")
            .send()
            .map_err(|e| anyhow!("Failed to request {}: {}", resolved_url, e))?;

        if !response.status().is_success() {
            return Err(anyhow!("HTTP Status {} for {}", response.status(), resolved_url));
        }

        if response.content_length().is_some_and(|len| len > MAX_RESOURCE_BYTES as u64) {
            return Err(anyhow!("Resource exceeds the {} MiB limit: {}", MAX_RESOURCE_BYTES / (1024 * 1024), resolved_url));
        }
        let mut limited = response.take(MAX_RESOURCE_BYTES as u64 + 1);
        let mut data = Vec::new();
        limited
            .read_to_end(&mut data)
            .map_err(|e| anyhow!("Failed to download {}: {}", resolved_url, e))?;
        if data.len() > MAX_RESOURCE_BYTES {
            return Err(anyhow!("Resource exceeds the {} MiB limit: {}", MAX_RESOURCE_BYTES / (1024 * 1024), resolved_url));
        }
        let raw = Bytes::from(data);

        self.cache_asset(&normalized, raw.clone());
        Ok(raw)
    }

    fn cache_asset(&self, key: &str, bytes: Bytes) {
        let mut cache = self.asset_cache.lock().unwrap();
        cache.insert(key.to_string(), bytes);
    }

    fn handle_internal_asset(&self, asset_path: &str) -> Option<Bytes> {
        match asset_path.trim_start_matches('/') {
            "logo.svg" | "assets/logo.svg" => Some(Bytes::from_static(LUMRA_LOGO_SVG.as_bytes())),
            _ => None,
        }
    }

    fn handle_internal_page(&self, page: &str) -> Result<String> {
        match page.trim_end_matches('/') {
            "" | "home" | "welcome" => Ok(HOME_PAGE.to_string()),
            "about" => Ok(ABOUT_PAGE.to_string()),
            "cheatsheet" => Ok(CHEATSHEET_PAGE.to_string()),
            other => Err(anyhow!("Unknown internal page: lumra://{}", other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocking_remote_fetch_respects_global_concurrency_budget() {
        let fetcher = ContentFetcher::new();
        let _all_permits = fetcher
            .remote_requests
            .try_acquire_many(MAX_CONCURRENT_REMOTE_REQUESTS as u32)
            .unwrap();

        let result = fetcher.fetch_bytes_blocking("https://example.com/asset.bin", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Too many concurrent"));
    }

    #[test]
    fn async_page_fetch_respects_global_concurrency_budget() {
        let fetcher = ContentFetcher::new();
        let _all_permits = fetcher
            .remote_requests
            .try_acquire_many(MAX_CONCURRENT_REMOTE_REQUESTS as u32)
            .unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let result = runtime.block_on(fetcher.fetch("https://example.com/"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Too many concurrent"));
    }

    #[test]
    fn private_and_special_use_addresses_are_blocked() {
        let blocked = [
            "127.0.0.1",
            "10.0.0.1",
            "169.254.169.254",
            "100.64.0.1",
            "198.18.0.1",
            "::1",
            "fd00::1",
            "fe80::1",
            "64:ff9b::808:808",
            "2002:0808:0808::1",
            "2001:0:4136:e378:8000:63bf:3fff:fdd2",
        ];
        for value in blocked {
            let ip = value.parse::<IpAddr>().unwrap();
            assert!(is_blocked_ip(ip), "expected {} to be blocked", value);
        }
        assert!(!is_blocked_ip("1.1.1.1".parse().unwrap()));
        assert!(!is_blocked_ip("2606:4700:4700::1111".parse().unwrap()));
    }

    #[test]
    fn internal_hostnames_are_blocked() {
        let url = Url::parse("https://metadata.google.internal/").unwrap();
        let result = validate_network_target(
            &url,
            &["142.250.72.14:443".parse::<SocketAddr>().unwrap()],
        );
        assert!(result.is_err());
    }

    #[test]
    fn remote_document_cannot_read_local_file_urls() {
        let fetcher = ContentFetcher::new();
        let result = fetcher.fetch_bytes_blocking("file:///etc/passwd", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("denied"));
    }

    #[test]
    fn local_document_cannot_escape_its_directory() {
        let fetcher = ContentFetcher::new();
        let unique = format!(
            "lumra-security-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(&unique).join("site");
        let outside = std::env::temp_dir().join(&unique).join("secret.txt");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&outside, b"secret").unwrap();

        let outside_url = format!("file://{}", outside.display());
        let result = fetcher.fetch_bytes_blocking(&outside_url, Some(&root));

        let _ = std::fs::remove_dir_all(std::env::temp_dir().join(&unique));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("outside"));
    }

    #[test]
    fn local_document_can_read_resources_inside_its_directory() {
        let fetcher = ContentFetcher::new();
        let unique = format!(
            "lumra-security-allowed-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&root).unwrap();
        let asset = root.join("asset.bin");
        std::fs::write(&asset, b"allowed").unwrap();

        let asset_url = format!("file://{}", asset.display());
        let result = fetcher.fetch_bytes_blocking(&asset_url, Some(&root));

        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(result.unwrap().as_ref(), b"allowed");
    }

    #[test]
    fn test_url_resolution() {
        let fetcher = ContentFetcher::new();

        // 1. Relative image on remote site
        let resolved = fetcher.resolve_url(Some("https://example.com/blog/article.typ"), "banner.png");
        assert_eq!(resolved, "https://example.com/blog/banner.png");

        // 2. Parent directory relative image
        let resolved = fetcher.resolve_url(Some("https://example.com/blog/article.typ"), "../images/logo.svg");
        assert_eq!(resolved, "https://example.com/images/logo.svg");

        // 3. Absolute URL should remain absolute
        let resolved = fetcher.resolve_url(Some("https://example.com/blog/"), "https://cdn.foo.com/pic.jpg");
        assert_eq!(resolved, "https://cdn.foo.com/pic.jpg");

        // 4. VirtualPath single slash normalization
        let resolved = fetcher.resolve_url(None, "lumra:/assets/logo.svg");
        assert_eq!(resolved, "lumra://assets/logo.svg");

        // 5. File URL resolution
        let resolved = fetcher.resolve_url(Some("file:///Users/test/index.typ"), "components.typ");
        assert_eq!(resolved, "file:///Users/test/components.typ");
    }

    #[test]
    fn asset_cache_evicts_oldest_entries_to_respect_budget() {
        let mut cache = AssetCache::new();
        let first = Bytes::from(vec![1u8; MAX_CACHE_BYTES / 2]);
        let second = Bytes::from(vec![2u8; MAX_CACHE_BYTES / 2]);
        let third = Bytes::from(vec![3u8; MAX_CACHE_BYTES / 2]);

        cache.insert("first".to_string(), first);
        cache.insert("second".to_string(), second);
        cache.insert("third".to_string(), third);

        assert!(cache.get("first").is_none());
        assert!(cache.get("second").is_some());
        assert!(cache.get("third").is_some());
        assert!(cache.total_bytes <= MAX_CACHE_BYTES);
    }

    #[test]
    fn asset_cache_does_not_store_entries_larger_than_budget() {
        let mut cache = AssetCache::new();
        cache.insert(
            "oversized".to_string(),
            Bytes::from(vec![0u8; MAX_CACHE_BYTES + 1]),
        );

        assert!(cache.get("oversized").is_none());
        assert_eq!(cache.total_bytes, 0);
    }
}

pub const LUMRA_LOGO_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100">
  <defs>
    <linearGradient id="grad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#3b82f6" />
      <stop offset="100%" stop-color="#1d4ed8" />
    </linearGradient>
  </defs>
  <rect width="100" height="100" rx="22" fill="url(#grad)" />
  <circle cx="50" cy="50" r="32" fill="none" stroke="#ffffff" stroke-width="6" opacity="0.4" />
  <path d="M34 28 L34 72 L68 72" fill="none" stroke="#ffffff" stroke-width="8" stroke-linecap="round" stroke-linejoin="round"/>
  <circle cx="68" cy="40" r="7" fill="#60a5fa" />
</svg>"##;

pub const HOME_PAGE: &str = r#"
#set page(paper: "a4", margin: (x: 2cm, y: 2.2cm))
#set text(font: ("Libertinus Serif", "New Computer Modern Math"), size: 11pt)
#show math.equation: set text(font: "New Computer Modern Math")

#align(center)[
  #image("lumra://assets/logo.svg", width: 56pt)
  #v(0.3em)
  #text(size: 26pt, weight: "bold", fill: rgb("1b3b6f"))[Lumra Browser] \
  #text(size: 12pt, style: "italic", fill: rgb("4a5568"))[The Fast, Typeset-Native Web Browser]
]

#v(0.8em)

#block(
  fill: rgb("edf2f7"),
  inset: 14pt,
  radius: 8pt,
  stroke: 0.5pt + rgb("cbd5e0"),
)[
  *ยินดีต้อนรับสู่ Lumra Browser!* \
  เบราว์เซอร์ยุคใหม่ที่แสดงผลหน้าเว็บด้วยเอกสาร *Typst (`.typ`)* โดยตรง 
  เพลิดเพลินกับความเร็วระดับมิลลิวินาทีและการจัดหน้าสวยงามระดับสื่อสิ่งพิมพ์ แทนที่ HTML/CSS ดั้งเดิม
  รองรับรูปภาพ Remote Assets (#raw("lumra://assets/logo.svg")), ฟอนต์, สูตรคณิตศาสตร์ และไฮเปอร์ลิงก์
]

#v(1em)

== 🧭 เริ่มต้นการสำรวจ
- เข้าชมเอกสารแนะนำ: #link("lumra://cheatsheet")[คู่มือ Typst Cheatsheet ฉบับรวดเร็ว]
- เกี่ยวกับเบราว์เซอร์: #link("lumra://about")[ข้อมูลและสถาปัตยกรรมของ Lumra]
- หรือพิมพ์ URL เว็บไซต์ หรือ `file:///path/to/file.typ` ในแถบด้านบนเพื่อเปิดดูเอกสาร

#v(1em)

== 📐 พลังของ Typst Typesetting
รองรับการเขียนสมการคณิตศาสตร์และตารางสไตล์สื่อสิ่งพิมพ์อย่างเป็นธรรมชาติ:

$ sum_(k=1)^n k = (n(n+1)) / 2 $

$ f(x) = integral_(-infinity)^x 1 / (sigma sqrt(2 pi)) e^(- (t - mu)^2 / (2 sigma^2)) dif t $

#v(1em)

#table(
  columns: (1.2fr, 2fr, 2fr),
  align: (center + horizon, left, left),
  fill: (x, y) => if y == 0 { rgb("e2e8f0") } else { none },
  [*มิติ*], [*HTML / Web เดิม*], [*Lumra + Typst*],
  [การจัดวางหน้า], [ต้องพึ่งพา CSS Box Model ที่ซับซ้อน], [ระบบ Typesetting Engine แท้ระดับโลก],
  [รูปภาพและ Assets], [ต้องเขียนแท็ก <img> พร้อม JS polyfills], [Native Remote Asset Fetcher ในตัว],
  [สมการคณิตศาสตร์], [ต้องเรนเดอร์ผ่าน JS (MathJax/KaTeX)], [Native High-Quality Math Engine],
  [ความเร็ว], [รัน Browser Engine ขนาดใหญ่หลายร้อย MB], [Rust Engine ขนาดกะทัดรัด คอมไพล์สดในไม่กี่ ms],
)
"#;

pub const ABOUT_PAGE: &str = r#"
#set page(paper: "a4", margin: (x: 2.5cm, y: 2.5cm))
#set text(font: ("Libertinus Serif", "New Computer Modern Math"), size: 11pt)
#show math.equation: set text(font: "New Computer Modern Math")

#align(center)[
  #image("lumra://assets/logo.svg", width: 48pt)
  #v(0.3em)
  #text(size: 24pt, weight: "bold", fill: rgb("1b3b6f"))[เกี่ยวกับ Lumra Browser] \
  #text(size: 11pt, fill: rgb("718096"))[เวอร์ชัน 0.2.0 (Typst Engine Edition)]
]

#v(1.5em)

== 🎯 วิสัยทัศน์
*Lumra* มีเป้าหมายในการสร้างมาตรฐานใหม่สำหรับการอ่านและนำเสนอเนื้อหาบนอินเทอร์เน็ต โดยใช้ *Typst* เป็นภาษาหลักในการเขียนเว็บแทนที่ HTML/CSS 

ทำให้เว็บเพจ:
1. *เร็วและเบาที่สุด* — ไม่มี JavaScript bloatware หรือ CSS framework ที่หนักเครื่อง
2. *สวยงาม ไร้ที่ติ* — ทุกตัวอักษร ช่องไฟ สมการ และรูปภาพได้รับการจัดวางด้วยมาตรฐานสิ่งพิมพ์ระดับมืออาชีพ
3. *ปลอดภัย 100%* — ไม่มีการรันสคริปต์สอดแนม หรือมัลแวร์ในระดับเบราว์เซอร์
4. *เชื่อมต่อสินทรัพย์ผ่านเว็บ* — รองรับการโหลดรูปภาพ โมดูล และฟอนต์ผ่านเน็ตเวิร์กได้อย่างราบรื่น

#v(1em)
กลับสู่: #link("lumra://home")[หน้าแรก Lumra Home]
"#;

pub const CHEATSHEET_PAGE: &str = r#"
#set page(paper: "a4", margin: (x: 2cm, y: 2cm))
#set text(font: ("Libertinus Serif", "New Computer Modern Math"), size: 11pt)
#show math.equation: set text(font: "New Computer Modern Math")

= 📝 Typst Syntax Cheatsheet

== 1. หัวข้อและข้อความ
- `= Heading 1`, `== Heading 2`, `=== Heading 3`
- `*ตัวหนา (Bold)*` และ `_ตัวเอียง (Italic)_`
- ขีดฆ่า: `~Strikethrough~`

== 2. ลิงก์และรูปภาพ (Hyperlinks & Images)
- ลิงก์: `#link("https://example.com")[ข้อความที่ต้องการ]`
- รูปภาพ: `#image("lumra://assets/logo.svg", width: 40pt)`
- รูปภาพจากเน็ต: `#image("https://example.com/photo.png", width: 100%)`

== 3. สมการคณิตศาสตร์ (Math)
- สมการในบรรทัด: `$x + y = z$`
- สมการแยกบรรทัด:
$ A = mat(
  1, 2;
  3, 4
) $

== 4. บล็อกโค้ด (Code Blocks)
```rust
fn main() {
    println!("Hello from Lumra Browser!");
}
```

#v(1em)
กลับสู่: #link("lumra://home")[หน้าแรก Lumra Home]
"#;
