//! Site-agnostic HTML extraction for news pages and articles.

use std::sync::LazyLock;

use chrono::{DateTime, NaiveDate, Utc};
use reqwest::Url;
use scraper::{Html, Selector};

use crate::fight_news::parser::{mentions_matchup, normalize};

/// Shorter text is a nav label or byline, longer is a teaser paragraph.
const MIN_HEADLINE_LEN: usize = 20;
const MAX_HEADLINE_LEN: usize = 250;

/// URL path sections for other sports and promotions.
const OTHER_SPORT_SECTIONS: &[&str] = &[
    "boxing",
    "pfl",
    "bellator",
    "one-championship",
    "bkfc",
    "wrestling",
    "wwe",
];

static LINK: LazyLock<Selector> = LazyLock::new(|| Selector::parse("a[href]").unwrap());
static DESCRIPTION: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(r#"meta[property="og:description"], meta[name="description"]"#).unwrap()
});
static PUBLISHED: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(
        r#"meta[property="article:published_time"], meta[itemprop="datePublished"], meta[name="pubdate"]"#,
    )
    .unwrap()
});
static TIME: LazyLock<Selector> = LazyLock::new(|| Selector::parse("time[datetime]").unwrap());

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadlineLink {
    pub headline: String,
    pub url: String,
}

/// Links on a news listing page whose headline names a matchup ("A vs. B").
///
/// The headline is the first headline-length piece of text inside the link.
/// On most sites that's the link's whole text; aggregators like Bleacher
/// Report pack the outlet name, age, headline and teaser into one link, and
/// the headline is the first of those long enough to count.
pub fn matchup_links(html: &str, page_url: &Url) -> Vec<HeadlineLink> {
    let doc = Html::parse_document(html);
    let mut links: Vec<HeadlineLink> = Vec::new();
    for a in doc.select(&LINK) {
        let Some(headline) = a
            .text()
            .map(normalize)
            .find(|t| (MIN_HEADLINE_LEN..=MAX_HEADLINE_LEN).contains(&t.len()))
        else {
            continue;
        };
        if !mentions_matchup(&headline) {
            continue;
        }
        let Some(url) = a.value().attr("href").and_then(|h| page_url.join(h).ok()) else {
            continue;
        };
        if !matches!(url.scheme(), "http" | "https") || is_other_sport(&url) {
            continue;
        }
        let url = url.to_string();
        if links.iter().any(|l| l.url == url || l.headline == headline) {
            continue;
        }
        links.push(HeadlineLink { headline, url });
    }
    links
}

fn is_other_sport(url: &Url) -> bool {
    url.path_segments().into_iter().flatten().any(|segment| {
        let segment = segment.to_lowercase();
        OTHER_SPORT_SECTIONS.iter().any(|s| {
            segment == *s
                || segment.contains(&format!("-{s}"))
                || segment.starts_with(&format!("{s}-"))
        })
    })
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ArticleDetails {
    pub summary: Option<String>,
    pub published: Option<DateTime<Utc>>,
}

/// The teaser and publish time an article page declares in its metadata.
pub fn article_details(html: &str) -> ArticleDetails {
    let doc = Html::parse_document(html);
    let summary = doc
        .select(&DESCRIPTION)
        .filter_map(|m| m.value().attr("content"))
        .map(normalize)
        .find(|s| !s.is_empty());
    let published = doc
        .select(&PUBLISHED)
        .filter_map(|m| m.value().attr("content"))
        .chain(doc.select(&TIME).filter_map(|t| t.value().attr("datetime")))
        .find_map(parse_timestamp);
    ArticleDetails { summary, published }
}

/// RFC 3339 timestamps, or a bare `YYYY-MM-DD` date.
pub fn parse_timestamp(raw: &str) -> Option<DateTime<Utc>> {
    let raw = raw.trim();
    DateTime::parse_from_rfc3339(raw)
        .map(|t| t.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            let date = NaiveDate::parse_from_str(raw.get(..10)?, "%Y-%m-%d").ok()?;
            Some(date.and_hms_opt(0, 0, 0)?.and_utc())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://www.example-mma.com/latest-news").unwrap()
    }

    #[test]
    fn finds_matchup_headlines_and_resolves_links() {
        let html = r#"
            <nav><a href="/ufc">UFC</a></nav>
            <h2><a href="/ufc/515707/ufc-335-oliveira-vs-lopes">UFC 335: Charles Oliveira vs. Diego Lopes adds top 10 flyweight to lineup</a></h2>
            <h2><a href="/ufc/1/other">Alex Pereira says he is ready for anything next</a></h2>
            <a href="/ufc/515707/ufc-335-oliveira-vs-lopes">UFC 335: Charles Oliveira vs. Diego Lopes adds top 10 flyweight to lineup</a>
        "#;
        assert_eq!(
            matchup_links(html, &base()),
            [HeadlineLink {
                headline:
                    "UFC 335: Charles Oliveira vs. Diego Lopes adds top 10 flyweight to lineup"
                        .into(),
                url: "https://www.example-mma.com/ufc/515707/ufc-335-oliveira-vs-lopes".into(),
            }]
        );
    }

    #[test]
    fn aggregator_cards_use_the_headline_not_the_teaser() {
        // Shape of a Bleacher Report card: outlet, age, headline, teaser.
        let html = r#"<a href="https://www.si.com/fannation/mma/news/ufc-edmonton">
            <span>www.si.com</span><span>•</span><span>3h</span>
            <span>UFC Edmonton: Joaquin Buckley vs. Mike Malott Full Fight Card Odds &amp; Best Bets</span>
            <span>The UFC returns to Canada with a welterweight clash.</span></a>"#;
        let links = matchup_links(html, &base());
        assert_eq!(links.len(), 1);
        assert_eq!(
            links[0].headline,
            "UFC Edmonton: Joaquin Buckley vs. Mike Malott Full Fight Card Odds & Best Bets"
        );
    }

    #[test]
    fn skips_other_sports_sections() {
        let html = r#"
            <a href="/boxing/478478/joshua-vs-fury">Dana White says credit Eddie Hearn for keeping Joshua vs. Fury in the U.K.</a>
            <a href="/professional-fighters-league-pfl/479091/card">Carmouche vs. Bishop 2 Start Time, Full Fight Card</a>
            <a href="/raf-wrestling/515854/raf-14">RAF 14: Tsarukyan vs. Danis headlined card marks a first</a>
            <a href="/ufc/515707/oliveira-vs-lopes">UFC 335: Charles Oliveira vs. Diego Lopes adds flyweight bout</a>"#;
        let urls: Vec<_> = matchup_links(html, &base())
            .into_iter()
            .map(|l| l.url)
            .collect();
        assert_eq!(
            urls,
            ["https://www.example-mma.com/ufc/515707/oliveira-vs-lopes"]
        );
    }

    #[test]
    fn reads_article_metadata() {
        let html = r#"<html><head>
            <meta property="og:description" content="The lightweight rematch is set for Nov. 14.">
            <meta property="article:published_time" content="2026-10-16T14:05:00-04:00">
        </head></html>"#;
        let details = article_details(html);
        assert_eq!(
            details.summary.as_deref(),
            Some("The lightweight rematch is set for Nov. 14.")
        );
        assert_eq!(details.published, parse_timestamp("2026-10-16T18:05:00Z"));
    }

    #[test]
    fn falls_back_to_time_element() {
        let html = r#"<article><time datetime="2026-10-15">Oct. 15</time></article>"#;
        assert_eq!(
            article_details(html).published,
            parse_timestamp("2026-10-15T00:00:00Z")
        );
    }
}
