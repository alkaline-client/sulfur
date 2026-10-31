# Microsoft sign-in

Sulfur signs players in with the OAuth **device code** flow: the Launch page
shows a short code, the player enters it at microsoft.com/link in any browser,
and Sulfur finishes the rest (`sulfur_minecraft::msa`):

1. Microsoft identity platform (`consumers` tenant), scope
   `XboxLive.signin offline_access` → Microsoft access and refresh tokens.
2. Xbox Live user token → XSTS token for `rp://api.minecraftservices.com/`.
3. `login_with_xbox` → Minecraft access token (about 24 hours).
4. Minecraft profile → name, UUID and skin. No profile means the account does
   not own Java Edition (bought or through Game Pass).

Accounts are saved in `<config>/accounts.json` (owner-only permissions on
Unix) with their refresh token; before a launch, a Minecraft token that
expires within 10 minutes is renewed without asking the player. Signing out
removes the account and its tokens.

Once a Microsoft account that owns the game is signed in, offline accounts
are allowed in every region (see Accounts and region in
[ARCHITECTURE.md](ARCHITECTURE.md)).

## What a build needs: an application ID

Microsoft sign-in only works with an application (client) ID that is
registered in Microsoft Entra **and approved by Mojang** for the Minecraft
API. The ID is not a secret, but it identifies whoever registered it, so it is
not in the repository; builds without one show sign-in as unavailable.

1. Sign in at <https://entra.microsoft.com> (a free Azure account works) and
   open **Applications → App registrations → New registration**.
   - Name: `Sulfur`.
   - Supported account types: **Personal Microsoft accounts only**.
   - Redirect URI: none needed (the device code flow does not redirect).
2. In the new app, open **Authentication** and turn on **Allow public client
   flows** (required for device codes). Save.
3. Copy the **Application (client) ID** from the Overview page.
4. Ask Mojang to allow the ID for Minecraft sign-in with their review form,
   <https://aka.ms/mce-reviewappid>. Approval can take a while. Until then,
   sign-in fails at step 3 with "Mojang has not approved this build's
   application ID".
5. Build with the ID:

   ```bash
   ./gradlew :app:run -Psulfur.msaClientId=<application id>
   ```

   or set `SULFUR_MSA_CLIENT_ID` when building, or put it in
   `local.properties` at the repository root, which Git ignores:

   ```properties
   sulfur.msaClientId=<application id>
   ```

   The Gradle property wins over the variable, which wins over
   `local.properties`. The same variable at run time overrides the built-in
   ID, which is handy while testing.

## Errors players may see

| Message | Cause |
|---|---|
| no Xbox profile yet | The Microsoft account never signed in to Xbox; signing in once at xbox.com creates the profile. |
| adult verification | Xbox requires age verification for this account (South Korea). |
| child account | The account must be added to a Microsoft family by an adult. |
| does not own Minecraft: Java Edition | No Minecraft profile on the account. |
| no longer valid; sign in again | The refresh token was revoked or expired (e.g. after a password change). |

## Tests

`cargo test -p sulfur-minecraft --test msa` runs the whole chain against a
local stand-in for every service, including refusals and cancellation.
