# Signing packages

Signatures prove that a `.blue` archive came from a publisher the person (or
their distribution) trusts and was not altered. They are **ed25519**.

## What is signed

```
blue-package-v1\n<kind>\n<id>\n<version>\n<sha256 of the .blue archive, lowercase hex>
```

Because the archive hash is inside the message, changing the archive, its id,
its version or its kind invalidates the signature. The shell checks the hash of
the downloaded bytes before it checks the signature.

## Publisher side

```bash
node scripts/blue-package-sign.mjs keygen acme
# → acme.key (private, 0600) and a ready-to-paste trusted-publishers entry
node scripts/blue-package-sign.mjs sign --key acme.key --kind app --id com.acme.counter \
     --version 1.0.0 --sha256 <hash> --signer acme
# → two lines for [package]: signer + signature
```

## Consumer side

Trusted keys are read from `/usr/share/blue-environment/trusted-publishers.json`
(shipped by the distribution) and `~/.config/Blue-Environment/store/trusted-publishers.json`:

```json
[{ "id": "acme", "name": "ACME Ltd", "publicKey": "<base64 of the 32-byte key>" }]
```

Policy in `~/.config/Blue-Environment/store/policy.json`:

```json
{ "signaturePolicy": "warn" }      // default; "require" accepts only trusted signatures
```

| status | `warn` | `require` |
|---|---|---|
| trusted signature | installs, dialog shows "Signed by …" | installs |
| unsigned | installs, dialog warns | refused (`signature_required`) |
| signed by an unknown key | installs, dialog warns | refused (`signature_untrusted`) |
| invalid / forged / malformed | **refused** (`signature_invalid`) | **refused** |

A forged signature is never installed, even under `warn`.
