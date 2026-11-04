// 12-bitboard chess representation.
//
// One u64 per (color, piece) pair. Square indexing: a1 = 0, b1 = 1, ... h8 = 63
// (`square = rank * 8 + file`, rank 0 = white's first rank). The bit for square
// `s` is `1u64 << s`. This is the "Little-Endian Rank-File" mapping; it lets us
// shift north/south by ±8 and east/west by ±1.

use log::warn;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub fn flip(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Piece {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

#[derive(Debug, Clone)]
pub struct Bitboards {
    // [Color][Piece] → bitboard. Indexed via `idx_color`/`idx_piece` so we keep
    // a flat [[u64; 6]; 2] without bringing in a hashmap.
    pub boards: [[u64; 6]; 2],
    pub side_to_move: Color,
}

const fn idx_color(c: Color) -> usize {
    match c {
        Color::White => 0,
        Color::Black => 1,
    }
}

const fn idx_piece(p: Piece) -> usize {
    match p {
        Piece::Pawn => 0,
        Piece::Knight => 1,
        Piece::Bishop => 2,
        Piece::Rook => 3,
        Piece::Queen => 4,
        Piece::King => 5,
    }
}

pub fn square(file: u8, rank: u8) -> u8 {
    rank * 8 + file
}

pub fn bit(sq: u8) -> u64 {
    1u64 << sq
}

impl Bitboards {
    pub fn empty() -> Self {
        Self {
            boards: [[0u64; 6]; 2],
            side_to_move: Color::White,
        }
    }

    pub fn starting_position() -> Self {
        // Standard FEN — `from_fen` is the source of truth so we don't drift.
        Self::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w - - 0 1")
            .expect("starting FEN must parse")
    }

    /// Look up which (color, piece) sits on `sq`, or None for an empty square.
    pub fn piece_at(&self, sq: u8) -> Option<(Color, Piece)> {
        let mask = bit(sq);
        for &color in &[Color::White, Color::Black] {
            for &piece in &[
                Piece::Pawn,
                Piece::Knight,
                Piece::Bishop,
                Piece::Rook,
                Piece::Queen,
                Piece::King,
            ] {
                if self.boards[idx_color(color)][idx_piece(piece)] & mask != 0 {
                    return Some((color, piece));
                }
            }
        }
        None
    }

    pub fn board(&self, color: Color, piece: Piece) -> u64 {
        self.boards[idx_color(color)][idx_piece(piece)]
    }

    fn set(&mut self, color: Color, piece: Piece, sq: u8) {
        self.boards[idx_color(color)][idx_piece(piece)] |= bit(sq);
    }

    fn clear(&mut self, color: Color, piece: Piece, sq: u8) {
        self.boards[idx_color(color)][idx_piece(piece)] &= !bit(sq);
    }

    /// Clear whatever piece happens to occupy `sq` from any board. Used by
    /// captures where we don't know the captured piece type up front.
    fn clear_any(&mut self, sq: u8) {
        let mask = !bit(sq);
        for c in 0..2 {
            for p in 0..6 {
                self.boards[c][p] &= mask;
            }
        }
    }

    /// Parse the position field of a FEN string. Only the position + side-to-move
    /// fields are honoured — castling rights / en passant / clocks are ignored
    /// because the board widget only needs the layout. `from_fen` always sets
    /// `side_to_move` from the FEN (defaulting to White if absent) so that
    /// `apply_uci_moves` knows whose pawn is moving on the first ply.
    pub fn from_fen(fen: &str) -> Result<Self, String> {
        // Lichess streams the literal "startpos" for games that begin from the
        // standard initial position; treat it as such instead of falling
        // through to the rank-count error. (Variant and from-position games
        // always carry a real FEN, so they parse normally below.)
        if fen.trim() == "startpos" {
            return Ok(Self::starting_position());
        }

        let mut parts = fen.split_whitespace();
        let position = parts.next().ok_or_else(|| "empty FEN".to_string())?;
        let side = parts.next().unwrap_or("w");

        let mut bb = Bitboards::empty();
        let ranks: Vec<&str> = position.split('/').collect();
        if ranks.len() != 8 {
            return Err(format!("FEN must have 8 ranks, got {}", ranks.len()));
        }
        for (i, rank_str) in ranks.iter().enumerate() {
            // FEN's first rank substring is rank 8 (the top), so invert.
            let rank = 7 - i as u8;
            let mut file: u8 = 0;
            for ch in rank_str.chars() {
                if let Some(skip) = ch.to_digit(10) {
                    file += skip as u8;
                } else {
                    // Reject a 9th piece BEFORE placing it: square(8, rank)
                    // on the top rank is index 64, and bit(64) panics.
                    if file >= 8 {
                        return Err(format!("FEN rank '{}' overflows 8 files", rank_str));
                    }
                    let (color, piece) = char_to_piece(ch)
                        .ok_or_else(|| format!("FEN: unknown piece char '{}'", ch))?;
                    bb.set(color, piece, square(file, rank));
                    file += 1;
                }
                // Digit skips (e.g. a lone "9") overrun without ever calling
                // set, so catch them after the increment too.
                if file > 8 {
                    return Err(format!("FEN rank '{}' overflows 8 files", rank_str));
                }
            }
        }

        bb.side_to_move = match side {
            "w" => Color::White,
            "b" => Color::Black,
            other => return Err(format!("FEN: bad side-to-move '{}'", other)),
        };

        Ok(bb)
    }

    /// Apply each space-separated UCI move to `self`. Moves the side-to-move
    /// after each ply. Unknown / malformed moves are logged and skipped — the
    /// board may briefly be wrong but the next streamed snapshot will reset us
    /// (the screen rebuilds from `initial_fen` on every event).
    pub fn apply_uci_moves(&mut self, moves: &str) {
        for mv in moves.split_whitespace() {
            if let Err(e) = self.apply_uci_move(mv) {
                warn!("Skipping move '{}': {}", mv, e);
            }
        }
    }

    pub fn apply_uci_move(&mut self, mv: &str) -> Result<(), String> {
        let bytes = mv.as_bytes();
        if bytes.len() < 4 {
            return Err(format!("UCI move too short: '{}'", mv));
        }
        let from = parse_square(&bytes[0..2])?;
        let to = parse_square(&bytes[2..4])?;
        let promo = if bytes.len() >= 5 {
            Some(promotion_piece(bytes[4])?)
        } else {
            None
        };

        let (color, piece) = self
            .piece_at(from)
            .ok_or_else(|| format!("no piece on {}", square_name(from)))?;

        // Castling: detect by the king moving exactly two files. Move the rook
        // alongside; the regular from/to update below handles the king itself.
        if piece == Piece::King {
            let from_file = from % 8;
            let to_file = to % 8;
            let rank = from / 8;
            if to_file as i8 - from_file as i8 == 2 {
                // O-O: rook h→f
                let rook_from = square(7, rank);
                let rook_to = square(5, rank);
                self.clear(color, Piece::Rook, rook_from);
                self.set(color, Piece::Rook, rook_to);
            } else if to_file as i8 - from_file as i8 == -2 {
                // O-O-O: rook a→d
                let rook_from = square(0, rank);
                let rook_to = square(3, rank);
                self.clear(color, Piece::Rook, rook_from);
                self.set(color, Piece::Rook, rook_to);
            }
        }

        // En passant: a pawn moves diagonally onto an empty square. The captured
        // pawn is on the same file as `to` but one rank back (relative to the
        // moving side). Detected before we clear `to` so we don't mistake a
        // normal capture for EP.
        if piece == Piece::Pawn && (from % 8) != (to % 8) && self.piece_at(to).is_none() {
            let captured_rank = if color == Color::White {
                to / 8 - 1
            } else {
                to / 8 + 1
            };
            let captured_sq = square(to % 8, captured_rank);
            self.clear(color.flip(), Piece::Pawn, captured_sq);
        }

        // Normal move/capture: vacate `from`, blank `to` (handles all captures
        // including EP-mismatch above), then place the piece (or its promotion).
        self.clear(color, piece, from);
        self.clear_any(to);
        let placed = promo.unwrap_or(piece);
        self.set(color, placed, to);

        self.side_to_move = self.side_to_move.flip();
        Ok(())
    }
}

fn char_to_piece(ch: char) -> Option<(Color, Piece)> {
    let color = if ch.is_ascii_uppercase() {
        Color::White
    } else {
        Color::Black
    };
    let piece = match ch.to_ascii_lowercase() {
        'p' => Piece::Pawn,
        'n' => Piece::Knight,
        'b' => Piece::Bishop,
        'r' => Piece::Rook,
        'q' => Piece::Queen,
        'k' => Piece::King,
        _ => return None,
    };
    Some((color, piece))
}

fn parse_square(b: &[u8]) -> Result<u8, String> {
    if b.len() != 2 {
        return Err(format!("bad square len {}", b.len()));
    }
    let file = b[0];
    let rank = b[1];
    if !(b'a'..=b'h').contains(&file) || !(b'1'..=b'8').contains(&rank) {
        return Err(format!("bad square '{}{}'", file as char, rank as char));
    }
    Ok(square(file - b'a', rank - b'1'))
}

fn promotion_piece(b: u8) -> Result<Piece, String> {
    match b {
        b'q' => Ok(Piece::Queen),
        b'r' => Ok(Piece::Rook),
        b'b' => Ok(Piece::Bishop),
        b'n' => Ok(Piece::Knight),
        other => Err(format!("bad promotion '{}'", other as char)),
    }
}

fn square_name(sq: u8) -> String {
    let file = (b'a' + (sq % 8)) as char;
    let rank = (b'1' + (sq / 8)) as char;
    format!("{}{}", file, rank)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starting_position_layout() {
        let bb = Bitboards::starting_position();
        assert_eq!(bb.piece_at(square(0, 0)), Some((Color::White, Piece::Rook)));
        assert_eq!(bb.piece_at(square(4, 0)), Some((Color::White, Piece::King)));
        assert_eq!(bb.piece_at(square(4, 7)), Some((Color::Black, Piece::King)));
        assert_eq!(bb.piece_at(square(3, 3)), None);
        assert_eq!(bb.side_to_move, Color::White);
    }

    #[test]
    fn pawn_double_then_capture() {
        let mut bb = Bitboards::starting_position();
        bb.apply_uci_moves("e2e4 d7d5 e4d5");
        assert_eq!(bb.piece_at(square(3, 4)), Some((Color::White, Piece::Pawn)));
        assert_eq!(bb.piece_at(square(4, 3)), None);
    }

    #[test]
    fn kingside_castle() {
        let mut bb =
            Bitboards::from_fen("rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 1")
                .unwrap();
        bb.apply_uci_move("f1c4").unwrap();
        bb.apply_uci_move("f8c5").unwrap();
        bb.apply_uci_move("e1g1").unwrap();
        assert_eq!(bb.piece_at(square(6, 0)), Some((Color::White, Piece::King)));
        assert_eq!(bb.piece_at(square(5, 0)), Some((Color::White, Piece::Rook)));
        assert_eq!(bb.piece_at(square(7, 0)), None);
    }

    #[test]
    fn en_passant_capture() {
        let mut bb = Bitboards::from_fen("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1").unwrap();
        bb.apply_uci_move("e5d6").unwrap();
        assert_eq!(bb.piece_at(square(3, 5)), Some((Color::White, Piece::Pawn)));
        assert_eq!(bb.piece_at(square(3, 4)), None); // captured pawn gone
    }

    #[test]
    fn promotion_to_queen() {
        let mut bb = Bitboards::from_fen("4k3/P7/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        bb.apply_uci_move("a7a8q").unwrap();
        assert_eq!(
            bb.piece_at(square(0, 7)),
            Some((Color::White, Piece::Queen))
        );
    }

    // ========================================================================
    // FEN Parsing Tests
    // ========================================================================

    #[test]
    fn fen_parsing_empty_board() {
        let bb = Bitboards::from_fen("8/8/8/8/8/8/8/8 w - - 0 1").unwrap();
        for sq in 0..64 {
            assert_eq!(bb.piece_at(sq), None, "Square {} should be empty", sq);
        }
        assert_eq!(bb.side_to_move, Color::White);
    }

    #[test]
    fn fen_parsing_single_piece() {
        let bb = Bitboards::from_fen("8/8/8/8/8/8/8/K7 w - - 0 1").unwrap();
        assert_eq!(bb.piece_at(square(0, 0)), Some((Color::White, Piece::King)));
        assert_eq!(bb.side_to_move, Color::White);
    }

    #[test]
    fn fen_parsing_black_to_move() {
        let bb =
            Bitboards::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b - - 0 1").unwrap();
        assert_eq!(bb.side_to_move, Color::Black);
    }

    #[test]
    fn fen_parsing_startpos_alias() {
        let bb1 = Bitboards::from_fen("startpos").unwrap();
        let bb2 = Bitboards::starting_position();
        assert_eq!(bb1.boards, bb2.boards);
        assert_eq!(bb1.side_to_move, bb2.side_to_move);
    }

    #[test]
    fn fen_parsing_invalid_rank_count() {
        let result = Bitboards::from_fen("8/8/8/8/8/8");
        assert!(result.is_err(), "Should fail with wrong rank count");
    }

    #[test]
    fn fen_parsing_unknown_piece() {
        let result = Bitboards::from_fen("8/8/8/8/8/8/8/X7 w - - 0 1");
        assert!(result.is_err(), "Should fail with unknown piece 'X'");
    }

    #[test]
    fn fen_parsing_rank_overflow() {
        // Digit overflow: rank 8 has "9" empty squares, one past the 8-file
        // limit. from_fen rejects it once the ninth file is counted.
        let digit = Bitboards::from_fen("9/8/8/8/8/8/8/8 w - - 0 1");
        assert!(digit.is_err(), "Should fail: digit overflows 8 files");

        // Piece overflow on the TOP rank: a 9th piece would be placed at
        // square(8, 7) == 64, and bit(64) panics. Must return Err, not panic.
        let piece = Bitboards::from_fen("8R/8/8/8/8/8/8/8 w - - 0 1");
        assert!(piece.is_err(), "Should fail: piece overflows 8 files");
    }

    #[test]
    fn fen_parsing_invalid_side_to_move() {
        let result = Bitboards::from_fen("8/8/8/8/8/8/8/8 x - - 0 1");
        assert!(result.is_err(), "Should fail with invalid side 'x'");
    }

    #[test]
    fn fen_parsing_complex_position() {
        let bb =
            Bitboards::from_fen("r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 1")
                .unwrap();
        assert_eq!(bb.piece_at(square(4, 0)), Some((Color::White, Piece::King)));
        assert_eq!(bb.piece_at(square(4, 7)), Some((Color::Black, Piece::King)));
        assert_eq!(
            bb.piece_at(square(5, 2)),
            Some((Color::White, Piece::Knight))
        );
        assert_eq!(
            bb.piece_at(square(2, 5)),
            Some((Color::Black, Piece::Knight))
        );
    }

    // ========================================================================
    // Piece Movement Tests
    // ========================================================================

    #[test]
    fn piece_at_all_white_pieces() {
        let bb = Bitboards::starting_position();
        assert_eq!(bb.piece_at(square(0, 0)), Some((Color::White, Piece::Rook)));
        assert_eq!(
            bb.piece_at(square(1, 0)),
            Some((Color::White, Piece::Knight))
        );
        assert_eq!(
            bb.piece_at(square(2, 0)),
            Some((Color::White, Piece::Bishop))
        );
        assert_eq!(
            bb.piece_at(square(3, 0)),
            Some((Color::White, Piece::Queen))
        );
        assert_eq!(bb.piece_at(square(4, 0)), Some((Color::White, Piece::King)));
        assert_eq!(
            bb.piece_at(square(5, 0)),
            Some((Color::White, Piece::Bishop))
        );
        assert_eq!(
            bb.piece_at(square(6, 0)),
            Some((Color::White, Piece::Knight))
        );
        assert_eq!(bb.piece_at(square(7, 0)), Some((Color::White, Piece::Rook)));
    }

    #[test]
    fn piece_at_all_white_pawns() {
        let bb = Bitboards::starting_position();
        for file in 0..8 {
            assert_eq!(
                bb.piece_at(square(file, 1)),
                Some((Color::White, Piece::Pawn))
            );
        }
    }

    #[test]
    fn piece_at_all_black_pieces() {
        let bb = Bitboards::starting_position();
        assert_eq!(bb.piece_at(square(0, 7)), Some((Color::Black, Piece::Rook)));
        assert_eq!(
            bb.piece_at(square(1, 7)),
            Some((Color::Black, Piece::Knight))
        );
        assert_eq!(
            bb.piece_at(square(2, 7)),
            Some((Color::Black, Piece::Bishop))
        );
        assert_eq!(
            bb.piece_at(square(3, 7)),
            Some((Color::Black, Piece::Queen))
        );
        assert_eq!(bb.piece_at(square(4, 7)), Some((Color::Black, Piece::King)));
    }

    #[test]
    fn board_method_white_pawns() {
        let bb = Bitboards::starting_position();
        let white_pawns = bb.board(Color::White, Piece::Pawn);
        // White pawns are on rank 1 (second rank), which is squares 8-15
        // bit 8 = 1 << 8 = 256 = 0x100
        // bit 15 = 1 << 15 = 32768 = 0x8000
        // So we need bits 8-15 set: 0x000000000000FF00
        let expected: u64 = 0x000000000000FF00;
        assert_eq!(white_pawns, expected);
    }

    // ========================================================================
    // UCI Move Application Tests
    // ========================================================================

    #[test]
    fn uci_simple_pawn_move() {
        let mut bb =
            Bitboards::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w - - 0 1").unwrap();
        bb.apply_uci_move("e2e4").unwrap();
        assert_eq!(bb.piece_at(square(4, 1)), None);
        assert_eq!(bb.piece_at(square(4, 3)), Some((Color::White, Piece::Pawn)));
    }

    #[test]
    fn uci_pawn_capture() {
        let mut bb = Bitboards::from_fen("rnbqkbnr/pppp1ppp/8/4p3/4P3/8/8/8 w - - 0 1").unwrap();
        bb.apply_uci_move("e4d5").unwrap();
        assert_eq!(bb.piece_at(square(4, 3)), None);
        assert_eq!(bb.piece_at(square(3, 4)), Some((Color::White, Piece::Pawn)));
    }

    #[test]
    fn uci_knight_move() {
        let mut bb =
            Bitboards::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w - - 0 1").unwrap();
        bb.apply_uci_move("g1f3").unwrap();
        assert_eq!(bb.piece_at(square(6, 0)), None);
        assert_eq!(
            bb.piece_at(square(5, 2)),
            Some((Color::White, Piece::Knight))
        );
    }

    #[test]
    fn uci_bishop_move() {
        let mut bb =
            Bitboards::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w - - 0 1").unwrap();
        bb.apply_uci_move("c1a3").unwrap();
        assert_eq!(bb.piece_at(square(2, 0)), None);
        assert_eq!(
            bb.piece_at(square(0, 2)),
            Some((Color::White, Piece::Bishop))
        );
    }

    #[test]
    fn uci_rook_move() {
        let mut bb =
            Bitboards::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w - - 0 1").unwrap();
        bb.apply_uci_move("a1a3").unwrap();
        assert_eq!(bb.piece_at(square(0, 0)), None);
        assert_eq!(bb.piece_at(square(0, 2)), Some((Color::White, Piece::Rook)));
    }

    #[test]
    fn uci_queen_move() {
        let mut bb =
            Bitboards::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w - - 0 1").unwrap();
        bb.apply_uci_move("d1d3").unwrap();
        assert_eq!(bb.piece_at(square(3, 0)), None);
        assert_eq!(
            bb.piece_at(square(3, 2)),
            Some((Color::White, Piece::Queen))
        );
    }

    #[test]
    fn uci_king_move() {
        let mut bb =
            Bitboards::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w - - 0 1").unwrap();
        bb.apply_uci_move("e1e2").unwrap();
        assert_eq!(bb.piece_at(square(4, 0)), None);
        assert_eq!(bb.piece_at(square(4, 1)), Some((Color::White, Piece::King)));
    }

    // ========================================================================
    // Special Moves Tests
    // ========================================================================

    #[test]
    fn uci_queenside_castle() {
        let mut bb =
            Bitboards::from_fen("r3kbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/R3KB1R w KQkq - 0 1")
                .unwrap();
        bb.apply_uci_move("e1c1").unwrap();
        assert_eq!(bb.piece_at(square(4, 0)), None);
        assert_eq!(bb.piece_at(square(2, 0)), Some((Color::White, Piece::King)));
        assert_eq!(bb.piece_at(square(0, 0)), None);
        assert_eq!(bb.piece_at(square(3, 0)), Some((Color::White, Piece::Rook)));
    }

    #[test]
    fn uci_queenside_castle_clear_path() {
        // apply_uci_move does no legality check, so it cannot reject a blocked
        // castle — see follow-up task t16 (obstruction validation). This test
        // covers what the impl guarantees today: a clear-path O-O-O succeeds.
        let mut bb =
            Bitboards::from_fen("r3kbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/R3KB1R w KQkq - 0 1")
                .unwrap();
        let result = bb.apply_uci_move("e1c1");
        assert!(
            result.is_ok(),
            "Clear-path queenside castling should succeed"
        );
    }

    #[test]
    fn uci_promotion_to_knight() {
        let mut bb = Bitboards::from_fen("4k3/P7/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        bb.apply_uci_move("a7a8n").unwrap();
        assert_eq!(
            bb.piece_at(square(0, 7)),
            Some((Color::White, Piece::Knight))
        );
    }

    #[test]
    fn uci_promotion_to_rook() {
        let mut bb = Bitboards::from_fen("4k3/P7/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        bb.apply_uci_move("a7a8r").unwrap();
        assert_eq!(bb.piece_at(square(0, 7)), Some((Color::White, Piece::Rook)));
    }

    #[test]
    fn uci_promotion_to_bishop() {
        let mut bb = Bitboards::from_fen("4k3/P7/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        bb.apply_uci_move("a7a8b").unwrap();
        assert_eq!(
            bb.piece_at(square(0, 7)),
            Some((Color::White, Piece::Bishop))
        );
    }

    // ========================================================================
    // Edge Cases Tests
    // ========================================================================

    #[test]
    fn uci_move_from_empty_square() {
        let mut bb = Bitboards::from_fen("8/8/8/8/8/8/8/8 w - - 0 1").unwrap();
        let result = bb.apply_uci_move("e2e4");
        assert!(result.is_err(), "Should fail moving from empty square");
    }

    #[test]
    fn uci_invalid_format_too_short() {
        let mut bb = Bitboards::starting_position();
        let result = bb.apply_uci_move("e2");
        assert!(result.is_err(), "Should fail with move too short");
    }

    #[test]
    fn uci_invalid_square() {
        let mut bb = Bitboards::starting_position();
        let result = bb.apply_uci_move("i1i2");
        assert!(result.is_err(), "Should fail with invalid square 'i1'");
    }

    #[test]
    fn square_indexing() {
        assert_eq!(square(0, 0), 0);
        assert_eq!(square(7, 0), 7);
        assert_eq!(square(0, 7), 56);
        assert_eq!(square(7, 7), 63);
        assert_eq!(square(4, 3), 28);
    }

    #[test]
    fn bit_function() {
        assert_eq!(bit(0), 1u64 << 0);
        assert_eq!(bit(7), 1u64 << 7);
        assert_eq!(bit(56), 1u64 << 56);
        assert_eq!(bit(63), 1u64 << 63);
    }

    #[test]
    fn color_flip() {
        assert_eq!(Color::White.flip(), Color::Black);
        assert_eq!(Color::Black.flip(), Color::White);
    }

    // ========================================================================
    // Helper Function Tests
    // ========================================================================

    #[test]
    fn char_to_piece_all_pieces() {
        assert_eq!(char_to_piece('P'), Some((Color::White, Piece::Pawn)));
        assert_eq!(char_to_piece('N'), Some((Color::White, Piece::Knight)));
        assert_eq!(char_to_piece('B'), Some((Color::White, Piece::Bishop)));
        assert_eq!(char_to_piece('R'), Some((Color::White, Piece::Rook)));
        assert_eq!(char_to_piece('Q'), Some((Color::White, Piece::Queen)));
        assert_eq!(char_to_piece('K'), Some((Color::White, Piece::King)));

        assert_eq!(char_to_piece('p'), Some((Color::Black, Piece::Pawn)));
        assert_eq!(char_to_piece('n'), Some((Color::Black, Piece::Knight)));
        assert_eq!(char_to_piece('b'), Some((Color::Black, Piece::Bishop)));
        assert_eq!(char_to_piece('r'), Some((Color::Black, Piece::Rook)));
        assert_eq!(char_to_piece('q'), Some((Color::Black, Piece::Queen)));
        assert_eq!(char_to_piece('k'), Some((Color::Black, Piece::King)));

        assert_eq!(char_to_piece('X'), None);
        assert_eq!(char_to_piece(' '), None);
    }

    #[test]
    fn parse_square_all_squares() {
        for file in 0..8 {
            for rank in 0..8 {
                let sq = square(file, rank);
                let file_char = (b'a' + file) as char;
                let rank_char = (b'1' + rank) as char;
                let square_str = format!("{}{}", file_char, rank_char);
                let parsed = parse_square(square_str.as_bytes()).unwrap();
                assert_eq!(parsed, sq, "Failed to parse {}", square_str);
            }
        }
    }

    #[test]
    fn parse_square_invalid() {
        assert!(parse_square(b"i1").is_err());
        assert!(parse_square(b"a9").is_err());
        assert!(parse_square(b"").is_err());
        assert!(parse_square(b"a").is_err());
    }
}
