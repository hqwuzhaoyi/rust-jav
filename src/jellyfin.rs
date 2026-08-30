use std::{collections::BTreeMap, time::Duration};

use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid Jellyfin URL: {0}")]
    InvalidUrl(#[from] url::ParseError),
    #[error("Jellyfin request failed: {0}")]
    Request(#[from] reqwest::Error),
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
        }
    }
}

#[derive(Debug, Clone)]
pub struct JellyfinClient {
    client: Client,
    base_url: Url,
    config: JellyfinConfig,
    api_key: String,
}

impl JellyfinClient {
    pub fn new(mut config: JellyfinConfig, api_key: String) -> Result<Self, Error> {
        let mut base_url = Url::parse(config.url.trim())?;
        if !base_url.path().ends_with('/') {
            base_url.set_path(&format!("{}/", base_url.path().trim_end_matches('/')));
        }
        config.url = base_url.as_str().trim_end_matches('/').to_owned();
        config.library_ids.sort();
        config.library_ids.dedup();
        Ok(Self {
            client: Client::new(),
            base_url,
            config,
            api_key,
        })
    }

    fn endpoint(&self, path: &str) -> Result<Url, Error> {
        Ok(self.base_url.join(path.trim_start_matches('/'))?)
    }

    fn get(&self, path: &str) -> Result<reqwest::RequestBuilder, Error> {
        Ok(self
            .client
            .get(self.endpoint(path)?)
            .header("X-Emby-Token", &self.api_key))
    }

    pub async fn test_connection(&self) -> Result<ConnectionStatus, Error> {
        let server: SystemInfo = self
            .get("System/Info")?
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let result: QueryResult<LibraryDto> = self
            .get("Library/MediaFolders")?
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
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
        for library_id in &self.config.library_ids {
            let result: QueryResult<JellyfinItem> = self
                .get("Items")?
                .query(&[
                    ("parentId", library_id.as_str()),
                    ("recursive", "true"),
                    ("fields", "Path,ProviderIds,UserData"),
                ])
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            items.extend(result.items);
        }
        Ok(items)
    }

    pub async fn people(&self) -> Result<Vec<JellyfinPerson>, Error> {
        let result: QueryResult<JellyfinPerson> = self
            .get("Persons")?
            .query(&[("fields", "ImageTags")])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(result.items)
    }

    pub async fn primary_image(
        &self,
        person: &JellyfinPerson,
        max_width: u32,
    ) -> Result<JellyfinImage, Error> {
        let tag = person.primary_image_tag().unwrap_or_default();
        let response = self
            .get(&format!("Items/{}/Images/Primary", person.id))?
            .query(&[("maxWidth", max_width.to_string()), ("tag", tag.to_owned())])
            .send()
            .await?
            .error_for_status()?;
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("image/jpeg")
            .to_owned();
        Ok(JellyfinImage {
            bytes: response.bytes().await?.to_vec(),
            content_type,
        })
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
