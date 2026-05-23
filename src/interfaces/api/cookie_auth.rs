use axum::http::{
    header::{COOKIE, SET_COOKIE},
    HeaderMap, HeaderValue,
};
use time::{Duration, OffsetDateTime};

pub const ACCESS_TOKEN_COOKIE: &str = "oxicloud_access_token";
pub const REFRESH_TOKEN_COOKIE: &str = "oxicloud_refresh_token";
pub const CSRF_TOKEN_COOKIE: &str = "oxicloud_csrf_token";

pub const ACCESS_TOKEN_COOKIE_NAME: &str = ACCESS_TOKEN_COOKIE;
pub const REFRESH_TOKEN_COOKIE_NAME: &str = REFRESH_TOKEN_COOKIE;
pub const CSRF_TOKEN_COOKIE_NAME: &str = CSRF_TOKEN_COOKIE;

const COOKIE_PATH: &str = "/";
const SAME_SITE: &str = "Strict";

#[derive(Debug, Clone, Copy)]
pub struct CookieOptions {
    pub secure: bool,
    pub http_only: bool,
    pub max_age: Option<Duration>,
}

impl Default for CookieOptions {
    fn default() -> Self {
        Self {
            secure: true,
            http_only: true,
            max_age: None,
        }
    }
}

pub fn get_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookie_header = headers.get(COOKIE)?.to_str().ok()?;

    cookie_header
        .split(';')
        .filter_map(|cookie| {
            let mut parts = cookie.trim().splitn(2, '=');
            let cookie_name = parts.next()?.trim();
            let cookie_value = parts.next()?.trim();

            if cookie_name == name {
                Some(cookie_value.to_owned())
            } else {
                None
            }
        })
        .next()
}

pub fn get_access_token(headers: &HeaderMap) -> Option<String> {
    get_cookie(headers, ACCESS_TOKEN_COOKIE)
}

pub fn get_refresh_token(headers: &HeaderMap) -> Option<String> {
    get_cookie(headers, REFRESH_TOKEN_COOKIE)
}

pub fn get_csrf_token(headers: &HeaderMap) -> Option<String> {
    get_cookie(headers, CSRF_TOKEN_COOKIE)
}

pub fn extract_access_token(headers: &HeaderMap) -> Option<String> {
    get_access_token(headers)
}

pub fn extract_refresh_token(headers: &HeaderMap) -> Option<String> {
    get_refresh_token(headers)
}

pub fn extract_csrf_token(headers: &HeaderMap) -> Option<String> {
    get_csrf_token(headers)
}

pub fn access_token_from_headers(headers: &HeaderMap) -> Option<String> {
    get_access_token(headers)
}

pub fn refresh_token_from_headers(headers: &HeaderMap) -> Option<String> {
    get_refresh_token(headers)
}

pub fn csrf_token_from_headers(headers: &HeaderMap) -> Option<String> {
    get_csrf_token(headers)
}

pub fn build_cookie(name: &str, value: &str, options: CookieOptions) -> String {
    let mut cookie = format!(
        "{}={}; Path={}; SameSite={}",
        name,
        encode_cookie_value(value),
        COOKIE_PATH,
        SAME_SITE
    );

    if options.http_only {
        cookie.push_str("; HttpOnly");
    }

    if options.secure {
        cookie.push_str("; Secure");
    }

    if let Some(max_age) = options.max_age {
        cookie.push_str(&format!("; Max-Age={}", max_age.whole_seconds()));
    }

    cookie
}

pub fn build_access_token_cookie(token: &str, secure: bool, max_age: Option<Duration>) -> String {
    build_cookie(
        ACCESS_TOKEN_COOKIE,
        token,
        CookieOptions {
            secure,
            http_only: true,
            max_age,
        },
    )
}

pub fn build_refresh_token_cookie(token: &str, secure: bool, max_age: Option<Duration>) -> String {
    build_cookie(
        REFRESH_TOKEN_COOKIE,
        token,
        CookieOptions {
            secure,
            http_only: true,
            max_age,
        },
    )
}

pub fn build_csrf_token_cookie(token: &str, secure: bool, max_age: Option<Duration>) -> String {
    build_cookie(
        CSRF_TOKEN_COOKIE,
        token,
        CookieOptions {
            secure,
            http_only: false,
            max_age,
        },
    )
}

pub fn create_access_token_cookie(token: &str, secure: bool, max_age: Option<Duration>) -> String {
    build_access_token_cookie(token, secure, max_age)
}

pub fn create_refresh_token_cookie(token: &str, secure: bool, max_age: Option<Duration>) -> String {
    build_refresh_token_cookie(token, secure, max_age)
}

pub fn create_csrf_token_cookie(token: &str, secure: bool, max_age: Option<Duration>) -> String {
    build_csrf_token_cookie(token, secure, max_age)
}

pub fn append_cookie(headers: &mut HeaderMap, cookie: String) {
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        headers.append(SET_COOKIE, value);
    }
}

pub fn set_access_token_cookie(
    headers: &mut HeaderMap,
    token: &str,
    secure: bool,
    max_age: Option<Duration>,
) {
    append_cookie(headers, build_access_token_cookie(token, secure, max_age));
}

pub fn set_refresh_token_cookie(
    headers: &mut HeaderMap,
    token: &str,
    secure: bool,
    max_age: Option<Duration>,
) {
    append_cookie(headers, build_refresh_token_cookie(token, secure, max_age));
}

pub fn set_csrf_token_cookie(
    headers: &mut HeaderMap,
    token: &str,
    secure: bool,
    max_age: Option<Duration>,
) {
    append_cookie(headers, build_csrf_token_cookie(token, secure, max_age));
}

pub fn set_auth_cookies(
    headers: &mut HeaderMap,
    access_token: &str,
    refresh_token: &str,
    csrf_token: Option<&str>,
    secure: bool,
) {
    set_access_token_cookie(headers, access_token, secure, None);
    set_refresh_token_cookie(headers, refresh_token, secure, None);

    if let Some(csrf_token) = csrf_token {
        set_csrf_token_cookie(headers, csrf_token, secure, None);
    }
}

pub fn expire_cookie(name: &str, secure: bool, http_only: bool) -> String {
    let expires = OffsetDateTime::UNIX_EPOCH;

    let mut cookie = format!(
        "{}=; Path={}; SameSite={}; Max-Age=0; Expires={}",
        name,
        COOKIE_PATH,
        SAME_SITE,
        expires
            .format(&time::format_description::well_known::Rfc2822)
            .unwrap_or_else(|_| "Thu, 01 Jan 1970 00:00:00 GMT".to_owned())
    );

    if http_only {
        cookie.push_str("; HttpOnly");
    }

    if secure {
        cookie.push_str("; Secure");
    }

    cookie
}

pub fn clear_auth_cookies(headers: &mut HeaderMap, secure: bool) {
    append_cookie(
        headers,
        expire_cookie(ACCESS_TOKEN_COOKIE, secure, true),
    );
    append_cookie(
        headers,
        expire_cookie(REFRESH_TOKEN_COOKIE, secure, true),
    );
    append_cookie(
        headers,
        expire_cookie(CSRF_TOKEN_COOKIE, secure, false),
    );
}

fn encode_cookie_value(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            ';' => "%3B".chars().collect::<Vec<_>>(),
            ',' => "%2C".chars().collect::<Vec<_>>(),
            ' ' => "%20".chars().collect::<Vec<_>>(),
            '"' => "%22".chars().collect::<Vec<_>>(),
            '\\' => "%5C".chars().collect::<Vec<_>>(),
            _ => vec![ch],
        })
        .collect()
}
