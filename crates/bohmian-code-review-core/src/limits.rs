//! Byte limits shared by validated core values.

/// Maximum repository-relative path bytes accepted by [`crate::RelativePath`].
pub const MAX_RELATIVE_PATH_BYTES: usize = 4096;

#[cfg(test)]
/// Contains focused regression tests for this module's locked values.
mod tests {
    use super::*;

    #[test]
    /// Pins the path byte limit copied from the donor extraction contract.
    fn path_limit_matches_the_donor_contract() {
        assert_eq!(MAX_RELATIVE_PATH_BYTES, 4096);
    }
}
