use super::{Bias, DisplayPoint, DisplaySnapshot, SelectionGoal, ToDisplayPoint};
use crate::{char_kind, CharKind, EditorStyle, ToOffset, ToPoint};
use gpui::{px, Pixels, TextSystem};
use language::Point;
use serde::de::IntoDeserializer;
use std::{ops::Range, sync::Arc};

#[derive(Debug, PartialEq)]
pub enum FindRange {
    SingleLine,
    MultiLine,
}

/// TextLayoutDetails encompasses everything we need to move vertically
/// taking into account variable width characters.
pub struct TextLayoutDetails {
    pub text_system: Arc<TextSystem>,
    pub editor_style: EditorStyle,
    pub rem_size: Pixels,
}

pub fn left(map: &DisplaySnapshot, mut point: DisplayPoint) -> DisplayPoint {
    if point.column() > 0 {
        *point.column_mut() -= 1;
    } else if point.row() > 0 {
        *point.row_mut() -= 1;
        *point.column_mut() = map.line_len(point.row());
    }
    map.clip_point(point, Bias::Left)
}

pub fn saturating_left(map: &DisplaySnapshot, mut point: DisplayPoint) -> DisplayPoint {
    if point.column() > 0 {
        *point.column_mut() -= 1;
    }
    map.clip_point(point, Bias::Left)
}

pub fn right(map: &DisplaySnapshot, mut point: DisplayPoint) -> DisplayPoint {
    let max_column = map.line_len(point.row());
    if point.column() < max_column {
        *point.column_mut() += 1;
    } else if point.row() < map.max_point().row() {
        *point.row_mut() += 1;
        *point.column_mut() = 0;
    }
    map.clip_point(point, Bias::Right)
}

pub fn saturating_right(map: &DisplaySnapshot, mut point: DisplayPoint) -> DisplayPoint {
    *point.column_mut() += 1;
    map.clip_point(point, Bias::Right)
}

pub fn up(
    map: &DisplaySnapshot,
    start: DisplayPoint,
    goal: SelectionGoal,
    preserve_column_at_start: bool,
    text_layout_details: &TextLayoutDetails,
) -> (DisplayPoint, SelectionGoal) {
    up_by_rows(
        map,
        start,
        1,
        goal,
        preserve_column_at_start,
        text_layout_details,
    )
}

pub fn down(
    map: &DisplaySnapshot,
    start: DisplayPoint,
    goal: SelectionGoal,
    preserve_column_at_end: bool,
    text_layout_details: &TextLayoutDetails,
) -> (DisplayPoint, SelectionGoal) {
    down_by_rows(
        map,
        start,
        1,
        goal,
        preserve_column_at_end,
        text_layout_details,
    )
}

pub fn up_by_rows(
    map: &DisplaySnapshot,
    start: DisplayPoint,
    row_count: u32,
    goal: SelectionGoal,
    preserve_column_at_start: bool,
    text_layout_details: &TextLayoutDetails,
) -> (DisplayPoint, SelectionGoal) {
    let mut goal_x = match goal {
        SelectionGoal::HorizontalPosition(x) => x.into(), // todo!("Can the fields in SelectionGoal by Pixels? We should extract a geometry crate and depend on that.")
        SelectionGoal::WrappedHorizontalPosition((_, x)) => x.into(),
        SelectionGoal::HorizontalRange { end, .. } => end.into(),
        _ => map.x_for_display_point(start, text_layout_details),
    };

    let prev_row = start.row().saturating_sub(row_count);
    let mut point = map.clip_point(
        DisplayPoint::new(prev_row, map.line_len(prev_row)),
        Bias::Left,
    );
    if point.row() < start.row() {
        *point.column_mut() = map.display_column_for_x(point.row(), goal_x, text_layout_details)
    } else if preserve_column_at_start {
        return (start, goal);
    } else {
        point = DisplayPoint::new(0, 0);
        goal_x = px(0.);
    }

    let mut clipped_point = map.clip_point(point, Bias::Left);
    if clipped_point.row() < point.row() {
        clipped_point = map.clip_point(point, Bias::Right);
    }
    (
        clipped_point,
        SelectionGoal::HorizontalPosition(goal_x.into()),
    )
}

