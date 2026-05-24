// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::{self, File};
use std::io::Write;

use anyhow::{Context, Result, bail};
use jiff::Zoned;

use super::flags::UpdateVersion;

pub(super) fn update_version(flags: UpdateVersion) -> Result<()> {
    let version = match flags.version {
        Some(version) => version,
        None => workspace_package_version()?,
    };
    let (yy, mm, rv, bn) = parse_product_version(&version, flags.build)?;

    let now = Zoned::now();
    let year = now.year();
    let month = now.month();
    let day = now.day();
    let mut version_rc = File::create("tip/rc/version.rc")?;
    indoc::writedoc!(
        version_rc,
        r#"
            #define VER_FILEVERSION             {yy},{mm},{rv},{bn}
            #define VER_FILEVERSION_STR         "{yy}.{mm}.{rv}.{bn}\0"
            #define VER_PRODUCTVERSION          {yy},{mm},{rv},{bn}
            #define VER_PRODUCTVERSION_STR      "{yy}.{mm}.{rv}.{bn}\0"
            #define ABOUT_CAPTION_WITH_VER      "關於精靈語輸入法 ({yy}.{mm}.{rv}.{bn})\0"
            #define ABOUT_VERSION_STR           "版本：{yy}.{mm}.{rv}.{bn}\0"
            #define ABOUT_RELEASE_DATE_STR      "發行日期：{year} 年 {month:02} 月 {day:02} 日\0"
            #define PREFS_TITLE_WITH_VER        "設定精靈語輸入法 ({yy}.{mm}.{rv}.{bn})\0"
        "#
    )?;
    let mut version_json = File::create("installer/version.json")?;
    indoc::writedoc!(
        version_json,
        r#"
            {{
                "product_version": "{yy}.{mm}.{rv}.{bn}",
                "build_date": "{year} 年 {month:02} 月 {day:02} 日"
            }}
        "#
    )?;

    let mut version_wxi = File::create("installer/version.wxi")?;
    indoc::writedoc!(
        version_wxi,
        r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <Include>
                <?define Version = "{yy}.{mm}.{rv}.{bn}"?>
            </Include>
        "#
    )?;
    Ok(())
}

fn workspace_package_version() -> Result<String> {
    let manifest = fs::read_to_string("Cargo.toml").context("failed to read Cargo.toml")?;
    let mut in_workspace_package = false;
    for raw_line in manifest.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') {
            in_workspace_package = line == "[workspace.package]";
            continue;
        }
        if !in_workspace_package || !line.starts_with("version") {
            continue;
        }
        let (_, value) = line
            .split_once('=')
            .context("malformed [workspace.package].version entry")?;
        let version = value.trim().trim_matches('"');
        if version.is_empty() {
            bail!("[workspace.package].version is empty");
        }
        return Ok(version.to_owned());
    }
    bail!("missing [workspace.package].version in Cargo.toml")
}

fn parse_product_version(
    version: &str,
    build_override: Option<u32>,
) -> Result<(u32, u32, u32, u32)> {
    let version = version.trim().trim_start_matches('v');
    let parts = version.split('.').collect::<Vec<_>>();
    if !matches!(parts.len(), 3 | 4) {
        bail!("product version must be MAJOR.MINOR.PATCH or MAJOR.MINOR.PATCH.BUILD");
    }

    let major = parse_version_part(parts[0], "major")?;
    let minor = parse_version_part(parts[1], "minor")?;
    let patch = parse_version_part(parts[2], "patch")?;
    let build = match (parts.get(3), build_override) {
        (_, Some(build)) => build,
        (Some(build), None) => parse_version_part(build, "build")?,
        (None, None) => 0,
    };

    Ok((major, minor, patch, build))
}

fn parse_version_part(part: &str, name: &str) -> Result<u32> {
    part.parse::<u32>()
        .with_context(|| format!("invalid {name} version component: {part}"))
}
