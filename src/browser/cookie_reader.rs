use super::types::{Cookie, Profile};
use crate::crypto;
use rusqlite::Connection;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CookieReaderError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Decryption error: {0}")]
    DecryptionError(#[from] crypto::CryptoError),

    #[error("No cookies found for domain: {0}")]
    NoCookiesFound(String),
}

pub type Result<T> = std::result::Result<T, CookieReaderError>;

pub struct CookieReader;

impl CookieReader {
    pub fn read_cookies(profile: &Profile, domain: &str) -> Result<Vec<Cookie>> {
        let cookies_path = profile.cookies_path();

        if !cookies_path.exists() {
            return Err(CookieReaderError::NoCookiesFound(format!(
                "Cookies file not found: {:?}",
                cookies_path
            )));
        }

        let temp_dir = std::env::temp_dir();
        let temp_cookies = temp_dir.join(format!("cookies_{}.db", std::process::id()));
        std::fs::copy(&cookies_path, &temp_cookies)?;

        let result = if profile.browser_type.is_chromium_based() {
            Self::read_chromium_cookies(&temp_cookies, domain, profile)
        } else if profile.browser_type == super::types::BrowserType::Firefox {
            Self::read_firefox_cookies(&temp_cookies, domain)
        } else if profile.browser_type == super::types::BrowserType::Safari {
            Self::read_safari_cookies(&cookies_path, domain)
        } else {
            Err(CookieReaderError::NoCookiesFound(format!(
                "Unsupported browser type: {:?}",
                profile.browser_type
            )))
        };

        let _ = std::fs::remove_file(&temp_cookies);

        result
    }

    fn read_chromium_cookies(
        db_path: &Path,
        domain: &str,
        _profile: &Profile,
    ) -> Result<Vec<Cookie>> {
        let conn = Connection::open(db_path)?;

        let mut stmt = conn.prepare(
            "SELECT name, encrypted_value, host_key, path, expires_utc, is_secure, is_httponly, samesite
             FROM cookies
             WHERE host_key LIKE ?1 OR host_key LIKE ?2
             ORDER BY creation_utc DESC",
        )?;

        let domain_pattern = format!("%{}", domain);
        let dot_domain_pattern = format!("%.{}", domain);

        let cookie_iter = stmt.query_map(
            rusqlite::params![&domain_pattern, &dot_domain_pattern],
            |row| {
                let name: String = row.get(0)?;
                let encrypted_value: Vec<u8> = row.get(1)?;
                let host_key: String = row.get(2)?;
                let path: String = row.get(3)?;
                let expires_utc: i64 = row.get(4)?;
                let is_secure: bool = row.get(5)?;
                let is_httponly: bool = row.get(6)?;
                let same_site: i32 = row.get(7)?;

                Ok((
                    name,
                    encrypted_value,
                    host_key,
                    path,
                    expires_utc,
                    is_secure,
                    is_httponly,
                    same_site,
                ))
            },
        )?;

        let mut cookies = Vec::new();

        for cookie_result in cookie_iter {
            let (
                name,
                encrypted_value,
                host_key,
                path,
                expires_utc,
                is_secure,
                is_httponly,
                same_site,
            ) = cookie_result?;

            match crypto::decrypt_cookie_value(&encrypted_value) {
                Ok(value) => {
                    cookies.push(Cookie {
                        name,
                        value,
                        domain: host_key,
                        path,
                        expires_utc,
                        is_secure,
                        is_httponly,
                        same_site,
                    });
                }
                Err(e) => {
                    eprintln!("  [warn] Failed to decrypt cookie '{}': {}", name, e);
                }
            }
        }

        if cookies.is_empty() {
            return Err(CookieReaderError::NoCookiesFound(domain.to_string()));
        }

        Ok(cookies)
    }

    fn read_firefox_cookies(db_path: &Path, domain: &str) -> Result<Vec<Cookie>> {
        let conn = Connection::open(db_path)?;

        let mut stmt = conn.prepare(
            "SELECT name, value, host, path, expiry, isSecure, isHttpOnly, sameSite
             FROM moz_cookies
             WHERE host LIKE ?1 OR host LIKE ?2
             ORDER BY creationTime DESC",
        )?;

        let domain_pattern = format!("%{}", domain);
        let dot_domain_pattern = format!("%.{}", domain);

        let cookie_iter = stmt.query_map(
            rusqlite::params![&domain_pattern, &dot_domain_pattern],
            |row| {
                let name: String = row.get(0)?;
                let value: String = row.get(1)?;
                let host: String = row.get(2)?;
                let path: String = row.get(3)?;
                let expiry: i64 = row.get(4)?;
                let is_secure: i32 = row.get(5)?;
                let is_httponly: i32 = row.get(6)?;
                let same_site: i32 = row.get(7)?;

                Ok((
                    name,
                    value,
                    host,
                    path,
                    expiry,
                    is_secure,
                    is_httponly,
                    same_site,
                ))
            },
        )?;

        let mut cookies = Vec::new();

        for cookie_result in cookie_iter {
            let (name, value, host, path, expiry, is_secure, is_httponly, same_site) =
                cookie_result?;

            let cookie = Cookie {
                name,
                value,
                domain: host,
                path,
                expires_utc: expiry * 1_000_000 + 11_644_473_600_000_000,
                is_secure: is_secure != 0,
                is_httponly: is_httponly != 0,
                same_site,
            };

            cookies.push(cookie);
        }

        if cookies.is_empty() {
            return Err(CookieReaderError::NoCookiesFound(domain.to_string()));
        }

        Ok(cookies)
    }

