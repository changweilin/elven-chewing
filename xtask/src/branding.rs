// SPDX-License-Identifier: GPL-3.0-or-later

use std::{fs, path::Path};

use anyhow::{Context, Result, bail};

const PREBUILT_TOOLS: [&str; 1] = ["build/installer/chewing-editor.exe"];

const CHEWING_LINKS: [&str; 5] = [
    "https://chewing.im",
    "https://codeberg.org/chewing",
    "https://github.com/chewing",
    "https://groups.google.com/group/chewing-devel",
    "chewing-devel",
];

pub(crate) fn brand_prebuilt_tools() -> Result<()> {
    let icon_path = Path::new("tip/rc/elven_ime.ico");

    for tool in PREBUILT_TOOLS {
        let exe_path = Path::new(tool);
        if !exe_path.exists() {
            continue;
        }

        replace_brand_strings(exe_path)
            .with_context(|| format!("failed to replace bundled branding in {tool}"))?;
        remove_chewing_links(exe_path)
            .with_context(|| format!("failed to remove bundled chewing links in {tool}"))?;
        replace_exe_icon(exe_path, icon_path)
            .with_context(|| format!("failed to replace icon resources in {tool}"))?;
    }

    Ok(())
}

fn replace_brand_strings(path: &Path) -> Result<()> {
    let mut data = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut replacements = 0;

    replacements += replace_all(&mut data, "新酷音".as_bytes(), "精靈語".as_bytes())?;
    replacements += replace_all(
        &mut data,
        &utf16le_bytes("新酷音"),
        &utf16le_bytes("精靈語"),
    )?;

    if replacements > 0 {
        fs::write(path, data).with_context(|| format!("failed to write {}", path.display()))?;
    }

    Ok(())
}

fn remove_chewing_links(path: &Path) -> Result<()> {
    let mut data = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut replacements = 0;

    for link in CHEWING_LINKS {
        replacements += replace_all(
            &mut data,
            link.as_bytes(),
            &vec![b' '; link.as_bytes().len()],
        )?;
        replacements += replace_all(
            &mut data,
            &utf16le_bytes(link),
            &utf16le_bytes(&" ".repeat(link.chars().count())),
        )?;
    }

    if replacements > 0 {
        fs::write(path, data).with_context(|| format!("failed to write {}", path.display()))?;
    }

    Ok(())
}

fn replace_all(data: &mut [u8], from: &[u8], to: &[u8]) -> Result<usize> {
    if from.len() != to.len() {
        bail!("replacement strings must have the same byte length");
    }
    if from.is_empty() {
        return Ok(0);
    }

    let mut count = 0;
    let mut offset = 0;
    while let Some(pos) = data[offset..]
        .windows(from.len())
        .position(|candidate| candidate == from)
    {
        let start = offset + pos;
        data[start..start + from.len()].copy_from_slice(to);
        count += 1;
        offset = start + from.len();
    }

    Ok(count)
}

fn utf16le_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>()
}

#[cfg(windows)]
fn replace_exe_icon(exe_path: &Path, icon_path: &Path) -> Result<()> {
    windows_icon::replace_exe_icon(exe_path, icon_path)
}

#[cfg(not(windows))]
fn replace_exe_icon(_exe_path: &Path, _icon_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(windows)]
mod windows_icon {
    use std::{iter, os::windows::ffi::OsStrExt, path::Path};

    use anyhow::{Context, Result, bail};
    use windows::{
        Win32::{
            Foundation::{FreeLibrary, HMODULE},
            System::LibraryLoader::{
                BeginUpdateResourceW, EndUpdateResourceW, EnumResourceLanguagesW,
                EnumResourceNamesW, LOAD_LIBRARY_AS_DATAFILE, LoadLibraryExW, UpdateResourceW,
            },
            UI::WindowsAndMessaging::{RT_GROUP_ICON, RT_ICON},
        },
        core::{BOOL, PCWSTR},
    };

    const ICON_RESOURCE_ID_BASE: u16 = 200;

