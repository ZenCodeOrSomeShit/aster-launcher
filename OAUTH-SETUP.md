# Publisher configuration

Aster uses Microsoft OAuth authorization code + PKCE (SHA-256) in the system browser. The callback listener binds to IPv4 loopback on a dynamically assigned port. Each attempt has a new state and verifier; no client secret is used. Successful authorization proceeds through Xbox Live, XSTS, Minecraft Services, ownership verification, and Windows DPAPI storage.

The publisher must register Aster for personal Microsoft accounts, configure the desktop/mobile `http://localhost` redirect, and obtain any required Minecraft Services access. Registering an application does not itself guarantee access to Minecraft Services. This repository does not contain an approved client ID. A Microsoft developer-portal tenant error cannot be fixed by changing the player's password or fabricating a client ID.

Build configuration in PowerShell:

```powershell
$env:ASTER_MICROSOFT_CLIENT_ID = 'YOUR-ASTER-APPLICATION-CLIENT-ID'
cargo build --release
```

The client ID is a public identifier. Never embed a client secret or borrow another launcher's application identity. Developer builds also have an advanced OAuth configuration field in Settings. A configured published build should require players only to click **Sign in with Microsoft** and complete Microsoft's browser page.

Protocol reference: https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow

Tests cover the RFC 7636 challenge, callback state validation, code decoding, declined authorization, and callback paths. Live Microsoft sign-in cannot be verified until Aster has a valid application registration.
