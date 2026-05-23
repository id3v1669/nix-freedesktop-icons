use crate::theme::paths::ThemePath;
use memmap2::Mmap;
use once_cell::sync::Lazy;
pub(crate) use paths::BASE_PATHS;
use std::collections::BTreeMap;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

mod directories;
mod parse;
mod paths;

pub static THEMES: Lazy<BTreeMap<Vec<u8>, Vec<Theme>>> = Lazy::new(get_all_themes);

#[inline]
pub fn read_ini_theme(path: &Path) -> std::io::Result<Mmap> {
    std::fs::File::open(path).and_then(|file| unsafe { Mmap::map(&file) })
}

#[derive(Debug)]
pub struct Theme {
    pub path: ThemePath,
    pub index: PathBuf,
}

impl Theme {
    pub fn try_get_icon(
        &self,
        name: &str,
        size: u16,
        scale: u16,
        force_svg: bool,
    ) -> Option<PathBuf> {
        let file = read_ini_theme(&self.index).ok()?;
        self.try_get_icon_exact_size(file.as_ref(), name, size, scale, force_svg)
            .or_else(|| self.try_get_icon_closest_size(file.as_ref(), name, size, scale, force_svg))
    }

    fn try_get_icon_exact_size(
        &self,
        file: &[u8],
        name: &str,
        size: u16,
        scale: u16,
        force_svg: bool,
    ) -> Option<PathBuf> {
        let dirs: Vec<&str> = self.match_size(file, size, scale).collect();
        self.pick_icon(&dirs, name, force_svg)
    }

    fn match_size<'a>(
        &'a self,
        file: &'a [u8],
        size: u16,
        scale: u16,
    ) -> impl Iterator<Item = &'a str> + 'a {
        self.get_all_directories(file)
            .filter(move |directory| directory.match_size(size, scale))
            .map(|dir| dir.name)
    }

    fn try_get_icon_closest_size(
        &self,
        file: &[u8],
        name: &str,
        size: u16,
        scale: u16,
        force_svg: bool,
    ) -> Option<PathBuf> {
        let dirs: Vec<&str> = self.closest_match_size(file, size, scale).collect();
        self.pick_icon(&dirs, name, force_svg)
    }

    fn closest_match_size<'a>(
        &'a self,
        file: &'a [u8],
        size: u16,
        scale: u16,
    ) -> impl Iterator<Item = &'a str> + 'a {
        self.get_all_directories(file)
            .fold(Vec::<(&'a str, i16)>::new(), |mut sorted, directory| {
                let distance = directory.directory_size_distance(size, scale);
                if distance < i16::MAX {
                    let a = distance.abs();
                    let pos = sorted
                        .binary_search_by(|(_, b)| b.cmp(&a))
                        .unwrap_or_else(|pos| pos);
                    sorted.insert(pos, (directory.name, a));
                }
                sorted
            })
            .into_iter()
            .map(|(name, _)| name)
    }

    /// Resolve `name` within the given size-matching `dirs` of this theme.
    ///
    /// The preferred extension (svg when `force_svg`, otherwise png) is searched
    /// across *all* `dirs` before falling back to the next extension, so a
    /// scalable svg is returned over a same-size raster png when requested.
    /// A single `PathBuf` and name buffer are reused across every probe.
    fn pick_icon(&self, dirs: &[&str], name: &str, force_svg: bool) -> Option<PathBuf> {
        let exts: [&str; 3] = if force_svg {
            [".svg", ".png", ".xmp"]
        } else {
            [".png", ".svg", ".xmp"]
        };

        let mut name_buf = String::with_capacity(name.len() + 4);
        let mut path = self.path().clone();

        for ext in exts {
            name_buf.clear();
            name_buf.push_str(name);
            name_buf.push_str(ext);

            for dir in dirs {
                path.push(dir);
                path.push(&name_buf);
                if path.exists() {
                    return Some(path);
                }
                // Restore `path` back to the theme base directory.
                path.pop(); // file name
                for _ in 0..dir.bytes().filter(|&c| c == b'/').count() + 1 {
                    path.pop();
                }
            }
        }

        None
    }

    fn path(&self) -> &PathBuf {
        &self.path.0
    }
}

