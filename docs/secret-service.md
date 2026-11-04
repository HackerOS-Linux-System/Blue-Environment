# Blue Vault as the system keyring

While Blue is running it answers `org.freedesktop.secrets`, so everything that
uses `libsecret` (`secret-tool`, git-credential-libsecret, GNOME apps, Electron's
`safeStorage`, QtKeychain, the Python `keyring` package…) stores its secrets in
the same Argon2id + AES-256-GCM vault as your passwords, behind one master password.

* **Unlocking:** the first request while the vault is locked shows a master-password
  dialog naming the application that asked. A wrong password lets you retry;
  Deny answers the application with "dismissed". (If a lookup finds the vault
  locked, you get an unlock prompt at most every 30 s.)
* **Where entries show up:** in Blue Accounts, tagged "stored by app", never
  mixed into the password-health audit.
* **Auto-lock:** after 15 minutes idle by default (Blue Accounts → Security).
  While unlocked, the master password is held in memory (overwritten on lock) so
  background stores don't need a dialog each time.
* **Audit trail:** Blue Accounts → Security lists which application read, stored
  or deleted which secret (last 100, in memory only).
* **Isolation:** `bwrap` clones get no session bus (or a filtered one), so a
  sandboxed app cannot reach this service.

Limits, on purpose: one collection (`default`/`login`); the `plain` transfer
algorithm only (the session bus is private to your login; clients fall back
automatically); no per-application approval yet — only the audit log. If
gnome-keyring or KWallet already owns the bus name Blue leaves it alone.

Try it: `echo -n s3cret | secret-tool store --label=demo service demo && secret-tool lookup service demo`.
