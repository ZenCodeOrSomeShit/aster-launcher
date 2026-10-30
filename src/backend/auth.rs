use super::{Account, BackendEvent, ProgressEvent};
use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc::Sender,
    time::{SystemTime, UNIX_EPOCH},
};

const OAUTH: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
#[derive(Clone, Serialize, Deserialize)]
struct Session {
    account: Account,
    access_token: String,
    refresh_token: String,
    expires_at: u64,
    #[serde(default)]
    xuid: String,
}
pub struct Auth {
    file: PathBuf,
    session: Option<Session>,
    client: Client,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl Auth {
    pub fn new(root: &Path, client: Client) -> Self {
        let file = root.join("account.bin");
        let session = fs::read(&file)
            .ok()
            .and_then(|b| unprotect(&b).ok())
            .and_then(|b| serde_json::from_slice::<Session>(&b).ok());
        Self {
            file,
            session,
            client,
        }
    }
    pub fn account(&self) -> Option<Account> {
        self.session.as_ref().map(|s| s.account.clone())
    }
    fn save(&self) -> Result<()> {
        let bytes = protect(&serde_json::to_vec(
            &self.session.as_ref().context("Missing sign-in session")?,
        )?)?;
        let temp = self.file.with_extension("tmp");
        fs::write(&temp, bytes)?;
        fs::rename(temp, &self.file).context("Could not store encrypted sign-in")?;
        Ok(())
    }
    pub fn logout(&mut self) -> Result<()> {
        self.session = None;
        if self.file.exists() {
            fs::remove_file(&self.file).context("Could not remove encrypted sign-in")?;
        }
        Ok(())
    }
    pub fn login(&mut self, client_id: &str, events: &Sender<BackendEvent>) -> Result<()> {
        let client_id = option_env!("ASTER_MICROSOFT_CLIENT_ID")
            .filter(|s| !s.is_empty())
            .unwrap_or(client_id);
        let _ = events.send(BackendEvent::Progress(ProgressEvent {
            profile_id: None,
            stage: "auth".into(),
            message: "Complete Microsoft sign-in in your browser, then return to Aster.".into(),
            percent: None,
            code: None,
        }));
        let tokens = super::browser_oauth::login(&self.client, client_id)?;
        self.session = Some(self.minecraft(&tokens)?);
        self.save()?;
        Ok(())
    }
    pub fn token(&mut self, client_id: &str) -> Result<(Account, String, String)> {
        let client_id = option_env!("ASTER_MICROSOFT_CLIENT_ID")
            .filter(|s| !s.is_empty())
            .unwrap_or(client_id);
        let session = self
            .session
            .as_ref()
            .context("Sign in with a Microsoft account that owns Minecraft Java Edition.")?;
        if session.expires_at < now() + 60 {
            if client_id.is_empty() {
                bail!("Add your Microsoft application client ID in Settings to refresh sign-in.");
            }
            let response = self
                .client
                .post(format!("{OAUTH}/token"))
                .form(&[
                    ("client_id", client_id),
                    ("grant_type", "refresh_token"),
                    ("refresh_token", session.refresh_token.as_str()),
                    ("scope", "XboxLive.signin offline_access"),
                ])
                .send()?;
            if !response.status().is_success() {
                bail!("Microsoft sign-in expired. Sign out and sign in again.");
            }
            let mut tokens: Value = response.json()?;
            if tokens["refresh_token"].as_str().is_none() {
                tokens["refresh_token"] = Value::String(session.refresh_token.clone());
            }
            self.session = Some(self.minecraft(&tokens)?);
            self.save()?;
        }
        let session = self.session.as_ref().context("Sign in again")?;
        Ok((
            session.account.clone(),
            session.access_token.clone(),
            session.xuid.clone(),
        ))
    }
    fn checked_json(
        &self,
        response: reqwest::blocking::Response,
        minecraft: bool,
    ) -> Result<Value> {
        let status = response.status();
        let data: Value = response.json().unwrap_or(Value::Null);
        if minecraft && status.as_u16() == 403 {
            bail!("Minecraft Services rejected this application. Its Microsoft client ID may need Minecraft API approval.");
        }
        if !status.is_success() {
            match data["XErr"].as_u64() { Some(2148916233) => bail!("Create an Xbox profile for your Microsoft account before signing in."), Some(2148916238) => bail!("This account needs Microsoft family approval before it can play."), _ => bail!("Account verification failed (HTTP {}). Check that this account owns Minecraft Java.", status.as_u16()) }
        }
        Ok(data)
    }
    fn minecraft(&self, tokens: &Value) -> Result<Session> {
        let oauth = tokens["access_token"]
            .as_str()
            .context("Microsoft access token missing")?;
        let xbox = self.checked_json(self.client.post("https://user.auth.xboxlive.com/user/authenticate").json(&json!({"Properties":{"AuthMethod":"RPS","SiteName":"user.auth.xboxlive.com","RpsTicket":format!("d={oauth}")},"RelyingParty":"http://auth.xboxlive.com","TokenType":"JWT"})).send()?, false)?;
        let xbox_token = xbox["Token"].as_str().context("Xbox token missing")?;
        let xsts = self.checked_json(self.client.post("https://xsts.auth.xboxlive.com/xsts/authorize").json(&json!({"Properties":{"SandboxId":"RETAIL","UserTokens":[xbox_token]},"RelyingParty":"rp://api.minecraftservices.com/","TokenType":"JWT"})).send()?, false)?;
        let uhs = xsts["DisplayClaims"]["xui"][0]["uhs"]
            .as_str()
            .context("Xbox identity missing")?;
        let xsts_token = xsts["Token"]
            .as_str()
            .context("Xbox authorization missing")?;
        let mc = self.checked_json(
            self.client
                .post("https://api.minecraftservices.com/authentication/login_with_xbox")
                .json(&json!({"identityToken":format!("XBL3.0 x={uhs};{xsts_token}")}))
                .send()?,
            true,
        )?;
        let access_token = mc["access_token"]
            .as_str()
            .context("Minecraft access token missing")?
            .to_owned();
        let entitlements = self.checked_json(
            self.client
                .get("https://api.minecraftservices.com/entitlements/mcstore")
                .bearer_auth(&access_token)
                .send()?,
            true,
        )?;
        if entitlements["items"]
            .as_array()
            .map_or(true, |v| v.is_empty())
        {
            bail!("This Microsoft account does not have a Minecraft Java license.");
        }
        let profile = self.checked_json(
            self.client
                .get("https://api.minecraftservices.com/minecraft/profile")
                .bearer_auth(&access_token)
                .send()?,
            true,
        )?;
        let name = profile["name"]
            .as_str()
            .context("This account has no Minecraft Java profile. Activate Java Edition first.")?
            .to_owned();
        let uuid = profile["id"]
            .as_str()
            .context("Minecraft profile identifier missing")?
            .to_owned();
        Ok(Session {
            account: Account { name, uuid },
            access_token,
            refresh_token: tokens["refresh_token"]
                .as_str()
                .context("Microsoft refresh token missing")?
                .to_owned(),
            expires_at: now() + mc["expires_in"].as_u64().unwrap_or(3600),
            xuid: xsts["DisplayClaims"]["xui"][0]["xid"]
                .as_str()
                .unwrap_or_default()
                .into(),
        })
    }
}

#[cfg(windows)]
mod dpapi {
    use anyhow::{bail, Result};
    use std::{ffi::c_void, ptr};
    #[repr(C)]
    struct Blob {
        size: u32,
        data: *mut u8,
    }
    #[link(name = "Crypt32")]
    unsafe extern "system" {
        fn CryptProtectData(
            input: *const Blob,
            description: *const u16,
            entropy: *const Blob,
            reserved: *mut c_void,
            prompt: *const c_void,
            flags: u32,
            output: *mut Blob,
        ) -> i32;
        fn CryptUnprotectData(
            input: *const Blob,
            description: *mut *mut u16,
            entropy: *const Blob,
            reserved: *mut c_void,
            prompt: *const c_void,
            flags: u32,
            output: *mut Blob,
        ) -> i32;
    }
    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn LocalFree(ptr: *mut c_void) -> *mut c_void;
    }
    pub fn transform(bytes: &[u8], encrypt: bool) -> Result<Vec<u8>> {
        let input = Blob {
            size: u32::try_from(bytes.len())?,
            data: bytes.as_ptr() as *mut u8,
        };
        let mut output = Blob {
            size: 0,
            data: ptr::null_mut(),
        };
        // DPAPI binds the secret to this Windows user; UI is explicitly forbidden.
        let ok = unsafe {
            if encrypt {
                CryptProtectData(
                    &input,
                    ptr::null(),
                    ptr::null(),
                    ptr::null_mut(),
                    ptr::null(),
                    1,
                    &mut output,
                )
            } else {
                CryptUnprotectData(
                    &input,
                    ptr::null_mut(),
                    ptr::null(),
                    ptr::null_mut(),
                    ptr::null(),
                    1,
                    &mut output,
                )
            }
        };
        if ok == 0 {
            bail!(
                "Windows could not {} the sign-in credentials.",
                if encrypt { "encrypt" } else { "decrypt" }
            );
        }
        let result =
            unsafe { std::slice::from_raw_parts(output.data, output.size as usize).to_vec() };
        unsafe {
            LocalFree(output.data.cast());
        }
        Ok(result)
    }
}
#[cfg(windows)]
fn protect(b: &[u8]) -> Result<Vec<u8>> {
    dpapi::transform(b, true)
}
#[cfg(windows)]
fn unprotect(b: &[u8]) -> Result<Vec<u8>> {
    dpapi::transform(b, false)
}
#[cfg(not(windows))]
fn protect(_: &[u8]) -> Result<Vec<u8>> {
    bail!("Secure sign-in storage currently requires Windows.")
}
#[cfg(not(windows))]
fn unprotect(_: &[u8]) -> Result<Vec<u8>> {
    bail!("Secure sign-in storage currently requires Windows.")
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn windows_dpapi_round_trip_never_stores_plaintext() {
        let secret = b"aster-test-secret-not-a-real-token";
        let encrypted = super::protect(secret).expect("DPAPI encryption");
        assert!(!encrypted.windows(secret.len()).any(|w| w == secret));
        assert_eq!(
            super::unprotect(&encrypted).expect("DPAPI decryption"),
            secret
        );
        assert!(super::unprotect(b"invalid credential blob").is_err());
    }
}