    pub(super) fn replace_exe_icon(exe_path: &Path, icon_path: &Path) -> Result<()> {
        let icon = IconFile::read(icon_path)?;
        let groups = icon_group_resources(exe_path)?;
        let groups = if groups.is_empty() {
            vec![IconGroupResource {
                name: ResourceName::Id(1),
                languages: vec![0],
            }]
        } else {
            groups
        };

        let exe_wide = wide_path(exe_path);
        let update = unsafe { BeginUpdateResourceW(PCWSTR::from_raw(exe_wide.as_ptr()), false) }
            .with_context(|| {
                format!("failed to begin resource update for {}", exe_path.display())
            })?;

        let result = update_icon_resources(update, &icon, &groups);
        match result {
            Ok(()) => unsafe { EndUpdateResourceW(update, false) }.with_context(|| {
                format!(
                    "failed to commit resource update for {}",
                    exe_path.display()
                )
            }),
            Err(error) => {
                let _ = unsafe { EndUpdateResourceW(update, true) };
                Err(error)
            }
        }
    }

    fn update_icon_resources(
        update: windows::Win32::Foundation::HANDLE,
        icon: &IconFile,
        groups: &[IconGroupResource],
    ) -> Result<()> {
        let languages = groups
            .iter()
            .flat_map(|group| group.languages.iter().copied())
            .collect::<std::collections::BTreeSet<_>>();

        for language in languages {
            for (index, image) in icon.images.iter().enumerate() {
                let id = ICON_RESOURCE_ID_BASE
                    .checked_add(u16::try_from(index).context("too many icon images")?)
                    .context("too many icon images")?;
                unsafe {
                    UpdateResourceW(
                        update,
                        RT_ICON,
                        int_resource(id),
                        language,
                        Some(image.data.as_ptr().cast()),
                        image
                            .data
                            .len()
                            .try_into()
                            .context("icon image is too large")?,
                    )
                }
                .context("failed to update RT_ICON resource")?;
            }
        }

        let group_data = icon.group_resource(ICON_RESOURCE_ID_BASE)?;
        for group in groups {
            for language in &group.languages {
                unsafe {
                    UpdateResourceW(
                        update,
                        RT_GROUP_ICON,
                        group.name.as_pcwstr(),
                        *language,
                        Some(group_data.as_ptr().cast()),
                        group_data
                            .len()
                            .try_into()
                            .context("icon group resource is too large")?,
                    )
                }
                .context("failed to update RT_GROUP_ICON resource")?;
            }
        }

        Ok(())
    }

    fn icon_group_resources(exe_path: &Path) -> Result<Vec<IconGroupResource>> {
        let exe_wide = wide_path(exe_path);
        let module = unsafe {
            LoadLibraryExW(
                PCWSTR::from_raw(exe_wide.as_ptr()),
                None,
                LOAD_LIBRARY_AS_DATAFILE,
            )
        }
        .with_context(|| format!("failed to load resources from {}", exe_path.display()))?;

        let mut names = Vec::<ResourceName>::new();
        let _ = unsafe {
            EnumResourceNamesW(
                Some(module),
                RT_GROUP_ICON,
                Some(enum_resource_name),
                (&mut names as *mut Vec<ResourceName>) as isize,
            )
        };

        let mut groups = Vec::with_capacity(names.len());
        for name in names {
            let mut languages = Vec::<u16>::new();
            unsafe {
                EnumResourceLanguagesW(
                    Some(module),
                    RT_GROUP_ICON,
                    name.as_pcwstr(),
                    Some(enum_resource_language),
                    (&mut languages as *mut Vec<u16>) as isize,
                )
            }
            .context("failed to enumerate icon group languages")?;
            if languages.is_empty() {
                languages.push(0);
            }
            groups.push(IconGroupResource { name, languages });
        }

        unsafe { FreeLibrary(module) }.context("failed to release resource module")?;
        Ok(groups)
    }

    unsafe extern "system" fn enum_resource_name(
        _module: HMODULE,
        _ty: PCWSTR,
        name: PCWSTR,
        lparam: isize,
    ) -> BOOL {
        let names = unsafe { &mut *(lparam as *mut Vec<ResourceName>) };
        names.push(unsafe { ResourceName::from_pcwstr(name) });
        true.into()
    }

    unsafe extern "system" fn enum_resource_language(
        _module: HMODULE,
        _ty: PCWSTR,
        _name: PCWSTR,
        language: u16,
        lparam: isize,
    ) -> BOOL {
        let languages = unsafe { &mut *(lparam as *mut Vec<u16>) };
        languages.push(language);
        true.into()
    }

    #[derive(Clone, Debug)]
    struct IconGroupResource {
        name: ResourceName,
        languages: Vec<u16>,
    }

    #[derive(Clone, Debug)]
    enum ResourceName {
        Id(u16),
        Str(Vec<u16>),
    }