pub fn down_by_rows(
    map: &DisplaySnapshot,
    start: DisplayPoint,
    row_count: u32,
    goal: SelectionGoal,
    preserve_column_at_end: bool,
    text_layout_details: &TextLayoutDetails,
) -> (DisplayPoint, SelectionGoal) {
    let mut goal_x = match goal {
        SelectionGoal::HorizontalPosition(x) => x.into(),
        SelectionGoal::WrappedHorizontalPosition((_, x)) => x.into(),
        SelectionGoal::HorizontalRange { end, .. } => end.into(),
        _ => map.x_for_display_point(start, text_layout_details),
    };

    let new_row = start.row() + row_count;
    let mut point = map.clip_point(DisplayPoint::new(new_row, 0), Bias::Right);
    if point.row() > start.row() {
        *point.column_mut() = map.display_column_for_x(point.row(), goal_x, text_layout_details)
    } else if preserve_column_at_end {
        return (start, goal);
    } else {
        point = map.max_point();
        goal_x = map.x_for_display_point(point, text_layout_details)
    }

    let mut clipped_point = map.clip_point(point, Bias::Right);
    if clipped_point.row() > point.row() {
        clipped_point = map.clip_point(point, Bias::Left);
    }
    (
        clipped_point,
        SelectionGoal::HorizontalPosition(goal_x.into()),
    )
}

pub fn line_beginning(
    map: &DisplaySnapshot,
    display_point: DisplayPoint,
    stop_at_soft_boundaries: bool,
) -> DisplayPoint {
    let point = display_point.to_point(map);
    let soft_line_start = map.clip_point(DisplayPoint::new(display_point.row(), 0), Bias::Right);
    let line_start = map.prev_line_boundary(point).1;

    if stop_at_soft_boundaries && display_point != soft_line_start {
        soft_line_start
    } else {
        line_start
    }
}

pub fn indented_line_beginning(
    map: &DisplaySnapshot,
    display_point: DisplayPoint,
    stop_at_soft_boundaries: bool,
) -> DisplayPoint {
    let point = display_point.to_point(map);
    let soft_line_start = map.clip_point(DisplayPoint::new(display_point.row(), 0), Bias::Right);
    let indent_start = Point::new(
        point.row,
        map.buffer_snapshot.indent_size_for_line(point.row).len,
    )
    .to_display_point(map);
    let line_start = map.prev_line_boundary(point).1;

    if stop_at_soft_boundaries && soft_line_start > indent_start && display_point != soft_line_start
    {
        soft_line_start
    } else if stop_at_soft_boundaries && display_point != indent_start {
        indent_start
    } else {
        line_start
    }
}

pub fn line_end(
    map: &DisplaySnapshot,
    display_point: DisplayPoint,
    stop_at_soft_boundaries: bool,
) -> DisplayPoint {
    let soft_line_end = map.clip_point(
        DisplayPoint::new(display_point.row(), map.line_len(display_point.row())),
        Bias::Left,
    );
    if stop_at_soft_boundaries && display_point != soft_line_end {
        soft_line_end
    } else {
        map.next_line_boundary(display_point.to_point(map)).1
    }
}

pub fn previous_word_start(map: &DisplaySnapshot, point: DisplayPoint) -> DisplayPoint {
    let raw_point = point.to_point(map);
    let scope = map.buffer_snapshot.language_scope_at(raw_point);

    find_preceding_boundary(map, point, FindRange::MultiLine, |left, right| {
        (char_kind(&scope, left) != char_kind(&scope, right) && !right.is_whitespace())
            || left == '\n'
    })
}

pub fn previous_subword_start(map: &DisplaySnapshot, point: DisplayPoint) -> DisplayPoint {
    let raw_point = point.to_point(map);
    let scope = map.buffer_snapshot.language_scope_at(raw_point);

    find_preceding_boundary(map, point, FindRange::MultiLine, |left, right| {
        let is_word_start =
            char_kind(&scope, left) != char_kind(&scope, right) && !right.is_whitespace();
        let is_subword_start =
            left == '_' && right != '_' || left.is_lowercase() && right.is_uppercase();
        is_word_start || is_subword_start || left == '\n'
    })
}

