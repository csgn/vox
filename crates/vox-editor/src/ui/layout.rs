//! Screen layout: where the editor, panels, modal, and notifications go.

use crate::surface::Rect;

/// Side panel position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidePosition {
    /// Left of the text.
    Left,
    /// Right of the text.
    #[default]
    Right,
}

/// Notification corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotifyPosition {
    /// Top right.
    #[default]
    TopRight,
    /// Bottom right (above the status line).
    BottomRight,
}

/// Where a dialog that can live in either place opens (the plugin manager, the command
/// browser).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ManagerPlace {
    /// Centered modal.
    #[default]
    Modal,
    /// Side panel.
    Side,
}

/// UI settings (`[ui]` in the config file).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiConfig {
    /// Side panel position.
    pub side_panel: SidePosition,
    /// Side panel width in columns.
    pub side_width: u16,
    /// Bottom panel height in rows.
    pub bottom_height: u16,
    /// Modal width, percent of the screen.
    pub modal_width: u16,
    /// Modal height, percent of the screen.
    pub modal_height: u16,
    /// How long notifications stay, in milliseconds.
    pub notify_timeout_ms: u64,
    /// Notification corner.
    pub notify_position: NotifyPosition,
    /// Where `:plugins` opens.
    pub plugin_manager: ManagerPlace,
    /// Where `:commands` opens.
    pub commands: ManagerPlace,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            side_panel: SidePosition::Right,
            side_width: 40,
            bottom_height: 12,
            modal_width: 70,
            modal_height: 60,
            notify_timeout_ms: 4000,
            notify_position: NotifyPosition::TopRight,
            plugin_manager: ManagerPlace::Modal,
            commands: ManagerPlace::Modal,
        }
    }
}

/// Computed screen areas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Layout {
    /// Text area (gutter + text).
    pub editor: Rect,
    /// Side panel, if open.
    pub side: Option<Rect>,
    /// Bottom panel, if open.
    pub bottom: Option<Rect>,
    /// Modal, if open.
    pub modal: Option<Rect>,
    /// Area notifications may use.
    pub body: Rect,
}

/// Rows below the body: status line + command line.
const CHROME_ROWS: u16 = 2;
/// Smallest text area kept when panels are open.
const MIN_EDITOR: u16 = 10;

/// Split a `w` x `h` screen. Panels shrink so the text area keeps at least a few cells.
pub(crate) fn compute(
    (w, h): (u16, u16),
    cfg: &UiConfig,
    side: bool,
    bottom: bool,
    modal: bool,
) -> Layout {
    let body = Rect::new(0, 0, w, h.saturating_sub(CHROME_ROWS));
    let mut rest = body;
    let bottom = bottom.then(|| {
        let bh = cfg
            .bottom_height
            .min(rest.h.saturating_sub(3))
            .max(3.min(rest.h));
        rest.h -= bh;
        Rect::new(rest.x, rest.y + rest.h, rest.w, bh)
    });
    let side = side.then(|| {
        let sw = cfg.side_width.min(rest.w.saturating_sub(MIN_EDITOR));
        rest.w -= sw;
        match cfg.side_panel {
            SidePosition::Left => {
                let r = Rect::new(rest.x, rest.y, sw, rest.h);
                rest.x += sw;
                r
            }
            SidePosition::Right => Rect::new(rest.x + rest.w, rest.y, sw, rest.h),
        }
    });
    let modal = modal.then(|| {
        let mw =
            (w as u32 * cfg.modal_width.clamp(10, 100) as u32 / 100).max(20.min(w as u32)) as u16;
        let mh =
            (h as u32 * cfg.modal_height.clamp(10, 100) as u32 / 100).max(5.min(h as u32)) as u16;
        Rect::new((w - mw) / 2, (h.saturating_sub(mh)) / 2, mw, mh)
    });
    Layout {
        editor: rest,
        side,
        bottom,
        modal,
        body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_panels() {
        let l = compute((80, 24), &UiConfig::default(), false, false, false);
        assert_eq!(l.editor, Rect::new(0, 0, 80, 22));
        assert_eq!((l.side, l.bottom, l.modal), (None, None, None));
    }

    #[test]
    fn side_right_and_bottom() {
        let l = compute((80, 24), &UiConfig::default(), true, true, false);
        assert_eq!(l.bottom, Some(Rect::new(0, 10, 80, 12)));
        assert_eq!(l.side, Some(Rect::new(40, 0, 40, 10)));
        assert_eq!(l.editor, Rect::new(0, 0, 40, 10));
    }

    #[test]
    fn side_left_and_small_screen() {
        let cfg = UiConfig {
            side_panel: SidePosition::Left,
            ..UiConfig::default()
        };
        let l = compute((30, 10), &cfg, true, false, false);
        assert_eq!(l.side, Some(Rect::new(0, 0, 20, 8)));
        assert_eq!(l.editor, Rect::new(20, 0, 10, 8));
    }

    #[test]
    fn modal_is_centered() {
        let l = compute((100, 40), &UiConfig::default(), false, false, true);
        assert_eq!(l.modal, Some(Rect::new(15, 8, 70, 24)));
    }
}
