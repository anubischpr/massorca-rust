//! Reading the SDDL text Windows writes for a file's owner and DACL (ADR 0073), and the `\\?\` form of a path.

use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::Path;

use crate::windows_dir_access::Sid;
use crate::windows_trust::Trusted;

/// Rights that let a holder change an object or what it contains: add a file or folder, delete a child, delete the
/// object itself, change its DACL or owner, and the generic write and all rights.
const WRITE_RIGHTS: u32 =
    0x2 | 0x4 | 0x40 | 0x1_0000 | 0x4_0000 | 0x8_0000 | 0x4000_0000 | 0x1000_0000;

/// `FILE_ALL_ACCESS`, which the owner-only entry grants.
const FULL_ACCESS: u32 = 0x001F_01FF;

/// One access control entry, with its trustee parsed into a SID. A field that cannot be read is `None`.
#[derive(Debug)]
pub(crate) struct Ace {
    allow: bool,
    flags: String,
    mask: Option<u32>,
    sid: Option<Sid>,
}

impl Ace {
    fn applies_to_object(&self) -> bool {
        !self.flags.contains("IO")
    }

    fn inherited_by_files_and_folders(&self) -> bool {
        self.flags.contains("OI") && self.flags.contains("CI")
    }
}

/// The owner and DACL of an object, read from the SDDL Windows writes for it. SIDs are parsed, never compared as text.
#[derive(Debug)]
pub(crate) struct Descriptor {
    pub owner: Option<Sid>,
    protected: bool,
    /// No DACL at all, which grants everyone everything.
    unrestricted: bool,
    aces: Vec<Ace>,
}

impl Descriptor {
    pub(crate) fn parse(sddl: &str) -> Self {
        let (owner, dacl) = split_sddl(sddl);
        let (flags, aces) = split_dacl(dacl);
        Self {
            owner: Sid::parse(owner).ok(),
            protected: flags.contains('P'),
            unrestricted: dacl.is_empty() || flags.contains("NO_ACCESS_CONTROL"),
            aces: top_level_groups(aces).map(parse_ace).collect(),
        }
    }

    /// Whether the DACL lets anyone outside `trusted` change the object. Inherit-only entries do not apply to the
    /// object and are skipped; an entry whose rights or trustee cannot be read counts as a write.
    pub(crate) fn grants_write_to_others(&self, trusted: &Trusted) -> bool {
        self.unrestricted
            || self.aces.iter().any(|ace| {
                ace.allow
                    && ace.applies_to_object()
                    && !ace.sid.as_ref().is_some_and(|sid| trusted.may_write(sid))
                    && ace.mask.is_none_or(|mask| mask & WRITE_RIGHTS != 0)
            })
    }

    /// Whether the DACL is protected and holds exactly one entry: full access for `user`, inherited by everything below.
    pub(crate) fn is_owner_only(&self, user: &Sid) -> bool {
        match self.aces.as_slice() {
            [ace] => {
                self.protected
                    && !self.unrestricted
                    && ace.allow
                    && ace.applies_to_object()
                    && ace.inherited_by_files_and_folders()
                    && ace.mask == Some(FULL_ACCESS)
                    && ace.sid.as_ref() == Some(user)
            }
            _ => false,
        }
    }
}

/// An ACE body: `type;flags;rights;object;inherited object;trustee[;condition]`.
fn parse_ace(body: &str) -> Ace {
    let fields: Vec<&str> = body.split(';').collect();
    let field = |index: usize| fields.get(index).copied().unwrap_or("");
    Ace {
        allow: matches!(field(0), "A" | "XA" | "OA" | "ZA"),
        flags: field(1).to_owned(),
        mask: parse_rights(field(2)),
        sid: Sid::parse(field(5)).ok(),
    }
}

/// The owner and the DACL sections of an `O:…D:…` SDDL string.
fn split_sddl(sddl: &str) -> (&str, &str) {
    let owner_start = sddl.find("O:").map_or(0, |index| index + 2);
    let dacl_start = sddl.find("D:").unwrap_or(sddl.len());
    let owner = sddl
        .get(owner_start..dacl_start.max(owner_start))
        .unwrap_or("");
    (owner, &sddl[dacl_start..])
}

