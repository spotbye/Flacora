//! Build and package the standalone Flacora GUI.

use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::thread;
use std::time::Duration;

const TARGETS: &[(&str, &str)] = &[
    ("windows-x64", "x86_64-pc-windows-msvc"),
    ("linux-x64", "x86_64-unknown-linux-gnu"),
    ("linux-arm64", "aarch64-unknown-linux-gnu"),
    ("macos-x64", "x86_64-apple-darwin"),
    ("macos-arm64", "aarch64-apple-darwin"),
];

trait Runner {
    fn run(
        &mut self,
        program: &str,
        args: &[OsString],
        cwd: &Path,
        env_overrides: &[(String, String)],
        env_removals: &[String],
    ) -> Result<(), String>;

    fn symlink(&mut self, original: &Path, link: &Path) -> Result<(), String>;
}

struct SystemRunner;

impl Runner for SystemRunner {
    fn run(
        &mut self,
        program: &str,
        args: &[OsString],
        cwd: &Path,
        env_overrides: &[(String, String)],
        env_removals: &[String],
    ) -> Result<(), String> {
        let mut cmd = Command::new(program);
        cmd.args(args).current_dir(cwd);
        for key in env_removals {
            cmd.env_remove(key);
        }
        for (key, value) in env_overrides {
            cmd.env(key, value);
        }
        let status = cmd
            .status()
            .map_err(|err| format!("Failed to start `{program}`: {err}"))?;
        if !status.success() {
            return Err(format!("Command `{program}` exited with {status}"));
        }
        Ok(())
    }

    fn symlink(&mut self, original: &Path, link: &Path) -> Result<(), String> {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(original, link).map_err(|err| {
                format!(
                    "Failed to create symlink {} -> {}: {err}",
                    link.display(),
                    original.display()
                )
            })
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_dir(original, link).map_err(|err| {
                format!(
                    "Failed to create symlink {} -> {}: {err}",
                    link.display(),
                    original.display()
                )
            })
        }
    }
}

struct Cli {
    platform: Option<String>,
    out: Option<PathBuf>,
    skip_build: bool,
}

fn parse_cli<I: IntoIterator<Item = String>>(args: I) -> Result<Cli, String> {
    let mut iter = args.into_iter().peekable();
    if iter.peek().map(String::as_str) == Some("build") {
        iter.next();
    }
    let mut cli = Cli {
        platform: None,
        out: None,
        skip_build: false,
    };
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--platform" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "Missing value for --platform".to_string())?;
                if target_triple(&value).is_none() {
                    return Err(format!("Unsupported --platform value: {value}"));
                }
                cli.platform = Some(value);
            }
            "--out" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "Missing value for --out".to_string())?;
                cli.out = Some(PathBuf::from(value));
            }
            "--skip-build" => cli.skip_build = true,
            "-h" | "--help" => {
                return Err(
                    "Usage: cargo xtask [--platform <platform>] [--out <dir>] [--skip-build]"
                        .to_string(),
                );
            }
            other => return Err(format!("Unknown argument: {other}")),
        }
    }
    Ok(cli)
}

fn target_triple(platform: &str) -> Option<&'static str> {
    TARGETS
        .iter()
        .find_map(|(name, triple)| (*name == platform).then_some(*triple))
}

fn platform_for_triple(triple: &str) -> Option<&'static str> {
    TARGETS
        .iter()
        .find_map(|(name, target)| (*target == triple).then_some(*name))
}

fn host_triple(cwd: &Path) -> Result<String, String> {
    let output = Command::new("rustc")
        .arg("-vV")
        .current_dir(cwd)
        .output()
        .map_err(|err| format!("Failed to run `rustc -vV`: {err}"))?;
    if !output.status.success() {
        return Err(format!("`rustc -vV` exited with {}", output.status));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("host: ").map(str::trim))
        .map(str::to_string)
        .ok_or_else(|| "Could not determine rustc host triple".to_string())
}

