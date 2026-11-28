# Troubleshooting

## macOS

1. Open `Flacora-1.0.3-macos-<arch>.dmg` and drag **Flacora.app** to **Applications**.
2. Release builds are ad-hoc signed and not notarized. If macOS blocks the first launch (for example, “Apple could not verify” or “is damaged”), clear the quarantine attribute and open Flacora again:

```sh
xattr -dr com.apple.quarantine /Applications/Flacora.app
```

You can also allow it under **System Settings → Privacy & Security → Open Anyway**.

## Linux

```sh
chmod +x Flacora-1.0.3-linux-x64.AppImage
./Flacora-1.0.3-linux-x64.AppImage
```

If it does not start, install FUSE (`sudo apt install fuse3`) or run it with
`--appimage-extract-and-run`.

### CloakBrowser sandbox on Ubuntu 23.10+

Ubuntu 23.10+ restricts the unprivileged user namespaces Chromium uses for its
sandbox. Flacora's browser-based extensions automatically retry with `--no-sandbox`
when that restriction is encountered. If you prefer to keep Chromium's user-namespace
sandbox enabled, an administrator can grant permission with an AppArmor profile:

1. Open **Settings → Data Folder** and find `cloakbrowser/chromium-<version>/chrome`.
2. Use `sudoedit /etc/apparmor.d/flacora-cloakbrowser` and replace the example path
   below with the full path to that executable.

   ```text
   abi <abi/4.0>,
   include <tunables/global>

   profile flacora-cloakbrowser "/absolute/path/to/cloakbrowser/chromium-<version>/chrome" flags=(unconfined) {
     userns,
   }
   ```

3. Load it with `sudo apparmor_parser -r /etc/apparmor.d/flacora-cloakbrowser`, then
   retry the job. If a browser update changes the executable path, update and
   reload the profile.

See the [Ubuntu release notes](https://documentation.ubuntu.com/release-notes/24.04/#unprivileged-user-namespace-restrictions).