/// The control flags and the ACEs of a `D:` SDDL section.
fn split_dacl(dacl: &str) -> (&str, &str) {
    let body = dacl.strip_prefix("D:").unwrap_or(dacl);
    body.split_at(body.find('(').unwrap_or(body.len()))
}

/// The ACE bodies of an SDDL ACE list, without their outer parentheses; conditional ACEs may nest parentheses.
fn top_level_groups(aces: &str) -> impl Iterator<Item = &str> {
    let mut groups = Vec::new();
    let mut depth = 0_usize;
    let mut start = 0;
    for (index, character) in aces.char_indices() {
        match character {
            '(' => {
                if depth == 0 {
                    start = index + 1;
                }
                depth += 1;
            }
            ')' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    groups.push(&aces[start..index]);
                }
            }
            _ => {}
        }
    }
    groups.into_iter()
}

/// An SDDL rights field as a mask: hexadecimal, or two-letter right codes. `None` when it cannot be read.
fn parse_rights(text: &str) -> Option<u32> {
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        return u32::from_str_radix(hex, 16).ok();
    }
    if text.is_empty() || !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len()).step_by(2).try_fold(0_u32, |mask, index| {
        right_code(text.get(index..index + 2)?).map(|bit| mask | bit)
    })
}

fn right_code(code: &str) -> Option<u32> {
    Some(match code {
        "GA" => 0x1000_0000,
        "GR" => 0x8000_0000,
        "GW" => 0x4000_0000,
        "GX" => 0x2000_0000,
        "FA" => 0x001F_01FF,
        "FR" => 0x0012_0089,
        "FW" => 0x0012_0116,
        "FX" => 0x0012_00A0,
        "SD" => 0x1_0000,
        "RC" => 0x2_0000,
        "WD" => 0x4_0000,
        "WO" => 0x8_0000,
        "CC" => 0x1,
        "DC" => 0x2,
        "LC" => 0x4,
        "SW" => 0x8,
        "RP" => 0x10,
        "WP" => 0x20,
        "DT" => 0x40,
        "LO" => 0x80,
        "CR" => 0x100,
        _ => return None,
    })
}

