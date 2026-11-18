# Release signing (macOS)

This document is for maintainers. It lists the GitHub secrets that
`.github/workflows/release.yml` uses to sign and notarize the macOS `.dmg` files, how to
obtain each one, and how to cut a test release.

Windows and Linux builds need no secrets: Windows ships unsigned in v1 (SmartScreen
shows a warning on first run), and the workflow uploads with the automatic
`GITHUB_TOKEN`.

## How the workflow uses the secrets

- Signing and notarization run only in `release.yml`, on tags matching `v*`. `ci.yml`
  never sees these secrets.
- The Tauri bundler reads the `APPLE_*` variables itself. With `APPLE_CERTIFICATE` and
  `APPLE_CERTIFICATE_PASSWORD` it creates a temporary keychain with a random password,
  imports the certificate, signs the app and the `.dmg`, and deletes the keychain. No
  manual `security import` step and no `KEYCHAIN_PASSWORD` secret are needed.
- With the three App Store Connect secrets, the workflow writes the `.p8` key to a file
  under `$RUNNER_TEMP`, points `APPLE_API_KEY_PATH` at it, and the bundler notarizes and
  staples the app before packing it into the `.dmg`. The file is deleted at the end of
  the job.
- The bundler does not notarize the `.dmg` container itself, so a further step submits
  the `.dmg` with `notarytool`, staples the ticket to it and replaces the asset already
  uploaded to the release. Without it Gatekeeper reports the disk image as
  "Unnotarized Developer ID" even though the app inside is notarized.
- Signing is optional. If no signing secrets are set, the job still succeeds and emits a
  warning: the `.dmg` is ad-hoc signed (no Apple identity) and not notarized, so
  Gatekeeper warns users. If only some secrets of a group are set, the job fails, since
  that is a configuration mistake.
- If the certificate is set but the App Store Connect secrets are not, the app is signed
  but not notarized, and the job emits a warning. Gatekeeper blocks such a build.
- After the build, a verification step checks the signature, that the authority is a
  Developer ID Application certificate, and (when notarized) the stapled ticket on both
  the app and the `.dmg`, and that Gatekeeper accepts both.

## Secrets

| Secret | Required | Content |
| --- | --- | --- |
| `APPLE_CERTIFICATE` | For signing | The Developer ID Application certificate and its private key, exported as `.p12` and base64-encoded on a single line |
| `APPLE_CERTIFICATE_PASSWORD` | For signing | The password chosen when exporting the `.p12` |
| `APPLE_SIGNING_IDENTITY` | Recommended | The full identity name, for example `Developer ID Application: Tegra Software (ABCDE12345)`. When set, the workflow rejects any identity that is not `Developer ID Application`, and the bundler checks that it matches the certificate. When unset, the bundler uses the identity found in the certificate |
| `APPLE_API_ISSUER` | For notarization | The Issuer ID of the App Store Connect API keys (a UUID) |
| `APPLE_API_KEY` | For notarization | The Key ID of the App Store Connect API key (for example `2X9R4HXF34`) |
| `APPLE_API_PRIVATE_KEY` | For notarization | The contents of the downloaded `AuthKey_<KEY_ID>.p8` file, as the raw PEM text (starting with `-----BEGIN PRIVATE KEY-----`). A base64 encoding of the file is also accepted |

`APPLE_API_KEY_PATH` and `KEYCHAIN_PASSWORD` are not secrets: the workflow sets the key
path itself, and the bundler manages its own keychain.

## Step by step

All steps run on a Mac. Creating the certificate requires the Apple Developer
**Account Holder** role.

### 1. Create the Developer ID Application certificate

1. Open **Keychain Access** and choose **Keychain Access > Certificate Assistant >
   Request a Certificate From a Certificate Authority**. Enter your email address and a
   common name, select **Saved to disk**, and save the `.certSigningRequest` file. The
   private key is created in your login keychain.