pub fn next_word_end(map: &DisplaySnapshot, point: DisplayPoint) -> DisplayPoint {
    let raw_point = point.to_point(map);
    let scope = map.buffer_snapshot.language_scope_at(raw_point);

    find_boundary(map, point, FindRange::MultiLine, |left, right| {
        (char_kind(&scope, left) != char_kind(&scope, right) && !left.is_whitespace())
            || right == '\n'
    })
}

pub fn next_subword_end(map: &DisplaySnapshot, point: DisplayPoint) -> DisplayPoint {
    let raw_point = point.to_point(map);
    let scope = map.buffer_snapshot.language_scope_at(raw_point);

    find_boundary(map, point, FindRange::MultiLine, |left, right| {
        let is_word_end =
            (char_kind(&scope, left) != char_kind(&scope, right)) && !left.is_whitespace();
        let is_subword_end =
            left != '_' && right == '_' || left.is_lowercase() && right.is_uppercase();
        is_word_end || is_subword_end || right == '\n'
    })
}

pub fn start_of_paragraph(
    map: &DisplaySnapshot,
    display_point: DisplayPoint,
    mut count: usize,
) -> DisplayPoint {
    let point = display_point.to_point(map);
    if point.row == 0 {
        return DisplayPoint::zero();
    }

    let mut found_non_blank_line = false;
    for row in (0..point.row + 1).rev() {
        let blank = map.buffer_snapshot.is_line_blank(row);
        if found_non_blank_line && blank {
            if count <= 1 {
                return Point::new(row, 0).to_display_point(map);
            }
            count -= 1;
            found_non_blank_line = false;
        }

        found_non_blank_line |= !blank;
    }

    DisplayPoint::zero()
}

pub fn end_of_paragraph(
    map: &DisplaySnapshot,
    display_point: DisplayPoint,
    mut count: usize,
) -> DisplayPoint {
    let point = display_point.to_point(map);
    if point.row == map.max_buffer_row() {
        return map.max_point();
    }

    let mut found_non_blank_line = false;
    for row in point.row..map.max_buffer_row() + 1 {
        let blank = map.buffer_snapshot.is_line_blank(row);
        if found_non_blank_line && blank {
            if count <= 1 {
                return Point::new(row, 0).to_display_point(map);
            }
            count -= 1;
            found_non_blank_line = false;
        }

        found_non_blank_line |= !blank;
    }

    map.max_point()
}

/// Scans for a boundary preceding the given start point `from` until a boundary is found,
/// indicated by the given predicate returning true.
/// The predicate is called with the character to the left and right of the candidate boundary location.
/// If FindRange::SingleLine is specified and no boundary is found before the start of the current line, the start of the current line will be returned.
pub fn find_preceding_boundary(
    map: &DisplaySnapshot,
    from: DisplayPoint,
    find_range: FindRange,
    mut is_boundary: impl FnMut(char, char) -> bool,
) -> DisplayPoint {
    let mut prev_ch = None;
    let mut offset = from.to_point(map).to_offset(&map.buffer_snapshot);

    for ch in map.buffer_snapshot.reversed_chars_at(offset) {
        if find_range == FindRange::SingleLine && ch == '\n' {
            break;
        }
        if let Some(prev_ch) = prev_ch {
            if is_boundary(ch, prev_ch) {
                break;
            }
        }

        offset -= ch.len_utf8();
        prev_ch = Some(ch);
    }

    map.clip_point(offset.to_display_point(map), Bias::Left)
}

