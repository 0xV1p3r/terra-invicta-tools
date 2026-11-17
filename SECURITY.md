# Security

Report a vulnerability privately through [GitHub's vulnerability reporting](https://github.com/0xV1p3r/terra-invicta-tools/security/advisories/new), not in a public issue.

## Verifying downloads

Every release lists the checksums of its bundles in `SHA256SUMS`. Releases after 0.1.0 also have build provenance attestations. To check that a bundle was built by this repository's release workflow:

```sh
gh attestation verify <downloaded file> --repo 0xV1p3r/terra-invicta-tools
```

The in-app updater only installs bundles signed with the release key (see [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md#updater-signing)).
