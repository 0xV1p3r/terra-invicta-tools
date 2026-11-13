//! Reads the installed game's bundles; `cargo test -p ti-unity -- --ignored`. Set `TI_GAME_DIR`
//! if the game is not in the default Steam location. Skips when the game is not installed.

use std::path::PathBuf;
use std::sync::Arc;

use ti_unity::{Bundle, SerializedFile, Sprite, Texture2D, class_id};

fn bundle_dir() -> Option<PathBuf> {
    let game = std::env::var_os("TI_GAME_DIR").map(PathBuf::from).or_else(|| {
        let home = std::env::var_os("HOME")?;
        Some(PathBuf::from(home).join(".local/share/Steam/steamapps/common/Terra Invicta"))
    })?;
    let dir = game.join("TerraInvicta_Data/StreamingAssets/AssetBundles");
    dir.is_dir().then_some(dir)
}

#[test]
#[ignore = "needs the installed game"]
fn decodes_every_texture_and_sprite_of_small_bundles() {
    let Some(dir) = bundle_dir() else {
        eprintln!("game not found, skipping");
        return;
    };
    for name in ["cursors", "councilor_missions", "faction_logos", "nationflags", "ui"] {
        let bundle = Arc::new(Bundle::open(&dir.join(name)).unwrap());
        let (mut textures, mut sprites) = (0, 0);
        for sf in SerializedFile::all(&bundle).unwrap() {
            for obj in &sf.objects {
                match obj.class_id {
                    class_id::TEXTURE_2D => {
                        let tex = Texture2D::read(&sf, obj).unwrap();
                        let im = tex.image(&sf).unwrap();
                        assert_eq!((im.width, im.height), (tex.width, tex.height));
                        textures += 1;
                    }
                    class_id::SPRITE => {
                        let sprite = Sprite::from_value(&sf.read(obj).unwrap(), &sf).unwrap();
                        let source = sprite.source_image(&sf).unwrap();
                        sprite.image(source).unwrap();
                        sprites += 1;
                    }
                    _ => {}
                }
            }
        }
        eprintln!("{name}: {textures} textures, {sprites} sprites");
        assert!(textures > 0);
    }
}
