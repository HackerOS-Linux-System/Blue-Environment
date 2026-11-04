# `blue.hk` reference

`blue.hk` is parsed by the shell's own `hk` reader (never JSON). Sections:

```
[package]
-> id          => com.acme.counter
-> name        => Counter
-> type        => app            # app | plugin | theme
-> version     => 1.0.0          # exactly three numeric parts
-> author      => ACME Ltd
-> description => Counts things.
-> icon        => Sparkles       # a Lucide name, or an image inside the archive
-> archive     => com.acme.counter-1.0.0.blue   # filled by blue-dev pack
-> sha256      => <64 hex>                       # filled by blue-dev pack
-> signer      => acme                           # optional, see signing.md
-> signature   => <base64>                       # optional

[app]                           # `[plugin]` for plugins (+ `-> kind`)
-> entry  => index.js           # one ES module
-> style  => style.css          # optional
-> width  => 480
-> height => 640

[permissions]
-> list => ["storage", "notifications"]
```

Validation (all done at install, nothing half-installed on failure):

* `id`: `^[a-z0-9]([a-z0-9._-]{0,62}[a-z0-9])?$`, must not collide with a built-in app.
* `name` ≤ 80 characters; `version` is `x.y.z`.
* `archive` must end in `.blue` and is resolved relative to the `blue.hk` URL.
* `permissions.list` may only contain known permissions ([permissions.md](permissions.md)).
* A package with a `signature` **must** publish `sha256` (the signature covers it).
* Unknown sections and fields are ignored, so newer manifests still install on older shells.