/// The `\\?\` form of an absolute path, so Win32 calls accept paths longer than 248 characters.
/// Relative and already-prefixed paths are returned as they are.
pub(crate) fn verbatim(path: &Path) -> OsString {
    let units: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .map(|unit| {
            if unit == u16::from(b'/') {
                u16::from(b'\\')
            } else {
                unit
            }
        })
        .collect();
    let text = |value: &str| value.encode_utf16().collect::<Vec<u16>>();
    let prefixed = if units.starts_with(&text(r"\\?\")) || units.starts_with(&text(r"\\.\")) {
        units
    } else if let Some(share) = units.strip_prefix(text(r"\\").as_slice()) {
        [text(r"\\?\UNC\").as_slice(), share].concat()
    } else if is_drive_absolute(&units) {
        [text(r"\\?\").as_slice(), &units].concat()
    } else {
        units
    };
    OsString::from_wide(&prefixed)
}

fn is_drive_absolute(units: &[u16]) -> bool {
    matches!(units, [letter, colon, slash, ..]
        if u8::try_from(*letter).is_ok_and(|letter| letter.is_ascii_alphabetic())
            && *colon == u16::from(b':')
            && *slash == u16::from(b'\\'))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::Path;

    use super::{Descriptor, parse_rights, verbatim};
    use crate::windows_dir_access::Sid;
    use crate::windows_trust::Trusted;

    /// The built-in Administrator, which SDDL writes as `LA`.
    fn administrator() -> Sid {
        Sid::parse("LA").expect("the built-in Administrator")
    }

    fn trusted() -> Trusted {
        Trusted::for_token(administrator(), administrator()).expect("trusted SIDs")
    }

    fn sid_string() -> String {
        administrator().to_string()
    }

    #[test]
    fn write_rights_for_an_untrusted_sid_are_found_in_every_spelling() {
        for ace in [
            "(A;OICIID;FA;;;AU)",
            "(A;OICIID;0x1301bf;;;S-1-5-21-9-9-9-1005)",
            "(A;;FW;;;WD)",
            "(A;;GA;;;BU)",
            "(A;;DCLC;;;WD)",
            "(A;;SD;;;WD)",
            "(A;;WDWO;;;WD)",
            "(A;;ZZ;;;WD)",
            "(A;;FA;;;not-a-sid)",
        ] {
            let sddl = format!("O:LAD:AI(A;OICI;FA;;;LA){ace}");
            assert!(
                Descriptor::parse(&sddl).grants_write_to_others(&trusted()),
                "{sddl}"
            );
        }
        assert!(Descriptor::parse("O:LAD:NO_ACCESS_CONTROL").grants_write_to_others(&trusted()));
    }

    #[test]
    fn read_rights_trusted_sids_in_any_spelling_denials_and_inherit_only_entries_do_not_count() {
        let user = sid_string();
        for ace in [
            "(A;OICIID;0x1200a9;;;S-1-5-21-9-9-9-1005)".to_owned(),
            "(A;;FR;;;WD)".to_owned(),
            "(A;OICI;FA;;;SY)".to_owned(),
            "(A;OICI;FA;;;S-1-5-18)".to_owned(),
            "(A;OICI;FA;;;BA)".to_owned(),
            "(A;OICI;FA;;;S-1-5-32-544)".to_owned(),
            format!("(A;OICI;FA;;;{user})"),
            "(D;;FA;;;WD)".to_owned(),
            "(A;OICIIO;FA;;;WD)".to_owned(),
        ] {
            let sddl = format!("O:LAD:PAI(A;OICI;FA;;;LA){ace}");
            assert!(
                !Descriptor::parse(&sddl).grants_write_to_others(&trusted()),
                "{sddl}"
            );
        }
    }

    #[test]
    fn an_owner_only_dacl_is_recognised_whichever_way_the_user_is_spelled() {
        let user = administrator();
        for sddl in [
            "O:LAD:P(A;OICI;FA;;;LA)".to_owned(),
            "O:LAD:PAI(A;OICI;FA;;;LA)".to_owned(),
            format!("O:{0}D:P(A;OICI;0x1f01ff;;;{0})", sid_string()),
        ] {
            assert!(Descriptor::parse(&sddl).is_owner_only(&user), "{sddl}");
        }
        for sddl in [
            "O:LAD:AI(A;OICI;FA;;;LA)",
            "O:LAD:P(A;OICI;FA;;;LA)(A;OICI;FA;;;SY)",
            "O:LAD:P(A;OICI;FA;;;SY)",
            "O:LAD:P(A;;FA;;;LA)",
            "O:LAD:P(A;OICI;FR;;;LA)",
            "O:LAD:NO_ACCESS_CONTROL",
        ] {
            assert!(!Descriptor::parse(sddl).is_owner_only(&user), "{sddl}");
        }
    }

    #[test]
    fn the_owner_is_parsed_into_a_sid() {
        let descriptor = Descriptor::parse(&format!("O:{}D:P(A;OICI;FA;;;LA)", sid_string()));
        assert_eq!(descriptor.owner, Some(administrator()));
        assert_eq!(
            Descriptor::parse("O:SYD:").owner,
            Some(Sid::parse("S-1-5-18").expect("SYSTEM"))
        );
        assert_eq!(Descriptor::parse("O:not-a-sidD:").owner, None);
    }

    #[test]
    fn rights_codes_and_hexadecimal_masks_parse() {
        assert_eq!(parse_rights("FA"), Some(0x001F_01FF));
        assert_eq!(parse_rights("0x1200a9"), Some(0x0012_00A9));
        assert_eq!(parse_rights("CCLC"), Some(0x5));
        assert_eq!(parse_rights("C"), None);
        assert_eq!(parse_rights("QQ"), None);
    }

    #[test]
    fn absolute_paths_take_the_verbatim_prefix() {
        assert_eq!(
            verbatim(Path::new(r"C:\Users\me/MassOrca")),
            OsString::from(r"\\?\C:\Users\me\MassOrca")
        );
        assert_eq!(
            verbatim(Path::new(r"\\server\share\home")),
            OsString::from(r"\\?\UNC\server\share\home")
        );
        assert_eq!(
            verbatim(Path::new(r"\\?\C:\already")),
            OsString::from(r"\\?\C:\already")
        );
        assert_eq!(
            verbatim(Path::new(r"relative\dir")),
            OsString::from(r"relative\dir")
        );
    }
}