2. Sign in to [Certificates, Identifiers & Profiles](https://developer.apple.com/account/resources/certificates/list)
   as the Account Holder and click the add button.
3. Choose **Developer ID Application** (not "Apple Development" and not "Apple
   Distribution"). If asked for a profile type, choose the G2 Sub-CA option. Upload the
   CSR and download the resulting `.cer` file.
4. Double-click the `.cer` file to install it in the **login** keychain.
5. Confirm the identity is valid and copy its name:

   ```sh
   security find-identity -v -p codesigning
   ```

   The line to use looks like
   `1) 0123ABCD... "Developer ID Application: Tegra Software (ABCDE12345)"`. The text
   between the quotes is the value of `APPLE_SIGNING_IDENTITY`.

### 2. Export the certificate as .p12 and encode it

1. In **Keychain Access**, open the **login** keychain, **My Certificates** tab, and find
   the "Developer ID Application: ..." entry.
2. Expand it, right-click the private key (or the certificate with its key), and choose
   **Export**. Save as `certificate.p12` and set a strong password. That password is
   `APPLE_CERTIFICATE_PASSWORD`.
3. Encode the file as a single base64 line:

   ```sh
   openssl base64 -A -in certificate.p12 -out certificate-base64.txt
   ```

   The contents of `certificate-base64.txt` are `APPLE_CERTIFICATE`.

### 3. Create the App Store Connect API key

1. Sign in to [App Store Connect > Users and Access](https://appstoreconnect.apple.com/access/users)
   and open the **Integrations** tab, **App Store Connect API**, **Team Keys**. The first
   time, the Account Holder has to request access to the API.
2. Click the add button, give the key a name (for example "PhantomPOS notarization"),
   and select the **Developer** access role, which is the role the Tauri documentation
   specifies for notarization.
3. The **Issuer ID** is shown above the keys table: that is `APPLE_API_ISSUER`.
4. The **Key ID** column of the new key is `APPLE_API_KEY`.
5. Click **Download API Key** to save `AuthKey_<KEY_ID>.p8`. Apple allows this download
   **only once** (the button may only appear after reloading the page). Store the file
   in a password manager; if it is lost, revoke the key and create a new one.

### 4. Register the secrets

Use the [GitHub CLI](https://cli.github.com) from the folder that contains the files.
When `--body` is omitted and stdin is a terminal, `gh` prompts for the value, which keeps
passwords out of the shell history.

```sh
gh secret set APPLE_CERTIFICATE --repo tegrasw/phantompos < certificate-base64.txt
gh secret set APPLE_CERTIFICATE_PASSWORD --repo tegrasw/phantompos
gh secret set APPLE_SIGNING_IDENTITY --repo tegrasw/phantompos \
  --body "Developer ID Application: Tegra Software (ABCDE12345)"
gh secret set APPLE_API_ISSUER --repo tegrasw/phantompos \
  --body "00000000-0000-0000-0000-000000000000"
gh secret set APPLE_API_KEY --repo tegrasw/phantompos --body "2X9R4HXF34"
gh secret set APPLE_API_PRIVATE_KEY --repo tegrasw/phantompos < AuthKey_2X9R4HXF34.p8
```

Replace the example values with the real ones, then check the list:

```sh
gh secret list --repo tegrasw/phantompos
```

Delete `certificate.p12` and `certificate-base64.txt` from disk once the secrets are
registered, or move them to a password manager.

## Cutting a test release

The release workflow runs for any pushed tag that starts with `v`. Tags containing a
hyphen (such as `v0.1.0-test.1`) create a pre-release. Release files take their version
from the app configuration (currently 0.1.0), not from the tag.

```sh
git tag v0.1.0-test.1
git push origin v0.1.0-test.1
gh run watch --repo tegrasw/phantompos
```

The workflow creates a **draft** release for the tag and attaches, from four jobs:

- Windows: `PhantomPOS_0.1.0_x64-setup.exe`
- macOS: `PhantomPOS_0.1.0_aarch64.dmg` and `PhantomPOS_0.1.0_x64.dmg` (tauri-action also
  attaches a `.app.tar.gz` archive of each app)
- Linux: `PhantomPOS_0.1.0_amd64.AppImage` and `PhantomPOS_0.1.0_amd64.deb`

Exact file names come from the Tauri bundler. Check the macOS job summary and warnings
to confirm whether the build was signed and notarized. To check a downloaded build on a
Mac, open the `.dmg`, copy the app to Applications, and run:

```sh
spctl --assess --type execute --verbose=2 /Applications/PhantomPOS.app
xcrun stapler validate /Applications/PhantomPOS.app
```

A notarized build reports `source=Notarized Developer ID`. The downloaded `.dmg` can be
checked the same way:

```sh
spctl --assess --type open --context context:primary-signature --verbose=2 PhantomPOS_0.1.0_aarch64.dmg
xcrun stapler validate PhantomPOS_0.1.0_aarch64.dmg
```

To discard a test release together with its tag:

```sh
gh release delete v0.1.0-test.1 --repo tegrasw/phantompos --cleanup-tag --yes
git tag -d v0.1.0-test.1
```

Pushing a tag that already has a published (non-draft) release fails the workflow;
re-running a tag that has a draft reuses that draft.

## References

- [macOS Code Signing, Tauri documentation](https://v2.tauri.app/distribute/sign/macos/)
- [GitHub pipeline, Tauri documentation](https://v2.tauri.app/distribute/pipelines/github/)
- [tauri-apps/tauri-action](https://github.com/tauri-apps/tauri-action)
