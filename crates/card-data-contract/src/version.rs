use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataVersion {
    major: u32,
    minor: u32,
    patch: u32,
    build_id: u64,
    revision: u64,
}

impl DataVersion {
    pub fn parse(value: &str) -> Result<Self, String> {
        let (patch_version, build_and_revision) = value
            .split_once("-build")
            .ok_or_else(|| format!("invalid data version: {value}"))?;
        let (build_id, revision) = build_and_revision
            .split_once("-r")
            .ok_or_else(|| format!("invalid data version: {value}"))?;

        let mut components = patch_version.split('.');
        let major = parse_component(components.next(), value)?;
        let minor = parse_component(components.next(), value)?;
        let patch = parse_component(components.next(), value)?;
        if components.next().is_some() {
            return Err(format!("invalid data version: {value}"));
        }

        let build_id = parse_positive_component(build_id, value)?;
        let revision = parse_positive_component(revision, value)?;

        Ok(Self {
            major,
            minor,
            patch,
            build_id,
            revision,
        })
    }

    pub fn official_patch_version(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }

    pub fn build_id(&self) -> u64 {
        self.build_id
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }
}

impl fmt::Display for DataVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}-build{}-r{}",
            self.official_patch_version(),
            self.build_id,
            self.revision
        )
    }
}

fn parse_component(component: Option<&str>, value: &str) -> Result<u32, String> {
    let component = component.ok_or_else(|| format!("invalid data version: {value}"))?;
    if component.is_empty() || !component.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("invalid data version: {value}"));
    }
    component
        .parse()
        .map_err(|_| format!("invalid data version: {value}"))
}

fn parse_positive_component(component: &str, value: &str) -> Result<u64, String> {
    if component.is_empty()
        || component.starts_with('0')
        || !component.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!("invalid data version: {value}"));
    }
    component
        .parse()
        .map_err(|_| format!("invalid data version: {value}"))
}

#[cfg(test)]
mod tests {
    use super::DataVersion;

    #[test]
    fn parses_the_approved_data_version() {
        let version = DataVersion::parse("36.0.3-build247416-r1").unwrap();
        assert_eq!(version.official_patch_version(), "36.0.3");
        assert_eq!(version.build_id(), 247_416);
        assert_eq!(version.revision(), 1);
        assert_eq!(version.to_string(), "36.0.3-build247416-r1");
    }

    #[test]
    fn rejects_zero_and_unstructured_versions() {
        for value in ["36.0.3-build0-r1", "36.0.3-build1-r0", "latest"] {
            assert!(DataVersion::parse(value).is_err(), "accepted {value}");
        }
    }
}
