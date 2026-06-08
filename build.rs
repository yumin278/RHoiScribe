use std::{
    env, fs,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
    process::Command,
};

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=resources/RHoiScribe.ico");

    if env::var("CARGO_CFG_WINDOWS").is_err() {
        return Ok(());
    }

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("out dir"));
    let icon_path = manifest_dir.join("resources").join("RHoiScribe.ico");
    let rc_path = out_dir.join("rhoiscribe.rc");
    let res_path = out_dir.join("rhoiscribe.res");

    fs::write(&rc_path, windows_resource_script(&icon_path))?;

    compile_resource(&rc_path, &res_path)?;

    println!("cargo:rustc-link-arg-bin=rhoiscribe={}", res_path.display());

    Ok(())
}

fn compile_resource(rc_path: &Path, res_path: &Path) -> io::Result<()> {
    let mut attempted = Vec::new();

    for rc in resource_compilers() {
        let output = match Command::new(&rc)
            .arg("/nologo")
            .arg("/C")
            .arg("65001")
            .arg("/fo")
            .arg(res_path)
            .arg(rc_path)
            .output()
        {
            Ok(output) => output,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                attempted.push(format!("{}: not found", rc.display()));
                continue;
            }
            Err(error) => return Err(error),
        };

        if output.status.success() {
            return Ok(());
        }

        attempted.push(format!(
            "{}: stdout:\n{}\nstderr:\n{}",
            rc.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    panic!(
        "failed to compile Windows resources; install llvm-rc or Windows SDK rc.exe, or set RC\n{}",
        attempted.join("\n")
    );
}

fn resource_compilers() -> Vec<PathBuf> {
    if let Some(rc) = env::var_os("RC") {
        vec![PathBuf::from(rc)]
    } else {
        vec![PathBuf::from("llvm-rc"), PathBuf::from("rc")]
    }
}

fn windows_resource_script(icon_path: &Path) -> String {
    let version = env!("CARGO_PKG_VERSION");
    let version_tuple = version_tuple(version);
    let version_string = format!("{}.0", version);
    let icon = rc_string(&icon_path.display().to_string());

    format!(
        r#"1 ICON "{icon}"

1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904B0"
        BEGIN
            VALUE "CompanyName", "CzXieDdan\0"
            VALUE "FileDescription", "RHoiScribe MCP Server\0"
            VALUE "FileVersion", "{version_string}\0"
            VALUE "InternalName", "rhoiscribe\0"
            VALUE "LegalCopyright", "Copyright © 2026 CzXieDdan\0"
            VALUE "OriginalFilename", "rhoiscribe.exe\0"
            VALUE "ProductName", "RHoiScribe\0"
            VALUE "ProductVersion", "{version_string}\0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x0409, 1200
    END
END
"#,
        icon = icon,
        major = version_tuple.0,
        minor = version_tuple.1,
        patch = version_tuple.2,
        version_string = version_string,
    )
}

fn version_tuple(version: &str) -> (u16, u16, u16) {
    let mut parts = version.split('.').map(|part| part.parse().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

fn rc_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
