#![allow(dead_code)]

use std::{fs, path::Path};

use base64::{engine::general_purpose::STANDARD, Engine};

pub struct ArtworkFixture {
    pub jav_code: &'static str,
    pub extension: &'static str,
    pub bytes: Vec<u8>,
    pub valid: bool,
    pub content_type: &'static str,
}

pub fn artwork_fixtures() -> Vec<ArtworkFixture> {
    vec![
        ArtworkFixture {
            jav_code: "JPG-101",
            extension: "jpg",
            bytes: STANDARD.decode("/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRofHh0aHBwgJC4nICIsIxwcKDcpLDAxNDQ0Hyc5PTgyPC4zNDL/2wBDAQkJCQwLDBgNDRgyIRwhMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjL/wAARCAABAAEDASIAAhEBAxEB/8QAHwAAAQUBAQEBAQEAAAAAAAAAAAECAwQFBgcICQoL/8QAtRAAAgEDAwIEAwUFBAQAAAF9AQIDAAQRBRIhMUEGE1FhByJxFDKBkaEII0KxwRVS0fAkM2JyggkKFhcYGRolJicoKSo0NTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXGx8jJytLT1NXW19jZ2uHi4+Tl5ufo6erx8vP09fb3+Pn6/8QAHwEAAwEBAQEBAQEBAQAAAAAAAAECAwQFBgcICQoL/8QAtREAAgECBAQDBAcFBAQAAQJ3AAECAxEEBSExBhJBUQdhcRMiMoEIFEKRobHBCSMzUvAVYnLRChYkNOEl8RcYGRomJygpKjU2Nzg5OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6goOEhYaHiImKkpOUlZaXmJmaoqOkpaanqKmqsrO0tba3uLm6wsPExcbHyMnK0tPU1dbX2Nna4uPk5ebn6Onq8vP09fb3+Pn6/9oADAMBAAIRAxEAPwD3+iiigD//2Q==").unwrap(),
            valid: true,
            content_type: "image/jpeg",
        },
        ArtworkFixture {
            jav_code: "PNG-102",
            extension: "png",
            bytes: STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=").unwrap(),
            valid: true,
            content_type: "image/png",
        },
        ArtworkFixture {
            jav_code: "WEBP-103",
            extension: "webp",
            bytes: STANDARD.decode("UklGRioAAABXRUJQVlA4IB4AAABwAQCdASoBAAEAAgA0JZwCdAGIQAD+9jxuTwUAAAA=").unwrap(),
            valid: true,
            content_type: "image/webp",
        },
        ArtworkFixture {
            jav_code: "ZERO-104",
            extension: "jpg",
            bytes: Vec::new(),
            valid: false,
            content_type: "image/jpeg",
        },
        ArtworkFixture {
            jav_code: "TRUNC-105",
            extension: "png",
            bytes: b"\x89PNG\r\n\x1a\n\0\0\0\r".to_vec(),
            valid: false,
            content_type: "image/png",
        },
        ArtworkFixture {
            jav_code: "DASS-591",
            extension: "jpg",
            bytes: b"<html>not an image</html>".to_vec(),
            valid: false,
            content_type: "image/jpeg",
        },
    ]
}

pub fn valid_jpeg() -> Vec<u8> {
    artwork_fixtures().remove(0).bytes
}

#[allow(dead_code)]
pub fn valid_png() -> Vec<u8> {
    artwork_fixtures().remove(1).bytes
}

#[allow(dead_code)]
pub fn valid_webp() -> Vec<u8> {
    artwork_fixtures().remove(2).bytes
}

#[allow(dead_code)]
pub fn oversized_lossy_webp(width: u16, height: u16) -> Vec<u8> {
    assert!((1..=0x3fff).contains(&width));
    assert!((1..=0x3fff).contains(&height));
    let mut bytes = valid_webp();
    bytes[26..28].copy_from_slice(&width.to_le_bytes());
    bytes[28..30].copy_from_slice(&height.to_le_bytes());
    bytes
}

#[allow(dead_code)]
pub fn oversized_alpha_webp(width: u16, height: u16) -> Vec<u8> {
    assert!((1..=0x3fff).contains(&width));
    assert!((1..=0x3fff).contains(&height));
    let mut bytes = STANDARD
        .decode("UklGRhoAAABXRUJQVlA4TA0AAAAvAAAAEAcQERGIiP4HAA==")
        .unwrap();
    let header = u32::from(width - 1) | (u32::from(height - 1) << 14) | (1 << 28);
    bytes[21..25].copy_from_slice(&header.to_le_bytes());
    bytes
}

#[allow(dead_code)]
pub fn animated_webp() -> Vec<u8> {
    STANDARD
        .decode("UklGRoQAAABXRUJQVlA4WAoAAAACAAAAAAAAAAAAQU5JTQYAAAD/////AABBTk1GKAAAAAAAAAAAAAAAAAAAAGQAAABWUDhMDwAAAC8AAAAABxD9j/4HIqL/AQBBTk1GKAAAAAAAAAAAAAAAAAAAAGQAAABWUDhMDwAAAC8AAAAABxDR//4HIqL/AQA=")
        .unwrap()
}

pub fn write_artwork_fixtures(root: &Path) -> Vec<ArtworkFixture> {
    let fixtures = artwork_fixtures();
    for fixture in &fixtures {
        fs::write(root.join(format!("{}.mp4", fixture.jav_code)), b"video").unwrap();
        fs::write(
            root.join(format!("{}.nfo", fixture.jav_code)),
            format!("<movie><title>{}</title></movie>", fixture.jav_code),
        )
        .unwrap();
        fs::write(
            root.join(format!("{}.{}", fixture.jav_code, fixture.extension)),
            &fixture.bytes,
        )
        .unwrap();
    }
    fixtures
}
