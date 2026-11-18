//! The external pages the About section links to.
//!
//! The webview names a link by id (`open_link` with `link: "website"`), never
//! by URL: the backend maps the id to a fixed address and opens it in the
//! system browser. The webview therefore cannot open an arbitrary URL and
//! needs no opener permission.

use serde::Deserialize;

/// A link the UI may open. Unknown ids fail to deserialize, so the command
/// rejects them before running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Link {
    /// Tegra Software's website.
    Website,
    /// The source repository.
    Repository,
    /// The issue tracker ("Report a problem").
    Issues,
    /// The MIT license as published in the repository.
    License,
}

impl Link {
    /// The fixed address this link opens.
    pub fn url(self) -> &'static str {
        match self {
            Link::Website => "https://tegra.app",
            Link::Repository => "https://github.com/tegrasw/phantompos",
            Link::Issues => "https://github.com/tegrasw/phantompos/issues",
            Link::License => "https://github.com/tegrasw/phantompos/blob/main/LICENSE",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Link;

    fn parse(id: &str) -> Result<Link, serde_json::Error> {
        serde_json::from_value(serde_json::Value::String(id.to_owned()))
    }

    #[test]
    fn ids_map_to_fixed_urls() {
        assert_eq!(parse("website").unwrap().url(), "https://tegra.app");
        assert_eq!(
            parse("repository").unwrap().url(),
            "https://github.com/tegrasw/phantompos"
        );
        assert_eq!(
            parse("issues").unwrap().url(),
            "https://github.com/tegrasw/phantompos/issues"
        );
        assert_eq!(
            parse("license").unwrap().url(),
            "https://github.com/tegrasw/phantompos/blob/main/LICENSE"
        );
    }

    #[test]
    fn unknown_ids_and_urls_are_rejected() {
        for id in [
            "",
            "Website",
            "WEBSITE",
            "docs",
            "https://tegra.app",
            "file:///etc/passwd",
        ] {
            assert!(parse(id).is_err(), "{id:?} should be rejected");
        }
        assert!(serde_json::from_value::<Link>(serde_json::json!(1)).is_err());
    }

    #[test]
    fn every_url_is_https() {
        for link in [Link::Website, Link::Repository, Link::Issues, Link::License] {
            assert!(link.url().starts_with("https://"), "{link:?}");
        }
    }
}