/// Scans for a boundary following the given start point until a boundary is found, indicated by the
/// given predicate returning true. The predicate is called with the character to the left and right
/// of the candidate boundary location, and will be called with `\n` characters indicating the start
/// or end of a line.
pub fn find_boundary(
    map: &DisplaySnapshot,
    from: DisplayPoint,
    find_range: FindRange,
    mut is_boundary: impl FnMut(char, char) -> bool,
) -> DisplayPoint {
    let mut offset = from.to_offset(&map, Bias::Right);
    let mut prev_ch = None;

    for ch in map.buffer_snapshot.chars_at(offset) {
        if find_range == FindRange::SingleLine && ch == '\n' {
            break;
        }
        if let Some(prev_ch) = prev_ch {
            if is_boundary(prev_ch, ch) {
                break;
            }
        }

        offset += ch.len_utf8();
        prev_ch = Some(ch);
    }
    map.clip_point(offset.to_display_point(map), Bias::Right)
}

pub fn chars_after(
    map: &DisplaySnapshot,
    mut offset: usize,
) -> impl Iterator<Item = (char, Range<usize>)> + '_ {
    map.buffer_snapshot.chars_at(offset).map(move |ch| {
        let before = offset;
        offset = offset + ch.len_utf8();
        (ch, before..offset)
    })
}

pub fn chars_before(
    map: &DisplaySnapshot,
    mut offset: usize,
) -> impl Iterator<Item = (char, Range<usize>)> + '_ {
    map.buffer_snapshot
        .reversed_chars_at(offset)
        .map(move |ch| {
            let after = offset;
            offset = offset - ch.len_utf8();
            (ch, offset..after)
        })
}

pub fn is_inside_word(map: &DisplaySnapshot, point: DisplayPoint) -> bool {
    let raw_point = point.to_point(map);
    let scope = map.buffer_snapshot.language_scope_at(raw_point);
    let ix = map.clip_point(point, Bias::Left).to_offset(map, Bias::Left);
    let text = &map.buffer_snapshot;
    let next_char_kind = text.chars_at(ix).next().map(|c| char_kind(&scope, c));
    let prev_char_kind = text
        .reversed_chars_at(ix)
        .next()
        .map(|c| char_kind(&scope, c));
    prev_char_kind.zip(next_char_kind) == Some((CharKind::Word, CharKind::Word))
}

pub fn surrounding_word(map: &DisplaySnapshot, position: DisplayPoint) -> Range<DisplayPoint> {
    let position = map
        .clip_point(position, Bias::Left)
        .to_offset(map, Bias::Left);
    let (range, _) = map.buffer_snapshot.surrounding_word(position);
    let start = range
        .start
        .to_point(&map.buffer_snapshot)
        .to_display_point(map);
    let end = range
        .end
        .to_point(&map.buffer_snapshot)
        .to_display_point(map);
    start..end
}

