use gpui::{AssetSource, Result, SharedString};
use gpui_component_assets::Assets as ComponentAssets;
use std::borrow::Cow;

const BRAIN_SVG: &[u8] = include_bytes!("../assets/icons/brain.svg");
const BOOK_CHECK_SVG: &[u8] = include_bytes!("../assets/icons/book-check.svg");
const CHEVRON_DOWN_SVG: &[u8] = include_bytes!("../assets/icons/chevron-down.svg");
const CHEVRON_RIGHT_SVG: &[u8] = include_bytes!("../assets/icons/chevron-right.svg");
const CIRCLE_DOT_SVG: &[u8] = include_bytes!("../assets/icons/circle-dot.svg");
const CIRCLE_CHECK_SVG: &[u8] = include_bytes!("../assets/icons/circle-check.svg");
const CIRCLE_X_SVG: &[u8] = include_bytes!("../assets/icons/circle-x.svg");
const CIRCLE_SVG: &[u8] = include_bytes!("../assets/icons/circle.svg");
const CLOCK_SVG: &[u8] = include_bytes!("../assets/icons/clock.svg");
const GIT_BRANCH_SVG: &[u8] = include_bytes!("../assets/icons/git-branch.svg");
const GIT_PULL_REQUEST_SVG: &[u8] = include_bytes!("../assets/icons/git-pull-request.svg");
const GIT_PULL_REQUEST_ARROW_SVG: &[u8] =
    include_bytes!("../assets/icons/git-pull-request-arrow.svg");
