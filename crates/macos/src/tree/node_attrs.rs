#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NodeAttrs {
    pub(crate) role: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) value: Option<String>,
    pub(crate) enabled: bool,
}

impl NodeAttrs {
    pub(crate) fn enabled_by_default(
        role: Option<String>,
        title: Option<String>,
        description: Option<String>,
        value: Option<String>,
        enabled: Option<String>,
    ) -> Self {
        Self {
            role,
            title,
            description,
            value,
            enabled: enabled.map(|s| s == "true").unwrap_or(true),
        }
    }
}
