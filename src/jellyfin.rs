use std::{collections::BTreeMap, time::Duration};

use futures::StreamExt;
use reqwest::{Client, Url};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_JSON_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid Jellyfin URL: {0}")]
    InvalidUrl(#[from] url::ParseError),
    #[error("Jellyfin URL must not contain user credentials")]
    CredentialsInUrl,
    #[error("Jellyfin URL must use http or https")]
    InvalidScheme,
    #[error("Jellyfin URL must not contain a query or fragment")]
    QueryOrFragment,
    #[error("Jellyfin endpoint escaped the configured server origin")]
    CrossOrigin,
    #[error("Jellyfin request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("Jellyfin returned invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Jellyfin JSON response exceeds the size limit")]
    JsonTooLarge,
    #[error("Jellyfin image exceeds the download size limit")]
    ImageTooLarge,
    #[error("Jellyfin returned unexpected HTTP status {0}")]
    UnexpectedStatus(u16),
    #[error("Jellyfin item has no Primary image tag")]
    MissingPrimaryImage,
    #[error("configured Jellyfin library was not returned by the server: {0}")]
    MissingLibrary(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JellyfinConfig {
    pub url: String,
    pub library_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JellyfinLibrary {
    pub id: String,
    pub name: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectionStatus {
    pub server_name: String,
    pub version: String,
    pub server_id: String,
    pub libraries: Vec<JellyfinLibrary>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SystemInfo {
    server_name: String,
    version: String,
    id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct QueryResult<T> {
    items: Vec<T>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct LibraryDto {
    id: String,
    name: String,
    path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinUserData {
    #[serde(default)]
    pub played: bool,
    #[serde(default)]
    pub play_count: u32,
    #[serde(default)]
    pub playback_position_ticks: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinItem {
    pub id: String,
    pub name: String,
    pub path: Option<String>,
    #[serde(default)]
    pub provider_ids: BTreeMap<String, String>,
    #[serde(default)]
    pub user_data: JellyfinUserData,
    #[serde(default)]
    pub image_tags: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinPerson {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub image_tags: BTreeMap<String, String>,
}

impl JellyfinPerson {
    pub fn fixture(id: &str, name: &str, primary_image_tag: Option<&str>) -> Self {
        let mut image_tags = BTreeMap::new();
        if let Some(tag) = primary_image_tag {
            image_tags.insert("Primary".to_owned(), tag.to_owned());
        }
        Self {
            id: id.to_owned(),
            name: name.to_owned(),
            image_tags,
        }
    }

    pub fn primary_image_tag(&self) -> Option<&str> {
        self.image_tags.get("Primary").map(String::as_str)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JellyfinImage {
    pub bytes: Vec<u8>,
    pub content_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JellyfinImageRef {
    pub item_id: String,
    pub image_tag: String,
}

impl JellyfinItem {
    pub fn fixture(id: &str, name: &str, path: Option<&str>, code: Option<&str>) -> Self {
        let mut provider_ids = BTreeMap::new();
        if let Some(code) = code {
            provider_ids.insert("Jav".to_owned(), code.to_owned());
        }
        Self {
            id: id.to_owned(),
            name: name.to_owned(),
            path: path.map(str::to_owned),
            provider_ids,
            user_data: JellyfinUserData::default(),
            image_tags: BTreeMap::new(),
        }
    }

    pub fn primary_image_ref(&self) -> Option<JellyfinImageRef> {
        primary_image_ref(&self.id, &self.image_tags)
    }
}

impl JellyfinPerson {
    pub fn primary_image_ref(&self) -> Option<JellyfinImageRef> {
        primary_image_ref(&self.id, &self.image_tags)
    }
}

fn primary_image_ref(
    item_id: &str,
    image_tags: &BTreeMap<String, String>,
) -> Option<JellyfinImageRef> {
    let image_tag = image_tags.get("Primary")?.trim();
    (!item_id.trim().is_empty() && !image_tag.is_empty()).then(|| JellyfinImageRef {
        item_id: item_id.to_owned(),
        image_tag: image_tag.to_owned(),
    })
}

#[derive(Debug, Clone)]
pub struct JellyfinClient {
    client: Client,
    base_url: Url,
    config: JellyfinConfig,
    api_key: String,
    cache_fingerprint: [u8; 32],
}

impl JellyfinClient {
    pub fn new(mut config: JellyfinConfig, api_key: String) -> Result<Self, Error> {
        let mut base_url = Url::parse(config.url.trim())?;
        if !base_url.username().is_empty() || base_url.password().is_some() {
            return Err(Error::CredentialsInUrl);
        }
        if !matches!(base_url.scheme(), "http" | "https") {
            return Err(Error::InvalidScheme);
        }
        if base_url.query().is_some() || base_url.fragment().is_some() {
            return Err(Error::QueryOrFragment);
        }
        if !base_url.path().ends_with('/') {
            base_url.set_path(&format!("{}/", base_url.path().trim_end_matches('/')));
        }
        config.url = base_url.as_str().trim_end_matches('/').to_owned();
        config.library_ids.sort();
        config.library_ids.dedup();
        let cache_fingerprint = config_fingerprint(&config, &api_key);
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            client,
            base_url,
            config,
            api_key,
            cache_fingerprint,
        })
    }

    fn endpoint(&self, path: &str) -> Result<Url, Error> {
        let endpoint = self.base_url.join(path.trim_start_matches('/'))?;
        self.require_same_origin(endpoint)
    }

    fn image_endpoint(&self, item_id: &str) -> Result<Url, Error> {
        let mut endpoint = self.base_url.clone();
        endpoint
            .path_segments_mut()
            .map_err(|_| Error::CrossOrigin)?
            .pop_if_empty()
            .push("Items")
            .push(item_id)
            .push("Images")
            .push("Primary");
        self.require_same_origin(endpoint)
    }

    fn require_same_origin(&self, endpoint: Url) -> Result<Url, Error> {
        if endpoint.origin() != self.base_url.origin() {
            return Err(Error::CrossOrigin);
        }
        Ok(endpoint)
    }

    fn get(&self, path: &str) -> Result<reqwest::RequestBuilder, Error> {
        Ok(self
            .client
            .get(self.endpoint(path)?)
            .header("X-Emby-Token", &self.api_key))
    }

    async fn bounded_json<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<T, Error> {
        let bytes = self.bounded_body(request, MAX_JSON_RESPONSE_BYTES).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    async fn bounded_body(
        &self,
        request: reqwest::RequestBuilder,
        max_bytes: usize,
    ) -> Result<Vec<u8>, Error> {
        let response = request.send().await?;
        if !response.status().is_success() {
            return Err(Error::UnexpectedStatus(response.status().as_u16()));
        }
        if response
            .content_length()
            .is_some_and(|length| length > max_bytes as u64)
        {
            return Err(Error::JsonTooLarge);
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if bytes.len().saturating_add(chunk.len()) > max_bytes {
                return Err(Error::JsonTooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }

    pub async fn test_connection(&self) -> Result<ConnectionStatus, Error> {
        let server: SystemInfo = self.bounded_json(self.get("System/Info")?).await?;
        let result: QueryResult<LibraryDto> =
            self.bounded_json(self.get("Library/MediaFolders")?).await?;
        let mut libraries = Vec::new();
        for id in &self.config.library_ids {
            let library = result
                .items
                .iter()
                .find(|library| &library.id == id)
                .ok_or_else(|| Error::MissingLibrary(id.clone()))?;
            libraries.push(JellyfinLibrary {
                id: library.id.clone(),
                name: library.name.clone(),
                path: library.path.clone(),
            });
        }
        Ok(ConnectionStatus {
            server_name: server.server_name,
            version: server.version,
            server_id: server.id,
            libraries,
        })
    }

    pub async fn selected_items(&self) -> Result<Vec<JellyfinItem>, Error> {
        let mut items = Vec::new();
        let mut response_bytes = 0usize;
        for library_id in &self.config.library_ids {
            let remaining = MAX_JSON_RESPONSE_BYTES
                .checked_sub(response_bytes)
                .ok_or(Error::JsonTooLarge)?;
            let bytes = self
                .bounded_body(
                    self.get("Items")?.query(&[
                        ("parentId", library_id.as_str()),
                        ("recursive", "true"),
                        ("fields", "Path,ProviderIds,UserData,ImageTags"),
                    ]),
                    remaining,
                )
                .await?;
            response_bytes = response_bytes
                .checked_add(bytes.len())
                .ok_or(Error::JsonTooLarge)?;
            let result: QueryResult<JellyfinItem> = serde_json::from_slice(&bytes)?;
            items.extend(result.items);
        }
        Ok(items)
    }

    pub async fn people(&self) -> Result<Vec<JellyfinPerson>, Error> {
        let result: QueryResult<JellyfinPerson> = self
            .bounded_json(self.get("Persons")?.query(&[("fields", "ImageTags")]))
            .await?;
        Ok(result.items)
    }

    pub async fn primary_image(
        &self,
        person: &JellyfinPerson,
        max_width: u32,
    ) -> Result<JellyfinImage, Error> {
        let image = person
            .primary_image_ref()
            .ok_or(Error::MissingPrimaryImage)?;
        self.primary_image_ref(&image, max_width).await
    }

    pub async fn primary_image_ref(
        &self,
        image: &JellyfinImageRef,
        max_width: u32,
    ) -> Result<JellyfinImage, Error> {
        let response = self
            .client
            .get(self.image_endpoint(&image.item_id)?)
            .header("X-Emby-Token", &self.api_key)
            .query(&[
                ("maxWidth", max_width.to_string()),
                ("tag", image.image_tag.clone()),
            ])
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(Error::UnexpectedStatus(response.status().as_u16()));
        }
        if response
            .content_length()
            .is_some_and(|length| length > crate::artwork_image::MAX_ARTWORK_BYTES)
        {
            return Err(Error::ImageTooLarge);
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("image/jpeg")
            .to_owned();
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if bytes.len().saturating_add(chunk.len())
                > crate::artwork_image::MAX_ARTWORK_BYTES as usize
            {
                return Err(Error::ImageTooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(JellyfinImage {
            bytes,
            content_type,
        })
    }

    pub(crate) fn cache_fingerprint(&self) -> &[u8; 32] {
        &self.cache_fingerprint
    }

    pub fn open_url(&self, item_id: &str) -> String {
        format!("{}web/#/details?id={}", self.base_url, item_id)
    }

    pub async fn refresh_batch(&self, policy: RetryPolicy) -> RefreshOutcome {
        let attempts = policy.max_attempts.clamp(1, 5);
        for attempt in 1..=attempts {
            let result = self
                .client
                .post(match self.endpoint("Library/Refresh") {
                    Ok(url) => url,
                    Err(_) => return RefreshOutcome::ManualRetryRequired { attempts: attempt },
                })
                .header("X-Emby-Token", &self.api_key)
                .send()
                .await;
            if result
                .map(|response| response.status().is_success())
                .unwrap_or(false)
            {
                return RefreshOutcome::Completed { attempts: attempt };
            }
            if attempt < attempts {
                tokio::time::sleep(policy.delay(attempt)).await;
            }
        }
        RefreshOutcome::ManualRetryRequired { attempts }
    }
}

fn config_fingerprint(config: &JellyfinConfig, api_key: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"rust-jav-jellyfin-config-v1\0");
    fingerprint_field(&mut hasher, config.url.as_bytes());
    for library_id in &config.library_ids {
        fingerprint_field(&mut hasher, library_id.as_bytes());
    }
    fingerprint_field(&mut hasher, api_key.as_bytes());
    hasher.finalize().into()
}

fn fingerprint_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_attempts: u8,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl RetryPolicy {
    fn delay(self, failed_attempt: u8) -> Duration {
        self.base_delay
            .saturating_mul(1u32 << failed_attempt.saturating_sub(1))
            .min(self.max_delay)
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefreshOutcome {
    Completed { attempts: u8 },
    ManualRetryRequired { attempts: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssociationConfidence {
    CertainPath,
    UncertainMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Association {
    pub item_id: String,
    pub confidence: AssociationConfidence,
    pub reason: String,
    pub played: bool,
    pub play_count: u32,
    pub playback_position_ticks: u64,
}

impl Association {
    pub fn may_authorize_deletion(&self) -> bool {
        self.confidence == AssociationConfidence::CertainPath
    }
}

pub fn associate(
    path: &str,
    jav_code: Option<&str>,
    title: Option<&str>,
    items: &[JellyfinItem],
) -> Option<Association> {
    let local_path = normalize_path(path);
    if let Some(item) = items.iter().find(|item| {
        item.path.as_deref().map(normalize_path).as_deref() == Some(local_path.as_str())
    }) {
        return Some(association(
            item,
            AssociationConfidence::CertainPath,
            "normalized path",
        ));
    }
    let local_parts = normalized_path_parts(&local_path);
    let suffix_matches = items
        .iter()
        .filter_map(|item| {
            let path = normalize_path(item.path.as_deref()?);
            let common = common_path_suffix(&local_parts, &normalized_path_parts(&path));
            (common >= 3).then_some((item, common))
        })
        .collect::<Vec<_>>();
    if let Some(maximum) = suffix_matches.iter().map(|(_, common)| *common).max() {
        let mut best = suffix_matches
            .iter()
            .filter(|(_, common)| *common == maximum)
            .map(|(item, _)| *item);
        if let Some(item) = best.next() {
            if best.next().is_none() {
                return Some(association(
                    item,
                    AssociationConfidence::CertainPath,
                    "unique normalized relative path suffix",
                ));
            }
        }
    }
    let code = jav_code.map(normalize_metadata);
    let title = title.map(normalize_metadata);
    items
        .iter()
        .find(|item| {
            let item_name = normalize_metadata(&item.name);
            code.as_ref().is_some_and(|code| {
                item_name == *code
                    || item
                        .provider_ids
                        .values()
                        .any(|value| normalize_metadata(value) == *code)
            }) || title.as_ref().is_some_and(|title| item_name == *title)
        })
        .map(|item| {
            association(
                item,
                AssociationConfidence::UncertainMetadata,
                "JAV code or title metadata; verify manually",
            )
        })
}

fn normalized_path_parts(value: &str) -> Vec<&str> {
    value.split('/').filter(|part| !part.is_empty()).collect()
}

fn common_path_suffix(left: &[&str], right: &[&str]) -> usize {
    left.iter()
        .rev()
        .zip(right.iter().rev())
        .take_while(|(left, right)| left == right)
        .count()
}

fn association(
    item: &JellyfinItem,
    confidence: AssociationConfidence,
    reason: &str,
) -> Association {
    Association {
        item_id: item.id.clone(),
        confidence,
        reason: reason.to_owned(),
        played: item.user_data.played,
        play_count: item.user_data.play_count,
        playback_position_ticks: item.user_data.playback_position_ticks,
    }
}

fn normalize_metadata(value: &str) -> String {
    value.trim().to_lowercase().replace(['_', ' '], "-")
}

pub fn match_person<'a>(name: &str, people: &'a [JellyfinPerson]) -> Option<&'a JellyfinPerson> {
    let wanted = normalize_person_name(name);
    let mut matches = people
        .iter()
        .filter(|person| normalize_person_name(&person.name) == wanted);
    let matched = matches.next()?;
    (matches.next().is_none() && matched.primary_image_tag().is_some()).then_some(matched)
}

fn normalize_person_name(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|character| match character {
            '\u{3000}' => ' ',
            '\u{ff01}'..='\u{ff5e}' => char::from_u32(character as u32 - 0xfee0).unwrap(),
            other => other,
        })
        .flat_map(char::to_lowercase)
        .collect()
}

fn normalize_path(value: &str) -> String {
    let value = value.trim().replace('\\', "/");
    let mut prefix = "";
    let mut rest = value.as_str();
    if rest.len() >= 2 && rest.as_bytes()[1] == b':' {
        prefix = &rest[..2];
        rest = &rest[2..];
    }
    let mut parts = Vec::new();
    for part in rest.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    format!("{prefix}/{}", parts.join("/"))
        .trim_end_matches('/')
        .to_lowercase()
}