const HOUSE_SVG: &[u8] = include_bytes!("../assets/icons/house.svg");
const LOADER_CIRCLE_SVG: &[u8] = include_bytes!("../assets/icons/loader-circle.svg");
const MESSAGE_CIRCLE_SVG: &[u8] = include_bytes!("../assets/icons/message-circle.svg");
const MESSAGE_SQUARE_DOT_SVG: &[u8] = include_bytes!("../assets/icons/message-square-dot.svg");
const MESSAGE_SQUARE_MORE_SVG: &[u8] = include_bytes!("../assets/icons/message-square-more.svg");
const NOTEBOOK_TEXT_SVG: &[u8] = include_bytes!("../assets/icons/notebook-text.svg");
const PLUS_SVG: &[u8] = include_bytes!("../assets/icons/plus.svg");
const PLAY_SVG: &[u8] = include_bytes!("../assets/icons/play.svg");
const SETTINGS_SVG: &[u8] = include_bytes!("../assets/icons/settings.svg");
const SPARKLES_SVG: &[u8] = include_bytes!("../assets/icons/sparkles.svg");
const SQUARE_KANBAN_SVG: &[u8] = include_bytes!("../assets/icons/square-kanban.svg");
const TIMER_SVG: &[u8] = include_bytes!("../assets/icons/timer.svg");
const USER_PEN_SVG: &[u8] = include_bytes!("../assets/icons/user-pen.svg");
const ZED_SVG: &[u8] = include_bytes!("../assets/icons/zed.svg");
const ARCHIVE_SVG: &[u8] = include_bytes!("../assets/icons/archive.svg");

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
            "icons/book-check.svg" => Ok(Some(Cow::Borrowed(BOOK_CHECK_SVG))),
            "icons/chevron-down.svg" => Ok(Some(Cow::Borrowed(CHEVRON_DOWN_SVG))),
            "icons/chevron-right.svg" => Ok(Some(Cow::Borrowed(CHEVRON_RIGHT_SVG))),
            "icons/circle-dot.svg" => Ok(Some(Cow::Borrowed(CIRCLE_DOT_SVG))),
            "icons/circle-check.svg" => Ok(Some(Cow::Borrowed(CIRCLE_CHECK_SVG))),
            "icons/circle-x.svg" => Ok(Some(Cow::Borrowed(CIRCLE_X_SVG))),
            "icons/circle.svg" => Ok(Some(Cow::Borrowed(CIRCLE_SVG))),
            "icons/clock.svg" => Ok(Some(Cow::Borrowed(CLOCK_SVG))),
            "icons/git-branch.svg" => Ok(Some(Cow::Borrowed(GIT_BRANCH_SVG))),
            "icons/git-pull-request.svg" => Ok(Some(Cow::Borrowed(GIT_PULL_REQUEST_SVG))),
            "icons/git-pull-request-arrow.svg" => {
                Ok(Some(Cow::Borrowed(GIT_PULL_REQUEST_ARROW_SVG)))
            }
            "icons/house.svg" => Ok(Some(Cow::Borrowed(HOUSE_SVG))),
            "icons/loader-circle.svg" => Ok(Some(Cow::Borrowed(LOADER_CIRCLE_SVG))),
            "icons/message-circle.svg" => Ok(Some(Cow::Borrowed(MESSAGE_CIRCLE_SVG))),
            "icons/message-square-dot.svg" => Ok(Some(Cow::Borrowed(MESSAGE_SQUARE_DOT_SVG))),
            "icons/message-square-more.svg" => Ok(Some(Cow::Borrowed(MESSAGE_SQUARE_MORE_SVG))),
            "icons/notebook-text.svg" => Ok(Some(Cow::Borrowed(NOTEBOOK_TEXT_SVG))),
            "icons/plus.svg" => Ok(Some(Cow::Borrowed(PLUS_SVG))),
            "icons/play.svg" => Ok(Some(Cow::Borrowed(PLAY_SVG))),
            "icons/settings.svg" => Ok(Some(Cow::Borrowed(SETTINGS_SVG))),
            "icons/sparkles.svg" => Ok(Some(Cow::Borrowed(SPARKLES_SVG))),
            "icons/square-kanban.svg" => Ok(Some(Cow::Borrowed(SQUARE_KANBAN_SVG))),
            "icons/timer.svg" => Ok(Some(Cow::Borrowed(TIMER_SVG))),
            "icons/user-pen.svg" => Ok(Some(Cow::Borrowed(USER_PEN_SVG))),
            "icons/zed.svg" => Ok(Some(Cow::Borrowed(ZED_SVG))),
            "icons/archive.svg" => Ok(Some(Cow::Borrowed(ARCHIVE_SVG))),
            _ => self.fallback.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut assets = self.fallback.list(path)?;

        if "icons/brain.svg".starts_with(path) {
            assets.push("icons/brain.svg".into());
        }
        if "icons/book-check.svg".starts_with(path) {
            assets.push("icons/book-check.svg".into());
        }
        if "icons/chevron-down.svg".starts_with(path) {
            assets.push("icons/chevron-down.svg".into());
        }
        if "icons/chevron-right.svg".starts_with(path) {
            assets.push("icons/chevron-right.svg".into());
        }
        if "icons/circle-dot.svg".starts_with(path) {
            assets.push("icons/circle-dot.svg".into());
        }
        if "icons/circle-check.svg".starts_with(path) {
            assets.push("icons/circle-check.svg".into());
        }
        if "icons/circle-x.svg".starts_with(path) {
            assets.push("icons/circle-x.svg".into());
        }
        if "icons/circle.svg".starts_with(path) {
            assets.push("icons/circle.svg".into());
        }
        if "icons/clock.svg".starts_with(path) {
            assets.push("icons/clock.svg".into());
        }
        if "icons/git-branch.svg".starts_with(path) {
            assets.push("icons/git-branch.svg".into());
        }
        if "icons/git-pull-request.svg".starts_with(path) {
            assets.push("icons/git-pull-request.svg".into());
        }
        if "icons/git-pull-request-arrow.svg".starts_with(path) {
            assets.push("icons/git-pull-request-arrow.svg".into());
        }
        if "icons/house.svg".starts_with(path) {
            assets.push("icons/house.svg".into());
        }
        if "icons/loader-circle.svg".starts_with(path) {
            assets.push("icons/loader-circle.svg".into());
        }
        if "icons/message-circle.svg".starts_with(path) {
            assets.push("icons/message-circle.svg".into());
        }
        if "icons/message-square-dot.svg".starts_with(path) {
            assets.push("icons/message-square-dot.svg".into());
        }
        if "icons/message-square-more.svg".starts_with(path) {
            assets.push("icons/message-square-more.svg".into());
        }
        if "icons/notebook-text.svg".starts_with(path) {
            assets.push("icons/notebook-text.svg".into());
        }
        if "icons/plus.svg".starts_with(path) {
            assets.push("icons/plus.svg".into());
        }
        if "icons/play.svg".starts_with(path) {
            assets.push("icons/play.svg".into());
        }
        if "icons/settings.svg".starts_with(path) {
            assets.push("icons/settings.svg".into());
        }
        if "icons/sparkles.svg".starts_with(path) {
            assets.push("icons/sparkles.svg".into());
        }
        if "icons/square-kanban.svg".starts_with(path) {
            assets.push("icons/square-kanban.svg".into());
        }
        if "icons/timer.svg".starts_with(path) {
            assets.push("icons/timer.svg".into());
        }
        if "icons/user-pen.svg".starts_with(path) {
            assets.push("icons/user-pen.svg".into());
        }
        if "icons/zed.svg".starts_with(path) {
            assets.push("icons/zed.svg".into());
        }
        if "icons/archive.svg".starts_with(path) {
            assets.push("icons/archive.svg".into());
        }

        assets.sort();
        assets.dedup();
        Ok(assets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_assets_load_custom_icons() {
        let assets = AppAssets::default();

        for path in [
            "icons/archive.svg",
            "icons/book-check.svg",
            "icons/chevron-down.svg",
            "icons/chevron-right.svg",
            "icons/circle-check.svg",
            "icons/circle-dot.svg",
            "icons/circle-x.svg",
            "icons/circle.svg",
            "icons/clock.svg",
            "icons/house.svg",
            "icons/git-branch.svg",
            "icons/git-pull-request.svg",
            "icons/git-pull-request-arrow.svg",
            "icons/loader-circle.svg",
            "icons/message-circle.svg",
            "icons/message-square-dot.svg",
            "icons/message-square-more.svg",
            "icons/notebook-text.svg",
            "icons/plus.svg",
            "icons/play.svg",
            "icons/settings.svg",
            "icons/sparkles.svg",
            "icons/square-kanban.svg",
            "icons/user-pen.svg",
        ] {
            let loaded = assets.load(path).expect("asset load should not fail");
            assert!(loaded.is_some(), "expected asset to exist: {path}");
        }
    }
}
