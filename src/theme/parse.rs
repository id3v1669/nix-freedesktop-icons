use crate::theme::Theme;
use crate::theme::directories::{Directory, DirectoryType};
use bstr::BStr;

impl Theme {
    pub(super) fn get_all_directories<'a>(
        &'a self,
        file: &'a [u8],
    ) -> impl Iterator<Item = Directory<'a>> + 'a {
        let mut iterator = sections(file);

        std::iter::from_fn(move || {
            let mut name = "";
            let mut size = None;
            let mut max_size = None;
            let mut min_size = None;
            let mut threshold = None;
            let mut scale = None;
            // let mut context = None;
            let mut dtype = DirectoryType::default();

            #[allow(clippy::while_let_on_iterator)]
            while let Some(event) = iterator.next() {
                match event {
                    DirectorySection::Property(key, value) => {
                        if name.is_empty() || name == "Icon Theme" {
                            continue;
                        }

                        match key {
                            b"Size" => size = btoi::btoi(value).ok(),
                            b"Scale" => scale = btoi::btoi(value).ok(),
                            // b"Context" => context = Some(value),
                            b"Type" => dtype = DirectoryType::from(value),
                            b"MaxSize" => max_size = btoi::btoi(value).ok(),
                            b"MinSize" => min_size = btoi::btoi(value).ok(),
                            b"Threshold" => threshold = btoi::btoi(value).ok(),
                            _ => (),
                        }
                    }

                    DirectorySection::Section(new_name) => {
                        name = std::str::from_utf8(new_name).unwrap_or("");
                        size = None;
                        max_size = None;
                        min_size = None;
                        threshold = None;
                        scale = None;
                        dtype = DirectoryType::default();
                    }

                    DirectorySection::EndSection => {
                        if name.is_empty() || name == "Icon Theme" {
                            continue;
                        }

                        let size = size.take()?;

                        return Some(Directory {
                            name,
                            size,
                            scale: scale.unwrap_or(1),
                            // context,
                            type_: dtype,
                            maxsize: max_size.unwrap_or(size),
                            minsize: min_size.unwrap_or(size),
                            threshold: threshold.unwrap_or(2),
                        });
                    }
                }
            }

            None
        })
    }

    pub fn inherits<'a>(&self, file: &'a [u8]) -> impl Iterator<Item = &'a [u8]> {
        icon_theme_section(file)
            .find(|&(key, _)| key == b"Inherits")
            .into_iter()
            .flat_map(|(_, parents)| {
                BStr::new(parents)
                    .split(|&char| char == b',')
                    // Filtering out 'hicolor' since we are going to fallback there anyway
                    .filter(|parent| parent != b"hicolor")
            })
    }
}

#[derive(Debug)]
enum DirectorySection<'a> {
    Property(&'a [u8], &'a [u8]),
    EndSection,
    Section(&'a [u8]),
}

fn sections(file: &[u8]) -> impl Iterator<Item = DirectorySection<'_>> {
    let mut finished = false;
    let mut table_found = false;
    let mut section: &[u8] = b"";
    let mut prev = 0;
    let mut tail_done = false;
    let mut line_indices = memchr::memchr_iter(b'\n', file);

    std::iter::from_fn(move || {
        if finished {
            return None;
        }

        if !section.is_empty() {
            let new_section = section;
            section = b"";
            return Some(DirectorySection::Section(new_section));
        }

        loop {
            let line_end = match line_indices.next() {
                Some(pos) => pos,
                None if !tail_done && prev < file.len() => {
                    tail_done = true;
                    file.len()
                }
                None => {
                    finished = true;
                    return Some(DirectorySection::EndSection);
                }
            };

            let line = BStr::new(&file[prev..line_end]).trim_ascii();
            prev = line_end + 1;

            if line.is_empty() {
                continue;
            }

            if line[0] == b'[' {
                let Some(new_section) = line
                    .strip_prefix(b"[")
                    .and_then(|rest| rest.strip_suffix(b"]"))
                else {
                    continue;
                };
                section = new_section;
                if table_found {
                    return Some(DirectorySection::EndSection);
                } else {
                    table_found = true;
                    return Some(DirectorySection::Section(section));
                }
            }

            if let Some((key, value)) = memchr::memchr(b'=', line).map(|pos| unsafe {
                // Position was already validated by memchr.
                line.split_at_unchecked(pos)
            }) {
                return Some(DirectorySection::Property(key, &value[1..]));
            }
        }
    })
}

