# Platform support

| Platform | Status | Notes |
|---|---|---|
| Linux arm64 | **Tested** (Ubuntu 24.04, kernel 6.1, RK3588) | App runs, installs work, Minecraft 26.3 launches natively. |
| Linux x64 | Expected to work | Same code paths as arm64 minus the LWJGL swap. Not run yet. |
| Windows 10/11 x64, arm64 | Expected to work | Not run yet. Compose Multiplatform supports Windows 10+. |
| macOS (Apple Silicon, Intel) | Expected to work | Not run yet. Compose's tested baseline is macOS 13; Big Sur (11) is unverified. |
| Windows 7 / 8.1 | Not supported | Compose Multiplatform and current JDKs require Windows 10. |

## Runtime requirements

- The app bundles its own Java runtime when packaged (`jpackage`).
- **Minecraft** needs a Java matching the version: 8 for ≤1.16, 16 for 1.17,
  17 for 1.18–1.20.4, 21 for 1.20.5–1.21.x, 25 for 26.x. Sulfur picks the best
  installed runtime automatically (exact major version first, native
  architecture preferred). Managed Java downloads are planned.

## Linux arm64

Mojang only ships x86-64 LWJGL natives for Linux. For LWJGL 3 libraries
(Minecraft 1.19+), Sulfur substitutes LWJGL's official
`natives-linux-arm64` artifacts from Maven Central and verifies them against
the published `.sha1`. Older versions that use LWJGL 2 have no arm64 natives
and cannot run natively.

## Graphics

Compose renders with Skia, choosing the best backend (OpenGL, Direct3D, Metal,
software). On machines without a working GL driver it falls back to software
rendering automatically. You may see `Cannot create Linux GL context` in the
console; it is harmless.

Vulkan and native Wayland for the launcher window are not available: Skiko
(Compose's renderer) only has OpenGL and software renderers for Linux windows,
and its AWT integration runs under XWayland. Skia itself has a Vulkan backend
that Skiko exposes (`DirectContext.makeVulkan`), so a custom window host could
add it later; until then Sulfur follows what Compose Multiplatform supports.

## Packaging

`./gradlew :app:packageDistributionForCurrentOS` builds an installer for the
OS it runs on (`.msi`, `.dmg`, `.deb`/`.rpm`/AppImage). Installers must be built
on each target OS; see `.github/workflows/ci.yml` for the build matrix.