fn check_linux_os_release() -> Result<(), String> {
    let text = fs::read_to_string("/etc/os-release")
        .map_err(|err| format!("Could not read /etc/os-release: {err}"))?;
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        if let Some((key, value)) = line.split_once('=') {
            fields.insert(key.trim(), value.trim().trim_matches('"'));
        }
    }
    if fields.get("ID").copied() != Some("ubuntu")
        || fields.get("VERSION_ID").copied() != Some("22.04")
    {
        return Err("Linux releases require Ubuntu 22.04 (glibc 2.35)".to_string());
    }
    Ok(())
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn path_prefixes(path: &Path) -> Vec<String> {
    let resolved = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let native = resolved.to_string_lossy().into_owned();
    let stripped = native.strip_prefix(r"\\?\").unwrap_or(&native).to_string();
    let posix = stripped.replace('\\', "/");
    let mut out = Vec::new();
    for candidate in [native, stripped, posix] {
        if !candidate.is_empty() && !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    out
}

fn release_environment(root: &Path, triple: &str) -> (Vec<(String, String)>, Vec<String>) {
    let mut flags = Vec::new();
    let mut mappings = Vec::new();
    if let Some(home) = home_dir() {
        let cargo_home = env::var_os("CARGO_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".cargo"));
        mappings.push((home, "/build/user"));
        mappings.push((cargo_home, "/build/cargo"));
    } else if let Some(cargo_home) = env::var_os("CARGO_HOME").map(PathBuf::from) {
        mappings.push((cargo_home, "/build/cargo"));
    }
    mappings.push((root.to_path_buf(), "/build/flacora"));

    for (path, replacement) in mappings {
        for prefix in path_prefixes(&path) {
            flags.push("--remap-path-prefix".to_string());
            flags.push(format!("{prefix}={replacement}"));
        }
    }
    if triple.ends_with("windows-msvc") {
        flags.push("-C".to_string());
        flags.push("target-feature=+crt-static".to_string());
    }

    let mut overrides = vec![
        ("CARGO_ENCODED_RUSTFLAGS".to_string(), flags.join("\x1f")),
        ("CARGO_PROFILE_RELEASE_STRIP".to_string(), "symbols".to_string()),
    ];
    if triple.ends_with("apple-darwin") && env::var_os("MACOSX_DEPLOYMENT_TARGET").is_none() {
        overrides.push(("MACOSX_DEPLOYMENT_TARGET".to_string(), "13.0".to_string()));
    }
    (overrides, vec!["RUSTFLAGS".to_string()])
}

fn read_package_version(cargo_toml: &Path) -> Result<String, String> {
    let text = fs::read_to_string(cargo_toml)
        .map_err(|err| format!("Failed to read {}: {err}", cargo_toml.display()))?;
    let mut in_package = false;
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if in_package
            && let Some(rest) = line.strip_prefix("version")
            && let Some(after_eq) = rest.trim_start().strip_prefix('=')
        {
            let value = after_eq.trim().trim_matches('"');
            if !value.is_empty() {
                return Ok(value.to_string());
            }
        }
    }
    Err(format!(
        "Could not find [package] version in {}",
        cargo_toml.display()
    ))
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn info_plist(version: &str, min_macos: &str) -> String {
    let version = xml_escape(version);
    let min_macos = xml_escape(min_macos);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDisplayName</key>
	<string>Flacora</string>
	<key>CFBundleExecutable</key>
	<string>Flacora</string>
	<key>CFBundleIconFile</key>
	<string>Flacora.icns</string>
	<key>CFBundleIdentifier</key>
	<string>app.flacora.desktop</string>
	<key>CFBundleName</key>
	<string>Flacora</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>{version}</string>
	<key>CFBundleVersion</key>
	<string>{version}</string>
	<key>LSMinimumSystemVersion</key>
	<string>{min_macos}</string>
	<key>NSHighResolutionCapable</key>
	<true/>
</dict>
</plist>
"#
    )
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .map_err(|err| format!("Failed to chmod {}: {err}", path.display()))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn os_args(items: &[&str]) -> Vec<OsString> {
    items.iter().map(OsString::from).collect()
}

fn package_gui<R: Runner>(
    binary: &Path,
    assets: &Path,
    output: &Path,
    platform: &str,
    version: &str,
    runner: &mut R,
) -> Result<PathBuf, String> {
    let name = format!("Flacora-{version}-{platform}");
    let destination = if platform.starts_with("windows-") {
        let dest = output.join(format!("{name}.exe"));
        fs::copy(binary, &dest)
            .map_err(|err| format!("Failed to copy {} to {}: {err}", binary.display(), dest.display()))?;
        dest
    } else if platform.starts_with("linux-") {
        let dest = output.join(format!("{name}.AppImage"));
        let tool = env::var("APPIMAGETOOL").unwrap_or_else(|_| "appimagetool".to_string());
        let temporary = tempfile::tempdir().map_err(|err| format!("Temp dir error: {err}"))?;
        let appdir = temporary.path().join("Flacora.AppDir");
        let bin_dir = appdir.join("usr/bin");
        fs::create_dir_all(&bin_dir).map_err(|err| format!("mkdir failed: {err}"))?;
        let app_bin = bin_dir.join("flacora");
        fs::copy(binary, &app_bin).map_err(|err| format!("Failed to copy binary: {err}"))?;
        set_executable(&app_bin)?;
        let desktop = fs::read_to_string(assets.join("flacora.desktop"))
            .map_err(|err| format!("Failed to read flacora.desktop: {err}"))?
            .replace("\r\n", "\n");
        fs::write(appdir.join("flacora.desktop"), desktop)
            .map_err(|err| format!("Failed to write flacora.desktop: {err}"))?;
        fs::copy(assets.join("appicon.png"), appdir.join("flacora.png"))
            .map_err(|err| format!("Failed to copy appicon.png: {err}"))?;
        runner.symlink(Path::new("usr/bin/flacora"), &appdir.join("AppRun"))?;

        let arch = if platform == "linux-arm64" {
            "aarch64"
        } else {
            "x86_64"
        };
        let mut args = vec![OsString::from("--no-appstream")];
        if let Ok(runtime_file) = env::var("APPIMAGETOOL_RUNTIME_FILE")
            && !runtime_file.is_empty()
        {
            args.push(OsString::from("--runtime-file"));
            args.push(OsString::from(runtime_file));
        }
        let pending = temporary.path().join("Flacora.AppImage");
        args.push(appdir.into_os_string());
        args.push(pending.clone().into_os_string());
        let overrides = vec![
            ("ARCH".to_string(), arch.to_string()),
            ("APPIMAGE_EXTRACT_AND_RUN".to_string(), "1".to_string()),
        ];
        runner.run(&tool, &args, assets, &overrides, &[])?;
        fs::copy(&pending, &dest)
            .map_err(|err| format!("Failed to copy AppImage to {}: {err}", dest.display()))?;
        set_executable(&dest)?;
        dest
    } else {
        let dest = output.join(format!("{name}.dmg"));
        let temporary = tempfile::tempdir().map_err(|err| format!("Temp dir error: {err}"))?;
        let staging = temporary.path().join("dmg");
        let app = staging.join("Flacora.app");
        let contents = app.join("Contents");
        let macos_dir = contents.join("MacOS");
        let resources_dir = contents.join("Resources");
        fs::create_dir_all(&macos_dir).map_err(|err| format!("mkdir failed: {err}"))?;
        fs::create_dir_all(&resources_dir).map_err(|err| format!("mkdir failed: {err}"))?;
        let app_bin = macos_dir.join("Flacora");
        fs::copy(binary, &app_bin).map_err(|err| format!("Failed to copy binary: {err}"))?;
        set_executable(&app_bin)?;

        let iconset = temporary.path().join("Flacora.iconset");
        fs::create_dir_all(&iconset).map_err(|err| format!("mkdir iconset failed: {err}"))?;
        let icon_src = assets.join("appicon.png");
        for size in [16u32, 32, 128, 256, 512] {
            for scale in [1u32, 2] {
                let px = (size * scale).to_string();
                let suffix = if scale == 2 { "@2x" } else { "" };
                let out_icon = iconset.join(format!("icon_{size}x{size}{suffix}.png"));
                let args = vec![
                    OsString::from("-z"),
                    OsString::from(&px),
                    OsString::from(&px),
                    icon_src.clone().into_os_string(),
                    OsString::from("--out"),
                    out_icon.into_os_string(),
                ];
                runner.run("sips", &args, assets, &[], &[])?;
            }
        }
        let icns_path = resources_dir.join("Flacora.icns");
        let iconutil_args = vec![
            OsString::from("-c"),
            OsString::from("icns"),
            iconset.into_os_string(),
            OsString::from("-o"),
            icns_path.into_os_string(),
        ];
        runner.run("iconutil", &iconutil_args, assets, &[], &[])?;

        let min_macos =
            env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or_else(|_| "13.0".to_string());
        fs::write(contents.join("Info.plist"), info_plist(version, &min_macos))
            .map_err(|err| format!("Failed to write Info.plist: {err}"))?;

        let mut codesign_args = os_args(&["--force", "--deep", "--sign", "-"]);
        codesign_args.push(app.into_os_string());
        runner.run("codesign", &codesign_args, assets, &[], &[])?;

        runner.symlink(Path::new("/Applications"), &staging.join("Applications"))?;

        let pending = temporary.path().join(format!("{name}.dmg"));
        let hdiutil_args = vec![
            OsString::from("create"),
            OsString::from("-volname"),
            OsString::from("Flacora"),
            OsString::from("-srcfolder"),
            staging.into_os_string(),
            OsString::from("-ov"),
            OsString::from("-format"),
            OsString::from("UDZO"),
            pending.clone().into_os_string(),
        ];
        for attempt in 0..3 {
            match runner.run("hdiutil", &hdiutil_args, assets, &[], &[]) {
                Ok(()) => break,
                Err(err) if attempt == 2 => return Err(err),
                Err(_) => {
                    let _ = fs::remove_file(&pending);
                    thread::sleep(Duration::from_secs(2));
                }
            }
        }
        fs::copy(&pending, &dest)
            .map_err(|err| format!("Failed to copy DMG to {}: {err}", dest.display()))?;
        dest
    };
    println!("{}", destination.display());
    Ok(destination)
}

fn gui_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives inside gui/")
        .to_path_buf()
}

fn run_main() -> Result<(), String> {
    let cli = parse_cli(env::args().skip(1))?;
    let root = gui_root();
    let triple = host_triple(&root)?;
    let platform = platform_for_triple(&triple).ok_or_else(|| {
        format!(
            "Use a native runner for {}; host is {triple}",
            cli.platform.as_deref().unwrap_or(&triple)
        )
    })?;
    if let Some(requested) = &cli.platform
        && requested != platform
    {
        return Err(format!(
            "Use a native runner for {requested}; host is {triple}"
        ));
    }
    if platform.starts_with("linux-") {
        check_linux_os_release()?;
    }

    let target_dir = root.join("target").join(if platform.starts_with("linux-") {
        "release-build-ubuntu-22.04"
    } else {
        "release-build"
    });

    let mut runner = SystemRunner;
    if !cli.skip_build {
        let (overrides, removals) = release_environment(&root, &triple);
        let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        let args = vec![
            OsString::from("build"),
            OsString::from("--release"),
            OsString::from("--locked"),
            OsString::from("-p"),
            OsString::from("flacora"),
            OsString::from("--target"),
            OsString::from(&triple),
            OsString::from("--target-dir"),
            target_dir.clone().into_os_string(),
        ];
        runner.run(&cargo, &args, &root, &overrides, &removals)?;
    }

    let output = match cli.out {
        Some(path) => path,
        None => root.join("dist").join(platform),
    };
    fs::create_dir_all(&output)
        .map_err(|err| format!("Failed to create {}: {err}", output.display()))?;

    let binary_name = if platform.starts_with("windows-") {
        "flacora.exe"
    } else {
        "flacora"
    };
    let binary = target_dir.join(&triple).join("release").join(binary_name);
    let version = read_package_version(&root.join("Cargo.toml"))?;
    package_gui(&binary, &root.join("assets"), &output, platform, &version, &mut runner)?;
    Ok(())
}

fn main() -> ExitCode {
    match run_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error: {err}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockRunner {
        commands: Vec<(String, Vec<String>)>,
        symlinks: Vec<(String, String)>,
        bundle_executable: Option<Vec<u8>>,
        bundle_icns: Option<Vec<u8>>,
        bundle_plist: Option<String>,
        staged_entries: Vec<String>,
    }

    impl MockRunner {
        fn new() -> Self {
            Self {
                commands: Vec::new(),
                symlinks: Vec::new(),
                bundle_executable: None,
                bundle_icns: None,
                bundle_plist: None,
                staged_entries: Vec::new(),
            }
        }
    }

    impl Runner for MockRunner {
        fn run(
            &mut self,
            program: &str,
            args: &[OsString],
            _cwd: &Path,
            _env_overrides: &[(String, String)],
            _env_removals: &[String],
        ) -> Result<(), String> {
            let string_args: Vec<String> = args
                .iter()
                .map(|a| a.to_string_lossy().into_owned())
                .collect();
            match program {
                "iconutil" => {
                    fs::write(Path::new(string_args.last().unwrap()), b"icns").unwrap();
                }
                "codesign" => {
                    let app = Path::new(string_args.last().unwrap());
                    self.bundle_executable =
                        Some(fs::read(app.join("Contents/MacOS/Flacora")).unwrap());
                    self.bundle_icns =
                        Some(fs::read(app.join("Contents/Resources/Flacora.icns")).unwrap());
                    self.bundle_plist =
                        Some(fs::read_to_string(app.join("Contents/Info.plist")).unwrap());
                }
                "hdiutil" => {
                    let idx = string_args
                        .iter()
                        .position(|a| a == "-srcfolder")
                        .unwrap();
                    let staging = Path::new(&string_args[idx + 1]);
                    let mut entries: Vec<String> = fs::read_dir(staging)
                        .unwrap()
                        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                        .collect();
                    entries.sort();
                    self.staged_entries = entries;
                    fs::write(Path::new(string_args.last().unwrap()), b"fake dmg image").unwrap();
                }
                _ => {}
            }
            self.commands.push((program.to_string(), string_args));
            Ok(())
        }

        fn symlink(&mut self, original: &Path, link: &Path) -> Result<(), String> {
            let name = link.file_name().unwrap().to_string_lossy().into_owned();
            let target = original.to_string_lossy().into_owned();
            self.symlinks.push((name, target.clone()));
            fs::write(link, target).unwrap();
            Ok(())
        }
    }

    #[test]
    fn windows_gui_package_keeps_the_built_executable() {
        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("flacora.exe");
        fs::write(&binary, b"test executable content").unwrap();
        let mut runner = MockRunner::new();
        let dest = package_gui(
            &binary,
            temp.path(),
            temp.path(),
            "windows-x64",
            "1.0.3",
            &mut runner,
        )
        .unwrap();
        assert_eq!(dest.file_name().unwrap(), "Flacora-1.0.3-windows-x64.exe");
        assert_eq!(fs::read(&dest).unwrap(), b"test executable content");
    }

    #[test]
    fn macos_gui_package_signs_app_and_builds_dmg() {
        let temp = tempfile::tempdir().unwrap();
        let assets = temp.path().join("assets");
        fs::create_dir(&assets).unwrap();
        fs::write(assets.join("appicon.png"), b"png").unwrap();
        let binary = temp.path().join("flacora");
        fs::write(&binary, b"macos binary").unwrap();
        let out = temp.path().join("dist");
        fs::create_dir(&out).unwrap();

        let mut runner = MockRunner::new();
        let dest = package_gui(&binary, &assets, &out, "macos-arm64", "1.0.3", &mut runner).unwrap();

        assert_eq!(dest.file_name().unwrap(), "Flacora-1.0.3-macos-arm64.dmg");
        assert_eq!(fs::read(&dest).unwrap(), b"fake dmg image");
        assert_eq!(runner.bundle_executable.as_deref(), Some(&b"macos binary"[..]));
        assert_eq!(runner.bundle_icns.as_deref(), Some(&b"icns"[..]));
        let plist = runner.bundle_plist.as_deref().unwrap();
        assert!(plist.contains("<string>1.0.3</string>"));
        assert!(plist.contains("<string>Flacora</string>"));
        assert_eq!(
            runner.symlinks,
            vec![("Applications".to_string(), "/Applications".to_string())]
        );
        assert_eq!(runner.staged_entries, vec!["Applications", "Flacora.app"]);
        let tools: Vec<&str> = runner.commands.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(tools.iter().filter(|&&t| t == "sips").count(), 10);
        assert_eq!(&tools[tools.len() - 3..], &["iconutil", "codesign", "hdiutil"]);
        assert_eq!(
            &runner.commands[runner.commands.len() - 2].1[..4],
            &["--force", "--deep", "--sign", "-"]
        );
        assert!(runner.commands.last().unwrap().1.contains(&"UDZO".to_string()));
    }

    #[test]
    fn reads_package_version_from_gui_cargo_toml() {
        let version = read_package_version(&gui_root().join("Cargo.toml")).unwrap();
        assert!(
            version.split('.').count() == 3
                && version.split('.').all(|part| part.chars().all(|c| c.is_ascii_digit())),
            "unexpected version {version}"
        );
    }
}
