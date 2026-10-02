# Security

## Supported versions

Only the latest release gets fixes.

## Reporting a vulnerability

Report privately through GitHub: the repository's **Security** tab → **Report a vulnerability**. Don't open a
public issue. Include the `bmr` version, the command, and a minimal input that triggers it (a crafted file is
better than a live URL).

## Scope

`bmr` downloads from web maps it doesn't control and parses PRBM, PNG, NBT, JSON and zip data from them. In scope:

- crashes, hangs or unbounded memory use from malicious map data;
- path traversal or writes outside the output directory;
- anything that lets a map server run code or read files on the machine running `bmr`.

Reconstruction quality problems are bugs, not vulnerabilities; use the issue tracker.
