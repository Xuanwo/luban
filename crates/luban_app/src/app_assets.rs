use gpui::{AssetSource, Result, SharedString};
use gpui_component_assets::Assets as ComponentAssets;
use std::borrow::Cow;

const BRAIN_SVG: &[u8] = include_bytes!("../assets/icons/brain.svg");
const TIMER_SVG: &[u8] = include_bytes!("../assets/icons/timer.svg");

pub struct AppAssets {
    fallback: ComponentAssets,
}

impl Default for AppAssets {
    fn default() -> Self {
        Self {
            fallback: ComponentAssets,
        }
    }
}

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        match path {
            "icons/brain.svg" => Ok(Some(Cow::Borrowed(BRAIN_SVG))),
            "icons/timer.svg" => Ok(Some(Cow::Borrowed(TIMER_SVG))),
            _ => self.fallback.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut assets = self.fallback.list(path)?;

        if "icons/brain.svg".starts_with(path) {
            assets.push("icons/brain.svg".into());
        }
        if "icons/timer.svg".starts_with(path) {
            assets.push("icons/timer.svg".into());
        }

        assets.sort();
        assets.dedup();
        Ok(assets)
    }
}
