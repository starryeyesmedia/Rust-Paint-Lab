BitchPixel - unsigned desktop creative studio

Rust Paint Lab remains the repository and internal engineering name.

Extract painting-alpha.tar.gz, then run ./canvas-demo in a terminal.
See README.md for supported features, controls, saving and recovery limitations.
No Rust installation or administrator privileges are required to launch.
BUILD.txt identifies the commit, Rust toolchain and native CPU target.
THIRD-PARTY-NOTICES.txt includes this target's dependency notices.
SYSTEM-LIBRARIES.txt records build-time system library imports.

Linux: requires a desktop X11/Wayland session, graphics drivers and compatible
system libraries/glibc. Built on GitHub's current Ubuntu image; this is not an
AppImage or a distribution-independent static binary. Older distributions may
not satisfy the imported GLIBC version. Native file dialogs use the XDG desktop
portal; a portal service such as xdg-desktop-portal must be available. Mesa software
Vulkan is used by CI.

macOS: native architecture follows BUILD.txt (current macos-latest generally
ARM64); no Intel universal binary, .app bundle, signing or notarization is claimed.
Gatekeeper/organization policy may prevent launching an unsigned download. Metal
requires compatible macOS and hardware. CI compilation/pixel tests are not a
physical drawing UX certification. Do not disable organizational security policy.

Windows remains a separate portable executable ZIP, with static CRT. The original
native Windows stylus diagnostic remains its own artifact. No official release.