pub(super) fn try_build_icon_path(path: &mut PathBuf, name: &str, force_svg: bool) -> bool {
    let mut name_buf = String::with_capacity(name.len() + 4);
    name_buf.push_str(name);
    path.push(name);
    if force_svg {
        try_build_ext(path, &mut name_buf, name, ".svg")
            || try_build_ext(path, &mut name_buf, name, ".png")
            || try_build_ext(path, &mut name_buf, name, ".xmp")
    } else {
        try_build_ext(path, &mut name_buf, name, ".png")
            || try_build_ext(path, &mut name_buf, name, ".svg")
            || try_build_ext(path, &mut name_buf, name, ".xmp")
    }
}

#[inline]
fn try_build_ext(path: &mut PathBuf, name_buf: &mut String, name: &str, ext: &'static str) -> bool {
    name_buf.truncate(name.len());
    name_buf.push_str(ext);
    path.set_file_name(&name_buf);
    path.exists()
}

// Iter through the base paths and get all theme directories
pub(super) fn get_all_themes() -> BTreeMap<Vec<u8>, Vec<Theme>> {
    let mut icon_themes = BTreeMap::<Vec<u8>, Vec<_>>::new();
    let mut found_indices = BTreeMap::new();
    let mut to_revisit = Vec::new();

    for theme_base_dir in BASE_PATHS.iter() {
        let dir_iter = match theme_base_dir.read_dir() {
            Ok(dir) => dir,
            Err(why) => {
                tracing::error!(?why, dir = ?theme_base_dir, "unable to read icon theme directory");
                continue;
            }
        };

        for entry in dir_iter.filter_map(std::io::Result::ok) {
            let name = entry.file_name();
            let fallback_index = found_indices.get(&name);
            if let Some(theme) = Theme::from_path(entry.path(), fallback_index) {
                if fallback_index.is_none() {
                    found_indices.insert(name.clone(), theme.index.clone());
                }
                icon_themes
                    .entry(name.as_bytes().to_owned())
                    .or_default()
                    .push(theme);
            } else if entry.path().is_dir() {
                to_revisit.push(entry);
            }
        }
    }

    for entry in to_revisit {
        let name = entry.file_name();
        let fallback_index = found_indices.get(&name);
        if let Some(theme) = Theme::from_path(entry.path(), fallback_index) {
            icon_themes
                .entry(name.as_bytes().to_owned())
                .or_default()
                .push(theme);
        }
    }

    icon_themes
}

impl Theme {
    pub(crate) fn from_path<P: AsRef<Path>>(path: P, index: Option<&PathBuf>) -> Option<Self> {
        let mut path = path.as_ref().to_path_buf();
        let is_dir = path.is_dir();
        path.push("index.theme");
        let local_index_exists = path.exists();
        let has_index = local_index_exists || index.is_some();

        if !has_index || !is_dir {
            return None;
        }

        index
            .cloned()
            .or_else(|| local_index_exists.then_some(path.clone()))
            .map(|index| Theme {
                path: ThemePath({
                    path.pop();
                    path
                }),
                index,
            })
    }
}

#[cfg(test)]
mod test {
    use crate::THEMES;

    #[test]
    fn get_one_icon() {
        let themes = THEMES.get(&b"Adwaita"[..]).unwrap();
        println!(
            "{:?}",
            themes.iter().find_map(|t| {
                let file = super::read_ini_theme(&t.index).ok()?;
                t.try_get_icon_exact_size(file.as_ref(), "edit-delete-symbolic", 24, 1, false)
            })
        );
    }

    #[test]
    fn should_get_png() {
        let themes = THEMES.get(&b"hicolor"[..]).unwrap();
        let icon = themes.iter().find_map(|t| {
            let file = super::read_ini_theme(&t.index).ok()?;
            t.try_get_icon_exact_size(file.as_ref(), "blueman", 24, 1, false)
        });
        assert!(
            icon.as_ref()
                .is_some_and(|p| p.ends_with("share/icons/hicolor/22x22/apps/blueman.png"))
        );
    }

    #[test]
    fn should_get_svg() {
        let themes = THEMES.get(&b"hicolor"[..]).unwrap();
        let icon = themes.iter().find_map(|t| {
            let file = super::read_ini_theme(&t.index).ok()?;
            t.try_get_icon_exact_size(file.as_ref(), "blueman", 24, 1, true)
        });
        assert!(
            icon.as_ref()
                .is_some_and(|p| p.ends_with("share/icons/hicolor/scalable/apps/blueman.svg"))
        );
    }
}
