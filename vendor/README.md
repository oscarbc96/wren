# Vendored dependencies

`zed-font-kit` is vendored (copied from the crates.io registry) because its
`freetype-sys` dependency pin conflicts with Servo's transitive graph when
left to resolve from crates.io; patching the version to `0.23` (the code path
is compiled out on macOS) and pointing at this local copy lets the workspace
build. Only `zed-font-kit` is vendored; its LICENSE files are retained.
