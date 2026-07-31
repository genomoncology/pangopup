# Linux executable installation

The installer rejects malformed requests before attempting a download.

```bash run id=installer-version-invalid exit=1 stream=stderr
../install.sh --version v0.1.0
```

```text expect=installer-version-invalid exact
pangopup installer: version must be latest or MAJOR.MINOR.PATCH
```

The deterministic maintainer entry point is discoverable without reading any
release input.

```bash
pangopup-build executable-release prepare --help | mustmatch like "Usage: pangopup-build executable-release prepare --executable <FILE> --sbom <CYCLONEDX_JSON> --version <MAJOR.MINOR.PATCH> --target-commit <40_LOWERCASE_HEX> --repository <DIR> --output <ABSENT_DIR>

Prepare the deterministic Linux x86_64 executable release set."
```
