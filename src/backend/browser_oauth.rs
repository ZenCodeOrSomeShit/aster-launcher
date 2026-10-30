use anyhow::{bail, ensure, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::{blocking::Client, Url};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, Instant},
};

const AUTHORITY: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
pub fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}
pub fn callback(target: &str, expected: &str) -> Result<String> {
    let url = Url::parse(&format!("http://localhost{target}"))?;
    ensure!(url.path() == "/", "Unexpected callback path");
    let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    ensure!(
        params.get("state").map(String::as_str) == Some(expected),
        "OAuth state mismatch"
    );
    if params.contains_key("error") {
        bail!("Microsoft sign-in was canceled or declined");
    }
    params
        .get("code")
        .filter(|s| !s.is_empty())
        .cloned()
        .context("Microsoft authorization code missing")
}
pub fn login(client: &Client, client_id: &str) -> Result<Value> {
    ensure!(!client_id.is_empty(),"Microsoft sign-in is not configured in this Aster build. The launcher publisher must supply Aster's registered Microsoft application ID and Minecraft API access. This is a launcher setup issue, not an account error.");
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .context("Could not start the local Microsoft sign-in callback")?;
    listener.set_nonblocking(true)?;
    let redirect = format!("http://localhost:{}/", listener.local_addr()?.port());
    let verifier = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let state = uuid::Uuid::new_v4().simple().to_string();
    let hash = challenge(&verifier);
    let mut url = Url::parse(&format!("{AUTHORITY}/authorize"))?;
    url.query_pairs_mut().extend_pairs([
        ("client_id", client_id),
        ("response_type", "code"),
        ("redirect_uri", redirect.as_str()),
        ("scope", "XboxLive.signin offline_access"),
        ("state", state.as_str()),
        ("code_challenge", hash.as_str()),
        ("code_challenge_method", "S256"),
        ("response_mode", "query"),
        ("prompt", "select_account"),
    ]);
    open::that(url.as_str()).context("Could not open Microsoft sign-in in your browser")?;
    let deadline = Instant::now() + Duration::from_secs(300);
    while Instant::now() < deadline {
        let (mut stream, _) = match listener.accept() {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(80));
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        let mut bytes = [0u8; 8192];
        let mut used = 0;
        while used < bytes.len() {
            match stream.read(&mut bytes[used..]) {
                Ok(0) => break,
                Ok(n) => {
                    used += n;
                    if bytes[..used].windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let request = String::from_utf8_lossy(&bytes[..used]);
        let mut first = request.lines().next().unwrap_or("").split_whitespace();
        if first.next() != Some("GET") {
            continue;
        }
        let target = first.next().unwrap_or("");
        let url = Url::parse(&format!("http://localhost{target}"));
        let valid_state = url.as_ref().ok().is_some_and(|u| {
            u.path() == "/" && u.query_pairs().any(|(k, v)| k == "state" && v == state)
        });
        if !valid_state {
            let _ = stream.write_all(
                b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
            continue;
        }
        let result = callback(target, &state);
        let message = if result.is_ok() {
            "Microsoft sign-in received. Return to Aster to finish connecting your Minecraft account."
        } else {
            "Sign-in was canceled. Return to Aster."
        };
        let response=format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",message.len(),message);
        let _ = stream.write_all(response.as_bytes());
        let code = result?;
        let response = client
            .post(format!("{AUTHORITY}/token"))
            .form(&[
                ("client_id", client_id),
                ("grant_type", "authorization_code"),
                ("code", code.as_str()),
                ("redirect_uri", redirect.as_str()),
                ("code_verifier", verifier.as_str()),
                ("scope", "XboxLive.signin offline_access"),
            ])
            .send()?;
        ensure!(response.status().is_success(),"Microsoft could not complete OAuth sign-in. The launcher application must allow personal Microsoft accounts and the http://localhost desktop redirect URI.");
        return Ok(response.json()?);
    }
    bail!("Microsoft sign-in timed out. Click Connect account to try again.")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pkce_matches_rfc7636() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
    #[test]
    fn callback_checks_state_and_decodes_code() {
        assert_eq!(
            callback("/?code=a%2Bb&state=expected", "expected").unwrap(),
            "a+b"
        );
        assert!(callback("/?code=stolen&state=wrong", "expected").is_err());
        assert!(callback("/?error=access_denied&state=expected", "expected").is_err());
        assert!(callback("/other?code=x&state=expected", "expected").is_err());
    }
}