fn icon_theme_section(file: &[u8]) -> impl Iterator<Item = (&[u8], &[u8])> + '_ {
    let mut found_table = false;
    let mut prev = 0;
    let mut tail_done = false;
    let mut line_indices = memchr::memchr_iter(b'\n', file);

    std::iter::from_fn(move || {
        loop {
            let line_end = match line_indices.next() {
                Some(pos) => pos,
                None if !tail_done && prev < file.len() => {
                    tail_done = true;
                    file.len()
                }
                None => return None,
            };
            let line = BStr::new(&file[prev..line_end]).trim_ascii();
            prev = line_end + 1;

            if line.is_empty() {
                continue;
            }

            if line[0] == b'[' {
                if found_table {
                    return None;
                }
                if let Some(section) = line
                    .strip_prefix(b"[")
                    .and_then(|rest| rest.strip_suffix(b"]"))
                {
                    found_table = section == b"Icon Theme";
                }
            }

            if let Some((key, value)) = memchr::memchr(b'=', line).map(|pos| unsafe {
                // Position was already validated by memchr.
                line.split_at_unchecked(pos)
            }) {
                return Some((key, &value[1..]));
            }
        }
    })
}

#[cfg(test)]
mod test {
    use crate::THEMES;
    use crate::theme::Theme;
    use crate::theme::paths::ThemePath;
    use speculoos::prelude::*;
    use std::path::PathBuf;

    fn fake_theme() -> Theme {
        Theme {
            path: ThemePath(PathBuf::from("/x")),
            index: PathBuf::new(),
        }
    }

    #[test]
    fn parses_last_directory_without_trailing_newline() {
        let theme = fake_theme();
        let file = b"[Icon Theme]\nName=X\n[48x48/apps]\nType=Fixed\nSize=48"; // no final '\n'
        let dirs: Vec<(String, i16)> = theme
            .get_all_directories(file)
            .map(|d| (d.name.to_string(), d.size))
            .collect();
        assert!(
            dirs.iter().any(|(n, s)| n == "48x48/apps" && *s == 48),
            "expected 48x48/apps with size 48, got {dirs:?}"
        );
    }

    #[test]
    fn malformed_section_header_is_skipped_without_panic() {
        let theme = fake_theme();
        let file = b"[Icon Theme]\nName=X\n[\nSize=99\n[16x16]\nSize=16\n";
        let dirs: Vec<String> = theme
            .get_all_directories(file)
            .map(|d| d.name.to_string())
            .collect();
        assert!(dirs.iter().any(|n| n == "16x16"), "got {dirs:?}");
    }

    #[test]
    fn inherits_without_trailing_newline() {
        let theme = fake_theme();
        let file = b"[Icon Theme]\nName=X\nInherits=Moka,Adwaita"; // no final '\n'
        let parents: Vec<&[u8]> = theme.inherits(file).collect();
        assert_eq!(parents, vec![&b"Moka"[..], &b"Adwaita"[..]]);
    }

    #[test]
    fn should_get_theme_parents() {
        for theme in THEMES.get(&b"Arc"[..]).unwrap() {
            let file = crate::theme::read_ini_theme(&theme.index).ok().unwrap();
            let parents: Vec<&[u8]> = theme.inherits(file.as_ref()).collect();

            assert_that!(&parents).does_not_contain(&b"hicolor"[..]);

            assert_that!(&parents).is_equal_to(vec![&b"Moka"[..], &b"Adwaita"[..], &b"gnome"[..]]);
        }
    }
}
