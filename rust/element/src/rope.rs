//    This file is part of org-rs.
//
//    org-rs is free software: you can redistribute it and/or modify
//    it under the terms of the GNU General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    org-rs is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU General Public License for more details.
//
//    You should have received a copy of the GNU General Public License
//    along with org-rs.  If not, see <https://www.gnu.org/licenses/>.

use regex::Regex;
use std::borrow::Cow;
use std::iter::FromIterator;
use std::str;
use xi_rope::find::is_multiline_regex;
use xi_rope::Cursor;
use xi_rope::Interval;
use xi_rope::RopeInfo;

/// Check whether the substring beginning at the cursor location matches
/// the provided regular expression. The substring begins at the beginning
/// of the start of the line.
/// If the regular expression can match multiple lines then the entire text
/// is consumed and matched against the regular expression. Otherwise only
/// the current line is matched. Returns captured text
/// TODO consider moving upstream to xi-rope
fn capture_cursor_regex(
    cursor: &mut Cursor<RopeInfo>,
    re: &Regex,
) -> Option<Vec<Option<Interval>>> {
    let orig_position = cursor.pos();
    let pat = re.as_str();
    let mut lines = cursor.root().lines_raw(cursor.pos()..);
    let text: Cow<str>;

    if is_multiline_regex(pat) {
        // consume all of the text if regex is multi line matching
        text = Cow::from(String::from_iter(lines));
    } else {
        match lines.next() {
            Some(line) => text = line,
            _ => return None,
        }
    }

    let maybe_captures = re.captures(&text);

    // {
    match &maybe_captures {
        Some(cap) => {
            // calculate start position based on where the match starts
            let start_position = orig_position + &cap.get(0).unwrap().start();
            // update cursor and set to end of match
            let end_position = orig_position + &cap.get(0).unwrap().end();
            cursor.set(end_position);
            let res = cap
                .iter()
                .map(|opt| {
                    opt.map(|mat| {
                        Interval::new(start_position + mat.start(), start_position + mat.end())
                    })
                })
                .collect();
            return Some(res);
        }
        None => {
            cursor.set(orig_position + text.len());
            None
        }
    }
}

mod test {
    use crate::affiliated::REGEX_AFFILIATED;
    use xi_rope::Cursor;
    use xi_rope::Interval;
    use xi_rope::Rope;
    use xi_rope::RopeInfo;

    use super::capture_cursor_regex;

    #[test]
    fn capture_cursor_regex_test() {
        let a1 = r"  #+CAPTION[GIT]: org-rs";
        let a2 = r"#+CAPTION: Orgmode";
        let a3 = r"#+RESNAME: someresult";
        let a4 = r"#+ATTR_HTML: :file filename.ext";
        let text = format!("{}\n{}\n{}\n{}\n", a1, a2, a3, a4);

        let rope = Rope::from(text.as_str());

        let mut c = Cursor::new(&rope, 0);

        let captured = capture_cursor_regex(&mut c, &*REGEX_AFFILIATED).unwrap();

        //  assert_eq!(&Interval::new(0, 18), captured.get(0).unwrap());
        //  assert_eq!(18, c.pos()); // cursor is at the end of the match
        //  assert_eq!('o', c.peek_next_codepoint().unwrap());
        //  assert_eq!(Interval::new(4, 11), *captured.get(1).unwrap());
        //  assert_eq!("GIT", rope.slice_to_cow(*captured.get(2).unwrap()));
        //  assert!(captured.get(3).is_none());
        //  assert!(captured.get(4).is_none());
        //  assert!(captured.get(5).is_none());

        //  assert!(capture_cursor_regex(&mut c, &*REGEX_AFFILIATED).is_none());

        // assert_eq!(Interval::new(26, 38), captured.get(0).unwrap());

        // assert_eq!("CAPTION", cap.get(1).unwrap().as_str());
        // assert_eq!("GIT", cap.get(2).unwrap().as_str());
        // assert_eq!(None, cap.get(3));
        // assert_eq!(None, cap.get(4));

        // cap = REGEX_AFFILIATED.captures(dual_part).unwrap();
        // assert_eq!("CAPTION", cap.get(1).unwrap().as_str());
        // assert_eq!(None, cap.get(2));
        // assert_eq!(None, cap.get(3));
        // assert_eq!(None, cap.get(4));

        // cap = REGEX_AFFILIATED.captures(single).unwrap();
        // assert_eq!("RESNAME", cap.get(3).unwrap().as_str());
        // assert_eq!(None, cap.get(1));
        // assert_eq!(None, cap.get(2));
        // assert_eq!(None, cap.get(4));

        // cap = REGEX_AFFILIATED.captures(attr).unwrap();
        // assert_eq!("ATTR_HTML", cap.get(4).unwrap().as_str());
        // assert_eq!(None, cap.get(1));
        // assert_eq!(None, cap.get(2));
        // assert_eq!(None, cap.get(3));
    }

}
