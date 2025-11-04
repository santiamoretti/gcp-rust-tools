# Publishing to crates.io

This guide shows how to publish the `gcp-observability-rs` crate to crates.io.

## Prerequisites

1. **crates.io Account**: Create an account at https://crates.io
2. **API Token**: Get your API token from https://crates.io/me
3. **Cargo Login**: Run `cargo login` with your token

## Pre-publication Checklist

- [x] Remove all personal information and project-specific references
- [x] Update crate name to `gcp-observability-rs`
- [x] Add proper license files (MIT and Apache-2.0)
- [x] Clean, generic documentation
- [x] Working example with generic data
- [x] All code compiles without warnings

## Publishing Steps

### 1. Login to crates.io
```bash
cargo login
# Enter your API token when prompted
```

### 2. Final checks
```bash
# Make sure everything compiles
cargo check

# Run tests (if any)
cargo test

# Build documentation locally
cargo doc --open

# Check the package contents
cargo package --list
```

### 3. Dry run
```bash
cargo publish --dry-run
```

### 4. Publish
```bash
cargo publish
```

## Post-publication

After successful publication:

1. **Update Documentation**: The docs will be automatically built at https://docs.rs/gcp-observability-rs
2. **Update Repository**: If you have a Git repository, update the Cargo.toml repository field
3. **Tag Release**: Create a git tag for the version
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

## Usage After Publication

Once published, users can add it to their projects:

```toml
[dependencies]
gcp-observability-rs = "0.1.0"
```

## Version Updates

For future versions:

1. Update version in `Cargo.toml`
2. Update changelog/release notes
3. Test thoroughly
4. `cargo publish`

## Repository Setup (Optional)

If you want to create a public repository:

```bash
# Create new repository on GitHub/GitLab
git init
git add .
git commit -m "Initial commit: gcp-observability-rs v0.1.0"
git remote add origin https://github.com/yourusername/gcp-observability-rs.git
git push -u origin main
```

Then update the `repository` field in `Cargo.toml` to point to your repo.

## Important Notes

- **Name Availability**: The name `gcp-observability-rs` must be available on crates.io
- **No Takeback**: Once published, you cannot delete versions from crates.io
- **Semver**: Follow semantic versioning for updates
- **Breaking Changes**: Only introduce breaking changes in major version updates