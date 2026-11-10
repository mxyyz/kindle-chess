//! Display dimensions and shared layout constants for the 1072×1448
//! Kindle Paperwhite 4 screen. All magic numbers that describe the screen
//! or shared widget geometry live here; per-screen one-off positions stay
//! literal in their screen files.

/// Screen width in pixels.
pub const SCREEN_W: i16 = 1072;
/// Screen width as u16 (for Rectangle fields).
pub const SCREEN_W_U16: u16 = 1072;
/// Screen height in pixels.
pub const SCREEN_H: i16 = 1448;
/// Screen height as u16 (for Rectangle fields).
pub const SCREEN_H_U16: u16 = 1448;

/// Horizontal centre of the screen.
pub const CENTER_X: i16 = SCREEN_W / 2;
/// Vertical centre of the screen.
pub const CENTER_Y: i16 = SCREEN_H / 2;

/// Board area is a square: SCREEN_W × SCREEN_W.
pub const BOARD_SIZE: u16 = SCREEN_W_U16;

/// One chess board cell (board is 8×8).
pub const SQUARE_SIZE: u16 = SCREEN_W_U16 / 8;

/// Sidebar starts below the board.
pub const SIDEBAR_Y: i16 = SCREEN_W;
/// Sidebar width = full screen width.
pub const SIDEBAR_W: u16 = SCREEN_W_U16;
/// Sidebar height = screen height − board height.
pub const SIDEBAR_H: u16 = SCREEN_H_U16 - SCREEN_W_U16;

/// Icon button footprint = one chess cell.
pub const ICON_BUTTON_SIZE: u16 = SQUARE_SIZE;

/// Default button width used by most screens.
pub const BTN_W: u16 = 600;
/// Default button height used by most screens.
pub const BTN_H: u16 = 96;
/// Vertical gap between stacked buttons.
pub const BTN_GAP: i16 = 28;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_dimensions_match_paperwhite4() {
        assert_eq!(SCREEN_W, 1072);
        assert_eq!(SCREEN_H, 1448);
    }

    #[test]
    fn square_size_is_eighth_of_screen() {
        assert_eq!(SQUARE_SIZE, 134);
        assert_eq!(SQUARE_SIZE * 8, SCREEN_W_U16);
    }

    #[test]
    fn sidebar_fills_remainder_below_board() {
        assert_eq!(SIDEBAR_Y as u16 + SIDEBAR_H, SCREEN_H_U16);
        assert_eq!(SIDEBAR_W, SCREEN_W_U16);
    }

    #[test]
    fn icon_button_equals_square() {
        assert_eq!(ICON_BUTTON_SIZE, SQUARE_SIZE);
    }

    #[test]
    fn center_is_half() {
        assert_eq!(CENTER_X, SCREEN_W / 2);
        assert_eq!(CENTER_Y, SCREEN_H / 2);
    }
}