pub fn split_display_range_by_lines(
    map: &DisplaySnapshot,
    range: Range<DisplayPoint>,
) -> Vec<Range<DisplayPoint>> {
    let mut result = Vec::new();

    let mut start = range.start;
    // Loop over all the covered rows until the one containing the range end
    for row in range.start.row()..range.end.row() {
        let row_end_column = map.line_len(row);
        let end = map.clip_point(DisplayPoint::new(row, row_end_column), Bias::Left);
        if start != end {
            result.push(start..end);
        }
        start = map.clip_point(DisplayPoint::new(row + 1, 0), Bias::Left);
    }

    // Add the final range from the start of the last end to the original range end.
    result.push(start..range.end);

    result
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::{
//         display_map::Inlay,
//         test::{},
//         Buffer, DisplayMap, ExcerptRange, InlayId, MultiBuffer,
//     };
//     use project::Project;
//     use settings::SettingsStore;
//     use util::post_inc;

//     #[gpui::test]
//     fn test_previous_word_start(cx: &mut gpui::AppContext) {
//         init_test(cx);

//         fn assert(marked_text: &str, cx: &mut gpui::AppContext) {
//             let (snapshot, display_points) = marked_display_snapshot(marked_text, cx);
//             assert_eq!(
//                 previous_word_start(&snapshot, display_points[1]),
//                 display_points[0]
//             );
//         }

//         assert("\nˇ   ˇlorem", cx);
//         assert("ˇ\nˇ   lorem", cx);
//         assert("    ˇloremˇ", cx);
//         assert("ˇ    ˇlorem", cx);
//         assert("    ˇlorˇem", cx);
//         assert("\nlorem\nˇ   ˇipsum", cx);
//         assert("\n\nˇ\nˇ", cx);
//         assert("    ˇlorem  ˇipsum", cx);
//         assert("loremˇ-ˇipsum", cx);
//         assert("loremˇ-#$@ˇipsum", cx);
//         assert("ˇlorem_ˇipsum", cx);
//         assert(" ˇdefγˇ", cx);
//         assert(" ˇbcΔˇ", cx);
//         assert(" abˇ——ˇcd", cx);
//     }

//     #[gpui::test]
//     fn test_previous_subword_start(cx: &mut gpui::AppContext) {
//         init_test(cx);

//         fn assert(marked_text: &str, cx: &mut gpui::AppContext) {
//             let (snapshot, display_points) = marked_display_snapshot(marked_text, cx);
//             assert_eq!(
//                 previous_subword_start(&snapshot, display_points[1]),
//                 display_points[0]
//             );
//         }

//         // Subword boundaries are respected
//         assert("lorem_ˇipˇsum", cx);
//         assert("lorem_ˇipsumˇ", cx);
//         assert("ˇlorem_ˇipsum", cx);
//         assert("lorem_ˇipsum_ˇdolor", cx);
//         assert("loremˇIpˇsum", cx);
//         assert("loremˇIpsumˇ", cx);

//         // Word boundaries are still respected
//         assert("\nˇ   ˇlorem", cx);
//         assert("    ˇloremˇ", cx);
//         assert("    ˇlorˇem", cx);
//         assert("\nlorem\nˇ   ˇipsum", cx);
//         assert("\n\nˇ\nˇ", cx);
//         assert("    ˇlorem  ˇipsum", cx);
//         assert("loremˇ-ˇipsum", cx);
//         assert("loremˇ-#$@ˇipsum", cx);
//         assert(" ˇdefγˇ", cx);
//         assert(" bcˇΔˇ", cx);
//         assert(" ˇbcδˇ", cx);
//         assert(" abˇ——ˇcd", cx);
//     }

//     #[gpui::test]
//     fn test_find_preceding_boundary(cx: &mut gpui::AppContext) {
//         init_test(cx);

//         fn assert(
//             marked_text: &str,
//             cx: &mut gpui::AppContext,
//             is_boundary: impl FnMut(char, char) -> bool,
//         ) {
//             let (snapshot, display_points) = marked_display_snapshot(marked_text, cx);
//             assert_eq!(
//                 find_preceding_boundary(
//                     &snapshot,
//                     display_points[1],
//                     FindRange::MultiLine,
//                     is_boundary
//                 ),
//                 display_points[0]
//             );
//         }

//         assert("abcˇdef\ngh\nijˇk", cx, |left, right| {
//             left == 'c' && right == 'd'
//         });
//         assert("abcdef\nˇgh\nijˇk", cx, |left, right| {
//             left == '\n' && right == 'g'
//         });
//         let mut line_count = 0;
//         assert("abcdef\nˇgh\nijˇk", cx, |left, _| {
//             if left == '\n' {
//                 line_count += 1;
//                 line_count == 2
//             } else {
//                 false
//             }
//         });
//     }

//     #[gpui::test]
//     fn test_find_preceding_boundary_with_inlays(cx: &mut gpui::AppContext) {
//         init_test(cx);

//         let input_text = "abcdefghijklmnopqrstuvwxys";
//         let family_id = cx
//             .font_cache()
//             .load_family(&["Helvetica"], &Default::default())
//             .unwrap();
//         let font_id = cx
//             .font_cache()
//             .select_font(family_id, &Default::default())
//             .unwrap();
//         let font_size = 14.0;
//         let buffer = MultiBuffer::build_simple(input_text, cx);
//         let buffer_snapshot = buffer.read(cx).snapshot(cx);
//         let display_map =
//             cx.add_model(|cx| DisplayMap::new(buffer, font_id, font_size, None, 1, 1, cx));

//         // add all kinds of inlays between two word boundaries: we should be able to cross them all, when looking for another boundary
//         let mut id = 0;
//         let inlays = (0..buffer_snapshot.len())
//             .map(|offset| {
//                 [
//                     Inlay {
//                         id: InlayId::Suggestion(post_inc(&mut id)),
//                         position: buffer_snapshot.anchor_at(offset, Bias::Left),
//                         text: format!("test").into(),
//                     },
//                     Inlay {
//                         id: InlayId::Suggestion(post_inc(&mut id)),
//                         position: buffer_snapshot.anchor_at(offset, Bias::Right),
//                         text: format!("test").into(),
//                     },
//                     Inlay {
//                         id: InlayId::Hint(post_inc(&mut id)),
//                         position: buffer_snapshot.anchor_at(offset, Bias::Left),
//                         text: format!("test").into(),
//                     },
//                     Inlay {
//                         id: InlayId::Hint(post_inc(&mut id)),
//                         position: buffer_snapshot.anchor_at(offset, Bias::Right),
//                         text: format!("test").into(),
//                     },
//                 ]
//             })
//             .flatten()
//             .collect();
//         let snapshot = display_map.update(cx, |map, cx| {
//             map.splice_inlays(Vec::new(), inlays, cx);
//             map.snapshot(cx)
//         });

//         assert_eq!(
//             find_preceding_boundary(
//                 &snapshot,
//                 buffer_snapshot.len().to_display_point(&snapshot),
//                 FindRange::MultiLine,
//                 |left, _| left == 'e',
//             ),
//             snapshot
//                 .buffer_snapshot
//                 .offset_to_point(5)
//                 .to_display_point(&snapshot),
//             "Should not stop at inlays when looking for boundaries"
//         );
//     }

//     #[gpui::test]
//     fn test_next_word_end(cx: &mut gpui::AppContext) {
//         init_test(cx);

//         fn assert(marked_text: &str, cx: &mut gpui::AppContext) {
//             let (snapshot, display_points) = marked_display_snapshot(marked_text, cx);
//             assert_eq!(
//                 next_word_end(&snapshot, display_points[0]),
//                 display_points[1]
//             );
//         }

//         assert("\nˇ   loremˇ", cx);
//         assert("    ˇloremˇ", cx);
//         assert("    lorˇemˇ", cx);
//         assert("    loremˇ    ˇ\nipsum\n", cx);
//         assert("\nˇ\nˇ\n\n", cx);
//         assert("loremˇ    ipsumˇ   ", cx);
//         assert("loremˇ-ˇipsum", cx);
//         assert("loremˇ#$@-ˇipsum", cx);
//         assert("loremˇ_ipsumˇ", cx);
//         assert(" ˇbcΔˇ", cx);
//         assert(" abˇ——ˇcd", cx);
//     }

//     #[gpui::test]
//     fn test_next_subword_end(cx: &mut gpui::AppContext) {
//         init_test(cx);

//         fn assert(marked_text: &str, cx: &mut gpui::AppContext) {
//             let (snapshot, display_points) = marked_display_snapshot(marked_text, cx);
//             assert_eq!(
//                 next_subword_end(&snapshot, display_points[0]),
//                 display_points[1]
//             );
//         }

//         // Subword boundaries are respected
//         assert("loˇremˇ_ipsum", cx);
//         assert("ˇloremˇ_ipsum", cx);
//         assert("loremˇ_ipsumˇ", cx);
//         assert("loremˇ_ipsumˇ_dolor", cx);
//         assert("loˇremˇIpsum", cx);
//         assert("loremˇIpsumˇDolor", cx);

//         // Word boundaries are still respected
//         assert("\nˇ   loremˇ", cx);
//         assert("    ˇloremˇ", cx);
//         assert("    lorˇemˇ", cx);
//         assert("    loremˇ    ˇ\nipsum\n", cx);
//         assert("\nˇ\nˇ\n\n", cx);
//         assert("loremˇ    ipsumˇ   ", cx);
//         assert("loremˇ-ˇipsum", cx);
//         assert("loremˇ#$@-ˇipsum", cx);
//         assert("loremˇ_ipsumˇ", cx);
//         assert(" ˇbcˇΔ", cx);
//         assert(" abˇ——ˇcd", cx);
//     }

//     #[gpui::test]
//     fn test_find_boundary(cx: &mut gpui::AppContext) {
//         init_test(cx);

//         fn assert(
//             marked_text: &str,
//             cx: &mut gpui::AppContext,
//             is_boundary: impl FnMut(char, char) -> bool,
//         ) {
//             let (snapshot, display_points) = marked_display_snapshot(marked_text, cx);
//             assert_eq!(
//                 find_boundary(
//                     &snapshot,
//                     display_points[0],
//                     FindRange::MultiLine,
//                     is_boundary
//                 ),
//                 display_points[1]
//             );
//         }

//         assert("abcˇdef\ngh\nijˇk", cx, |left, right| {
//             left == 'j' && right == 'k'
//         });
//         assert("abˇcdef\ngh\nˇijk", cx, |left, right| {
//             left == '\n' && right == 'i'
//         });
//         let mut line_count = 0;
//         assert("abcˇdef\ngh\nˇijk", cx, |left, _| {
//             if left == '\n' {
//                 line_count += 1;
//                 line_count == 2
//             } else {
//                 false
//             }
//         });
//     }

//     #[gpui::test]
//     fn test_surrounding_word(cx: &mut gpui::AppContext) {
//         init_test(cx);

//         fn assert(marked_text: &str, cx: &mut gpui::AppContext) {
//             let (snapshot, display_points) = marked_display_snapshot(marked_text, cx);
//             assert_eq!(
//                 surrounding_word(&snapshot, display_points[1]),
//                 display_points[0]..display_points[2],
//                 "{}",
//                 marked_text.to_string()
//             );
//         }

//         assert("ˇˇloremˇ  ipsum", cx);
//         assert("ˇloˇremˇ  ipsum", cx);
//         assert("ˇloremˇˇ  ipsum", cx);
//         assert("loremˇ ˇ  ˇipsum", cx);
//         assert("lorem\nˇˇˇ\nipsum", cx);
//         assert("lorem\nˇˇipsumˇ", cx);
//         assert("loremˇ,ˇˇ ipsum", cx);
//         assert("ˇloremˇˇ, ipsum", cx);
//     }

//     #[gpui::test]
//     async fn test_move_up_and_down_with_excerpts(cx: &mut gpui::TestAppContext) {
//         cx.update(|cx| {
//             init_test(cx);
//         });

//         let mut cx = EditorTestContext::new(cx).await;
//         let editor = cx.editor.clone();
//         let window = cx.window.clone();
//         cx.update_window(window, |cx| {
//             let text_layout_details =
//                 editor.read_with(cx, |editor, cx| editor.text_layout_details(cx));

//             let family_id = cx
//                 .font_cache()
//                 .load_family(&["Helvetica"], &Default::default())
//                 .unwrap();
//             let font_id = cx
//                 .font_cache()
//                 .select_font(family_id, &Default::default())
//                 .unwrap();

//             let buffer =
//                 cx.add_model(|cx| Buffer::new(0, cx.model_id() as u64, "abc\ndefg\nhijkl\nmn"));
//             let multibuffer = cx.add_model(|cx| {
//                 let mut multibuffer = MultiBuffer::new(0);
//                 multibuffer.push_excerpts(
//                     buffer.clone(),
//                     [
//                         ExcerptRange {
//                             context: Point::new(0, 0)..Point::new(1, 4),
//                             primary: None,
//                         },
//                         ExcerptRange {
//                             context: Point::new(2, 0)..Point::new(3, 2),
//                             primary: None,
//                         },
//                     ],
//                     cx,
//                 );
//                 multibuffer
//             });
//             let display_map =
//                 cx.add_model(|cx| DisplayMap::new(multibuffer, font_id, 14.0, None, 2, 2, cx));
//             let snapshot = display_map.update(cx, |map, cx| map.snapshot(cx));

//             assert_eq!(snapshot.text(), "\n\nabc\ndefg\n\n\nhijkl\nmn");

//             let col_2_x = snapshot.x_for_point(DisplayPoint::new(2, 2), &text_layout_details);

//             // Can't move up into the first excerpt's header
//             assert_eq!(
//                 up(
//                     &snapshot,
//                     DisplayPoint::new(2, 2),
//                     SelectionGoal::HorizontalPosition(col_2_x),
//                     false,
//                     &text_layout_details
//                 ),
//                 (
//                     DisplayPoint::new(2, 0),
//                     SelectionGoal::HorizontalPosition(0.0)
//                 ),
//             );
//             assert_eq!(
//                 up(
//                     &snapshot,
//                     DisplayPoint::new(2, 0),
//                     SelectionGoal::None,
//                     false,
//                     &text_layout_details
//                 ),
//                 (
//                     DisplayPoint::new(2, 0),
//                     SelectionGoal::HorizontalPosition(0.0)
//                 ),
//             );

//             let col_4_x = snapshot.x_for_point(DisplayPoint::new(3, 4), &text_layout_details);

//             // Move up and down within first excerpt
//             assert_eq!(
//                 up(
//                     &snapshot,
//                     DisplayPoint::new(3, 4),
//                     SelectionGoal::HorizontalPosition(col_4_x),
//                     false,
//                     &text_layout_details
//                 ),
//                 (
//                     DisplayPoint::new(2, 3),
//                     SelectionGoal::HorizontalPosition(col_4_x)
//                 ),
//             );
//             assert_eq!(
//                 down(
//                     &snapshot,
//                     DisplayPoint::new(2, 3),
//                     SelectionGoal::HorizontalPosition(col_4_x),
//                     false,
//                     &text_layout_details
//                 ),
//                 (
//                     DisplayPoint::new(3, 4),
//                     SelectionGoal::HorizontalPosition(col_4_x)
//                 ),
//             );

//             let col_5_x = snapshot.x_for_point(DisplayPoint::new(6, 5), &text_layout_details);

//             // Move up and down across second excerpt's header
//             assert_eq!(
//                 up(
//                     &snapshot,
//                     DisplayPoint::new(6, 5),
//                     SelectionGoal::HorizontalPosition(col_5_x),
//                     false,
//                     &text_layout_details
//                 ),
//                 (
//                     DisplayPoint::new(3, 4),
//                     SelectionGoal::HorizontalPosition(col_5_x)
//                 ),
//             );
//             assert_eq!(
//                 down(
//                     &snapshot,
//                     DisplayPoint::new(3, 4),
//                     SelectionGoal::HorizontalPosition(col_5_x),
//                     false,
//                     &text_layout_details
//                 ),
//                 (
//                     DisplayPoint::new(6, 5),
//                     SelectionGoal::HorizontalPosition(col_5_x)
//                 ),
//             );

//             let max_point_x = snapshot.x_for_point(DisplayPoint::new(7, 2), &text_layout_details);

//             // Can't move down off the end
//             assert_eq!(
//                 down(
//                     &snapshot,
//                     DisplayPoint::new(7, 0),
//                     SelectionGoal::HorizontalPosition(0.0),
//                     false,
//                     &text_layout_details
//                 ),
//                 (
//                     DisplayPoint::new(7, 2),
//                     SelectionGoal::HorizontalPosition(max_point_x)
//                 ),
//             );
//             assert_eq!(
//                 down(
//                     &snapshot,
//                     DisplayPoint::new(7, 2),
//                     SelectionGoal::HorizontalPosition(max_point_x),
//                     false,
//                     &text_layout_details
//                 ),
//                 (
//                     DisplayPoint::new(7, 2),
//                     SelectionGoal::HorizontalPosition(max_point_x)
//                 ),
//             );
//         });
//     }

//     fn init_test(cx: &mut gpui::AppContext) {
//         cx.set_global(SettingsStore::test(cx));
//         theme::init(cx);
//         language::init(cx);
//         crate::init(cx);
//         Project::init_settings(cx);
//     }
// }