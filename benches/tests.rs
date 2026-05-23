use gtk4::{IconLookupFlags, IconTheme, TextDirection};
use nix_freedesktop_icons::lookup;
use speculoos::prelude::*;

#[test]
fn gtk_lookup() {
    gtk4::init().unwrap();
    let theme = IconTheme::new();

    let x = theme.lookup_icon(
        "firefox",
        &[],
        24,
        1,
        TextDirection::None,
        IconLookupFlags::empty(),
    );

    assert!(x.icon_name().is_some())
}

// Linicon sometimes fails with theme that have unknown parents
// This test only ensure we are running the correct function in the benchmarks
// And results are identical.
#[test]
fn linicon() {
    // Current theme
    let lin_user_home = linicon::lookup_icon("user-home")
        .from_theme("Adwaita")
        .with_size(16)
        .with_scale(1)
        .next();

    let user_home = lookup("user-home")
        .with_theme("Adwaita")
        .with_size(16)
        .with_scale(1)
        .find();

    asserting!("Linicon return some icon")
        .that(&lin_user_home.unwrap())
        .is_ok()
        .map(|icon| &icon.path)
        .matches(|p| p.ends_with("share/icons/Adwaita/16x16/places/user-home.png"));

    asserting!("Our implementation should return the same result as linicon")
        .that(&user_home)
        .is_some()
        .matches(|p| p.ends_with("share/icons/Adwaita/16x16/places/user-home.png"));

    // Fallback to hicolor
    let lin_firefox = linicon::lookup_icon("firefox")
        .from_theme("Adwaita")
        .with_size(16)
        .with_scale(1)
        .next();

    let firefox = lookup("firefox")
        .with_theme("Adwaita")
        .with_size(16)
        .with_scale(1)
        .find();

    asserting!("Linicon return some icon")
        .that(&lin_firefox.unwrap())
        .is_ok()
        .map(|icon| &icon.path)
        .matches(|p| p.ends_with("share/icons/hicolor/16x16/apps/firefox.png"));

    asserting!("Our implementation should return the same result as linicon")
        .that(&firefox)
        .is_some()
        .matches(|p| p.ends_with("share/icons/hicolor/16x16/apps/firefox.png"));

    // pixmaps
    let lin_steam = linicon::lookup_icon("steam_tray_mono")
        .from_theme("Adwaita")
        .with_size(16)
        .with_scale(1)
        .next();

    let steam = lookup("steam_tray_mono")
        .with_theme("Adwaita")
        .with_size(16)
        .with_scale(1)
        .find();

    asserting!("Linicon fails to fallback to pixmaps")
        .that(&lin_steam)
        .is_none();

    asserting!("But we succeed")
        .that(&steam)
        .is_some()
        .matches(|p| p.ends_with("share/pixmaps/steam_tray_mono.png"));
}