    impl ResourceName {
        unsafe fn from_pcwstr(value: PCWSTR) -> Self {
            let raw = value.as_ptr() as usize;
            if raw <= u16::MAX as usize {
                ResourceName::Id(raw as u16)
            } else {
                let mut text = unsafe { value.as_wide() }.to_vec();
                text.push(0);
                ResourceName::Str(text)
            }
        }

        fn as_pcwstr(&self) -> PCWSTR {
            match self {
                ResourceName::Id(id) => int_resource(*id),
                ResourceName::Str(value) => PCWSTR::from_raw(value.as_ptr()),
            }
        }
    }

    struct IconFile {
        images: Vec<IconImage>,
    }

    struct IconImage {
        width: u8,
        height: u8,
        color_count: u8,
        planes: u16,
        bit_count: u16,
        data: Vec<u8>,
    }

    impl IconFile {
        fn read(path: &Path) -> Result<Self> {
            let data = std::fs::read(path)
                .with_context(|| format!("failed to read icon {}", path.display()))?;
            if data.len() < 6 {
                bail!("icon file is too small: {}", path.display());
            }
            if read_u16(&data, 0)? != 0 || read_u16(&data, 2)? != 1 {
                bail!("not a Windows .ico file: {}", path.display());
            }

            let count = read_u16(&data, 4)? as usize;
            let directory_len = 6usize
                .checked_add(
                    count
                        .checked_mul(16)
                        .context("icon directory is too large")?,
                )
                .context("icon directory is too large")?;
            if data.len() < directory_len {
                bail!("truncated icon directory: {}", path.display());
            }

            let mut images = Vec::with_capacity(count);
            for index in 0..count {
                let entry = 6 + index * 16;
                let bytes_in_res = read_u32(&data, entry + 8)? as usize;
                let image_offset = read_u32(&data, entry + 12)? as usize;
                let image_end = image_offset
                    .checked_add(bytes_in_res)
                    .context("icon image is too large")?;
                if image_end > data.len() {
                    bail!("truncated icon image in {}", path.display());
                }

                images.push(IconImage {
                    width: data[entry],
                    height: data[entry + 1],
                    color_count: data[entry + 2],
                    planes: read_u16(&data, entry + 4)?,
                    bit_count: read_u16(&data, entry + 6)?,
                    data: data[image_offset..image_end].to_vec(),
                });
            }

            Ok(Self { images })
        }

        fn group_resource(&self, first_icon_id: u16) -> Result<Vec<u8>> {
            let count = u16::try_from(self.images.len()).context("too many icon images")?;
            let mut data = Vec::with_capacity(6 + self.images.len() * 14);
            push_u16(&mut data, 0);
            push_u16(&mut data, 1);
            push_u16(&mut data, count);

            for (index, image) in self.images.iter().enumerate() {
                let id = first_icon_id
                    .checked_add(u16::try_from(index).context("too many icon images")?)
                    .context("too many icon images")?;
                data.push(image.width);
                data.push(image.height);
                data.push(image.color_count);
                data.push(0);
                push_u16(&mut data, image.planes);
                push_u16(&mut data, image.bit_count);
                push_u32(
                    &mut data,
                    image
                        .data
                        .len()
                        .try_into()
                        .context("icon image is too large")?,
                );
                push_u16(&mut data, id);
            }

            Ok(data)
        }
    }

    fn read_u16(data: &[u8], offset: usize) -> Result<u16> {
        let bytes = data
            .get(offset..offset + 2)
            .with_context(|| format!("missing u16 at offset {offset}"))?;
        Ok(u16::from_le_bytes(
            bytes.try_into().expect("slice length checked"),
        ))
    }

    fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
        let bytes = data
            .get(offset..offset + 4)
            .with_context(|| format!("missing u32 at offset {offset}"))?;
        Ok(u32::from_le_bytes(
            bytes.try_into().expect("slice length checked"),
        ))
    }

    fn push_u16(data: &mut Vec<u8>, value: u16) {
        data.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u32(data: &mut Vec<u8>, value: u32) {
        data.extend_from_slice(&value.to_le_bytes());
    }

    fn wide_path(path: &Path) -> Vec<u16> {
        path.as_os_str()
            .encode_wide()
            .chain(iter::once(0))
            .collect()
    }

    fn int_resource(id: u16) -> PCWSTR {
        PCWSTR::from_raw(id as usize as *const u16)
    }
}
