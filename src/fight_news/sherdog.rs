//! Sherdog's UFC event listings and fight cards.
//!
//! Sherdog marks events and bouts up with schema.org microdata: each event row
//! is an `Event` with a `name`, `url` and `startDate`, and each bout on an
//! event page is a `subEvent` whose `name` is "Fighter A vs Fighter B".

use std::sync::LazyLock;

use chrono::NaiveDate;
use reqwest::Url;
use scraper::{ElementRef, Html, Selector};

use crate::fight_news::pages::parse_timestamp;
use crate::fight_news::parser::normalize;

pub const UFC_EVENTS_URL: &str =
    "https://www.sherdog.com/organizations/Ultimate-Fighting-Championship-UFC-2";

static UPCOMING_EVENT: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(r#"#upcoming_tab [itemtype="http://schema.org/Event"]"#).unwrap()
});
static EVENT_LINK: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(r#"a[itemprop="url"]"#).unwrap());
static NAME: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(r#"[itemprop="name"]"#).unwrap());
static START_DATE: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(r#"meta[itemprop="startDate"]"#).unwrap());
static BOUT: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(r#"[itemprop="subEvent"]"#).unwrap());
/// A bout's own name is its first `name` meta; fighter names inside it are
/// `span`s.
static BOUT_NAME: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(r#"meta[itemprop="name"]"#).unwrap());

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventLink {
    /// e.g. "UFC 333", without the "- Volkanovski vs. Evloev" suffix.
    pub name: String,
    pub url: String,
    pub date: NaiveDate,
}

/// Upcoming events on Sherdog's UFC organization page, today included.
pub fn upcoming_events(html: &str, page_url: &Url, today: NaiveDate) -> Vec<EventLink> {
    let doc = Html::parse_document(html);
    doc.select(&UPCOMING_EVENT)
        .filter_map(|row| {
            let link = row.select(&EVENT_LINK).next()?;
            let url = page_url.join(link.value().attr("href")?).ok()?.to_string();
            let full_name = normalize(&row.select(&NAME).next()?.text().collect::<String>());
            let date = start_date(row)?;
            Some(EventLink {
                name: short_event_name(&full_name),
                url,
                date,
            })
        })
        .filter(|event| event.date >= today)
        .collect()
}

/// The bouts on an event page, as (fighter, fighter) pairs.
pub fn bouts(html: &str) -> Vec<(String, String)> {
    let doc = Html::parse_document(html);
    doc.select(&BOUT)
        .filter_map(|bout| {
            let name = bout.select(&BOUT_NAME).next()?.value().attr("content")?;
            let (a, b) = normalize(name)
                .split_once(" vs ")
                .map(|(a, b)| (a.to_string(), b.to_string()))?;
            (!a.is_empty() && !b.is_empty()).then_some((a, b))
        })
        .collect()
}

fn start_date(element: ElementRef) -> Option<NaiveDate> {
    let raw = element
        .select(&START_DATE)
        .next()?
        .value()
        .attr("content")?;
    Some(parse_timestamp(raw)?.date_naive())
}

fn short_event_name(full: &str) -> String {
    full.split_once(" - ")
        .map_or(full, |(name, _)| name)
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Trimmed from Sherdog's markup on 2026-10-17.
    const ORG_PAGE: &str = r#"
        <div class="single_tab" id="upcoming_tab"><table class="new_table event">
          <tr class="table_head"><td>Date</td><td>Fight Title</td></tr>
          <tr itemscope itemtype="http://schema.org/Event">
            <td><meta itemprop="startDate" content="2026-10-17T00:00:00+00:00"></td>
            <td><a itemprop="url" href="/events/UFC-Fight-Night-291-Buckley-vs-Malott-113800">
              <span itemprop="name">UFC Fight Night 291 - Buckley vs. Malott</span></a></td>
          </tr>
          <tr itemscope itemtype="http://schema.org/Event">
            <td><meta itemprop="startDate" content="2026-10-24T00:00:00+00:00"></td>
            <td><a itemprop="url" href="/events/UFC-333-Volkanovski-vs-Evloev-113863">
              <span itemprop="name">UFC 333 - Volkanovski vs. Evloev</span></a></td>
          </tr>
        </table></div>
        <div class="single_tab" id="recent_tab"><table class="new_table event">
          <tr itemscope itemtype="http://schema.org/Event">
            <td><meta itemprop="startDate" content="2026-10-04T00:00:00+00:00"></td>
            <td><a itemprop="url" href="/events/UFC-332-Silva-vs-Cong-1"><span itemprop="name">UFC 332 - Silva vs. Cong</span></a></td>
          </tr>
        </table></div>"#;

    const EVENT_PAGE: &str = r#"
        <div itemprop="subEvent" itemscope itemtype="http://schema.org/Event">
          <meta itemprop="name" content="Alexander Volkanovski vs Movsar Evloev">
          <meta itemprop="startDate" content="2026-10-24T00:00:00+00:00">
          <div class="fight_card">
            <div class="fighter left_side" itemprop="performer" itemscope itemtype="http://schema.org/Person">
              <h3><a href="/fighter/Alexander-Volkanovski-101527"><span itemprop="name">Alexander Volkanovski</span></a></h3>
            </div>
          </div>
        </div>
        <table class="new_table upcoming">
          <tr itemprop="subEvent" itemscope itemtype="http://schema.org/Event">
            <td><meta itemprop="name" content="Petr Yan vs Merab Dvalishvili">
                <meta itemprop="startDate" content="2026-10-24T00:00:00+00:00">13</td>
            <td itemprop="performer" itemscope itemtype="http://schema.org/Person">
              <a itemprop="url" href="/fighter/Petr-Yan-186873"><span itemprop="name">Petr<br />Yan</span></a></td>
          </tr>
          <tr itemprop="subEvent" itemscope itemtype="http://schema.org/Event">
            <td><meta itemprop="name" content="Lone&#039;er Kavanagh vs Ramazonbek Temirov"></td>
          </tr>
        </table>"#;

    #[test]
    fn lists_upcoming_events_only() {
        let page = Url::parse(UFC_EVENTS_URL).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 10, 20).unwrap();
        assert_eq!(
            upcoming_events(ORG_PAGE, &page, today),
            [EventLink {
                name: "UFC 333".into(),
                url: "https://www.sherdog.com/events/UFC-333-Volkanovski-vs-Evloev-113863".into(),
                date: NaiveDate::from_ymd_opt(2026, 10, 24).unwrap(),
            }]
        );
    }

    #[test]
    fn reads_every_bout_on_the_card() {
        assert_eq!(
            bouts(EVENT_PAGE),
            [
                ("Alexander Volkanovski".into(), "Movsar Evloev".into()),
                ("Petr Yan".into(), "Merab Dvalishvili".into()),
                ("Lone'er Kavanagh".into(), "Ramazonbek Temirov".into()),
            ]
        );
    }
}
