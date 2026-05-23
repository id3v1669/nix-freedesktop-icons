use once_cell::sync::Lazy;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

pub(crate) static CACHE: Lazy<Cache> = Lazy::new(Cache::default);

type Theme = Box<str>;
type Icon = Box<str>;
type SizedMap = BTreeMap<(u16, u16), CacheEntry>;
type IconMap = BTreeMap<Icon, SizedMap>;
type ThemeMap = BTreeMap<Theme, IconMap>;

#[derive(Default)]
pub(crate) struct Cache(RwLock<ThemeMap>);

#[derive(Debug, Clone, PartialEq)]
pub enum CacheEntry {
    // We already looked for this and nothing was found, indicates we should not try to perform a lookup.
    NotFound,
    // We have this entry.
    Found(PathBuf),
    // We don't know this entry yet, indicate we should perform a lookup.
    Unknown,
}

impl Cache {
    pub fn insert<P: AsRef<Path>>(
        &self,
        theme: &str,
        size: u16,
        scale: u16,
        icon_name: &str,
        icon_path: &Option<P>,
    ) {
        let entry = icon_path
            .as_ref()
            .map(|path| CacheEntry::Found(path.as_ref().to_path_buf()))
            .unwrap_or(CacheEntry::NotFound);

        self.0
            .write()
            .unwrap()
            .entry(theme.into())
            .or_default()
            .entry(icon_name.into())
            .or_default()
            .insert((size, scale), entry);
    }

    pub fn get(&self, theme: &str, size: u16, scale: u16, icon_name: &str) -> CacheEntry {
        self.0
            .read()
            .unwrap()
            .get(theme)
            .and_then(|icon_map| icon_map.get(icon_name))
            .and_then(|sized_map| sized_map.get(&(size, scale)).cloned())
            .unwrap_or(CacheEntry::Unknown)
    }
}