    fn read_safari_cookies(cookies_path: &Path, domain: &str) -> Result<Vec<Cookie>> {
        let data = std::fs::read(cookies_path)?;

        if data.len() < 8 {
            return Err(CookieReaderError::NoCookiesFound(
                "Invalid Safari cookies file".to_string(),
            ));
        }

        let mut pos = 4;
        let num_pages =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        let mut cookies = Vec::new();

        for _ in 0..num_pages {
            if pos + 4 > data.len() {
                break;
            }

            let page_offset =
                u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                    as usize;
            pos += 4;

            if page_offset >= data.len() {
                continue;
            }

            if let Ok(page_cookies) = Self::parse_safari_page(&data, page_offset, domain) {
                cookies.extend(page_cookies);
            }
        }

        if cookies.is_empty() {
            return Err(CookieReaderError::NoCookiesFound(domain.to_string()));
        }

        Ok(cookies)
    }

    fn parse_safari_page(data: &[u8], offset: usize, domain: &str) -> Result<Vec<Cookie>> {
        let mut pos = offset + 4;

        if pos + 4 > data.len() {
            return Ok(Vec::new());
        }

        let num_cookies =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        let mut cookie_offsets = Vec::new();
        for _ in 0..num_cookies {
            if pos + 4 > data.len() {
                break;
            }
            let cookie_offset =
                u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                    as usize;
            cookie_offsets.push(offset + cookie_offset);
            pos += 4;
        }

        let mut cookies = Vec::new();
        for cookie_offset in cookie_offsets {
            if let Ok(Some(cookie)) = Self::parse_safari_cookie(data, cookie_offset, domain) {
                cookies.push(cookie);
            }
        }

        Ok(cookies)
    }

    fn parse_safari_cookie(
        data: &[u8],
        offset: usize,
        filter_domain: &str,
    ) -> Result<Option<Cookie>> {
        let mut pos = offset;

        if pos + 4 > data.len() {
            return Ok(None);
        }

        let _cookie_size =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;

        if pos + 8 > data.len() {
            return Ok(None);
        }

        pos += 8;

        if pos + 16 > data.len() {
            return Ok(None);
        }

        let flags = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;

        pos += 4;

        let url_offset =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;
        let name_offset =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;
        let path_offset =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;
        let value_offset =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        if pos + 8 > data.len() {
            return Ok(None);
        }

        let expiry_bytes = [
            data[pos],
            data[pos + 1],
            data[pos + 2],
            data[pos + 3],
            data[pos + 4],
            data[pos + 5],
            data[pos + 6],
            data[pos + 7],
        ];
        let expiry = f64::from_le_bytes(expiry_bytes);

        let domain = Self::read_safari_string(data, offset + url_offset)?;
        let name = Self::read_safari_string(data, offset + name_offset)?;
        let path = Self::read_safari_string(data, offset + path_offset)?;
        let value = Self::read_safari_string(data, offset + value_offset)?;

        if !domain.contains(filter_domain) {
            return Ok(None);
        }

        let expires_utc = ((expiry + 978307200.0) * 1_000_000.0) as i64 + 11_644_473_600_000_000;

        Ok(Some(Cookie {
            name,
            value,
            domain,
            path,
            expires_utc,
            is_secure: (flags & 0x1) != 0,
            is_httponly: (flags & 0x4) != 0,
            same_site: 0,
        }))
    }

    fn read_safari_string(data: &[u8], offset: usize) -> Result<String> {
        let mut pos = offset;
        let mut bytes = Vec::new();

        while pos < data.len() && data[pos] != 0 {
            bytes.push(data[pos]);
            pos += 1;
        }

        String::from_utf8(bytes)
            .map_err(|_| CookieReaderError::NoCookiesFound("Invalid UTF-8 in cookie".to_string()))
    }
}
