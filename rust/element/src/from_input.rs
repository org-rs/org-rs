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

/// Lifetime-preserving analogue of [`std::str::FromStr`].
///
/// `FromStr` cannot be implemented for types that borrow from the input because
/// its signature has no binding between the `&str` parameter and `Self`.  This
/// trait carries `'a` on the trait itself — the same pattern serde uses for
/// zero-copy deserialization — so the output lifetime is tied to the input.
pub trait FromInput<'a>: Sized {
    type Err;
    fn from_input(s: &'a str) -> Result<Self, Self::Err>;
}

/// Extension trait that surfaces [`FromInput`] as a method on `str`,
/// mirroring the ergonomics of `str::parse::<T>()`.
pub trait InputExt {
    fn parse_org<'a, T: FromInput<'a>>(&'a self) -> Result<T, T::Err>;
}

impl InputExt for str {
    fn parse_org<'a, T: FromInput<'a>>(&'a self) -> Result<T, T::Err> {
        T::from_input(self)
    }
}
