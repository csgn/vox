//! Versions (`1.2.3`) and version requirements (`>=0.10`, `^1.2`, `>=1, <2`).

use std::fmt;

/// A `MAJOR.MINOR.PATCH` version. Missing parts are 0 (`1.2` = `1.2.0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    /// Major.
    pub major: u64,
    /// Minor.
    pub minor: u64,
    /// Patch.
    pub patch: u64,
}

impl Version {
    /// Parse `1`, `1.2`, or `1.2.3` (a leading `v` is allowed; `-pre`/`+build` are ignored).
    pub fn parse(s: &str) -> Result<Version, String> {
        let core = s
            .trim()
            .trim_start_matches('v')
            .split(['-', '+'])
            .next()
            .unwrap_or("");
        let parts: Vec<&str> = core.split('.').collect();
        if parts.is_empty() || parts.len() > 3 || parts.iter().any(|p| p.is_empty()) {
            return Err(format!("`{s}` is not a version like 1.2.3"));
        }
        let num = |i: usize| -> Result<u64, String> {
            parts.get(i).map_or(Ok(0), |p| {
                p.parse()
                    .map_err(|_| format!("`{s}` is not a version like 1.2.3"))
            })
        };
        Ok(Version {
            major: num(0)?,
            minor: num(1)?,
            patch: num(2)?,
        })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// One comparison in a requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Ge,
    Gt,
    Le,
    Lt,
    Eq,
    /// Compatible: `^1.2` = `>=1.2, <2`; `^0.2` = `>=0.2, <0.3`.
    Caret,
}

/// A version requirement: comparisons joined by commas, all must hold. A bare version
/// means `^` (compatible), like Cargo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionReq {
    parts: Vec<(Op, Version)>,
    text: String,
}

impl VersionReq {
    /// Parse e.g. `>=0.10`, `^1.2`, `1.2`, `>=1, <2`, `=0.3.1`, `*`.
    pub fn parse(s: &str) -> Result<VersionReq, String> {
        let mut parts = Vec::new();
        for item in s.split(',').map(str::trim) {
            if item == "*" {
                continue;
            }
            let (op, rest) = [
                (">=", Op::Ge),
                ("<=", Op::Le),
                (">", Op::Gt),
                ("<", Op::Lt),
                ("=", Op::Eq),
                ("^", Op::Caret),
            ]
            .into_iter()
            .find_map(|(p, op)| item.strip_prefix(p).map(|r| (op, r)))
            .unwrap_or((Op::Caret, item));
            parts.push((op, Version::parse(rest)?));
        }
        Ok(VersionReq {
            parts,
            text: s.trim().to_string(),
        })
    }

    /// Whether `v` satisfies every comparison.
    pub fn matches(&self, v: Version) -> bool {
        self.parts.iter().all(|&(op, r)| match op {
            Op::Ge => v >= r,
            Op::Gt => v > r,
            Op::Le => v <= r,
            Op::Lt => v < r,
            Op::Eq => v == r,
            Op::Caret => {
                v >= r
                    && match (r.major, r.minor) {
                        (0, 0) => v.major == 0 && v.minor == 0 && v.patch == r.patch,
                        (0, m) => v.major == 0 && v.minor == m,
                        (m, _) => v.major == m,
                    }
            }
        })
    }
}

impl fmt::Display for VersionReq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.text.is_empty() {
            "*"
        } else {
            &self.text
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    fn ok(req: &str, ver: &str) -> bool {
        VersionReq::parse(req).unwrap().matches(v(ver))
    }

    #[test]
    fn parse_versions() {
        assert_eq!(
            v("1"),
            Version {
                major: 1,
                minor: 0,
                patch: 0
            }
        );
        assert_eq!(
            v("v0.10.2-beta+3"),
            Version {
                major: 0,
                minor: 10,
                patch: 2
            }
        );
        assert_eq!(v("1.2.3").to_string(), "1.2.3");
        assert!(Version::parse("1..2").is_err());
        assert!(Version::parse("x").is_err());
        assert!(Version::parse("1.2.3.4").is_err());
    }

    #[test]
    fn requirements() {
        assert!(ok(">=0.10", "0.10.0") && ok(">=0.10", "1.0.0") && !ok(">=0.10", "0.9.9"));
        assert!(ok(">=1, <2", "1.9.0") && !ok(">=1, <2", "2.0.0"));
        assert!(ok("1.2", "1.9.0") && !ok("1.2", "2.0.0") && !ok("1.2", "1.1.0"));
        assert!(ok("^0.2", "0.2.7") && !ok("^0.2", "0.3.0"));
        assert!(ok("^0.0.3", "0.0.3") && !ok("^0.0.3", "0.0.4"));
        assert!(ok("=0.3.1", "0.3.1") && !ok("=0.3.1", "0.3.2"));
        assert!(ok("<1", "0.99.0") && ok("<=1", "1.0.0") && ok(">0.1", "0.1.1"));
        assert!(ok("*", "9.9.9"));
        assert!(VersionReq::parse(">=x").is_err());
        assert_eq!(
            VersionReq::parse(" >=1, <2 ").unwrap().to_string(),
            ">=1, <2"
        );
    }
}
