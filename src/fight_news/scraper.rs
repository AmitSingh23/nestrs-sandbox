use std::collections::HashMap;
use std::time::Duration;

use chrono::{NaiveDate, Utc};
use nestrs::sea_orm::DbConn;
use reqwest::Url;
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::entities::fight::{self, FightStatus};
use crate::entities::fighter;
use crate::fight_news::pages::{ArticleDetails, HeadlineLink, article_details, matchup_links};
use crate::fight_news::parser::{Matchup, NewsItem, extract_matchups, normalize};
use crate::fight_news::sherdog;
use crate::fight_news::sources::{SOURCES, Source, SourceKind};

const USER_AGENT: &str = "Mozilla/5.0 (compatible; nestrs-sandbox-fight-news/0.1; +https://github.com/AmitSingh23/nestrs-sandbox)";

/// Pause between article requests to the same site.
const ARTICLE_DELAY: Duration = Duration::from_millis(300);

/// News published this soon after a pair's fight is still about that fight.
/// Fight nights in the Americas end after midnight UTC, and recaps follow.
const POST_FIGHT_COVERAGE: chrono::Duration = chrono::Duration::days(2);

/// Upper bound on articles opened per source and run.
const MAX_ARTICLES_PER_SOURCE: usize = 40;

/// Hosts whose pages can't be fetched without signing in.
const SKIP_ARTICLE_HOSTS: &[&str] = &["twitter.com", "x.com"];

#[derive(Debug, Default)]
pub struct ScrapeSummary {
    pub sources_ok: usize,
    pub sources_failed: usize,
    pub fights_seen: usize,
    pub fights_created: usize,
    pub fights_updated: usize,
    pub fights_unchanged: usize,
    /// News about a fight that has already happened.
    pub fights_skipped: usize,
}

/// One fight reported by a source, with where it was reported.
struct Report {
    matchup: Matchup,
    url: String,
    headline: String,
    /// When the article was published, if known.
    published: Option<NaiveDate>,
}

enum Saved {
    Created,
    Updated,
    Unchanged,
    Skipped,
}

/// Scrapes every source, extracts upcoming matchups and saves them. One
/// source failing doesn't stop the others.
pub async fn scrape_all(db: &DbConn) -> Result<ScrapeSummary, sea_orm::DbErr> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(20))
        .build()
        .expect("failed to build HTTP client");
    let today = Utc::now().date_naive();
    let mut summary = ScrapeSummary::default();

    let mut scraped = Vec::new();
    for source in SOURCES {
        let reports = match &source.kind {
            SourceKind::NewsPages(pages) => news_reports(&client, source, pages, today).await,
            SourceKind::SherdogEvents => sherdog_reports(&client, today).await,
        };
        match reports {
            Ok(reports) => {
                summary.sources_ok += 1;
                scraped.push((source, reports));
            }
            Err(err) => {
                tracing::warn!(source = source.name, %err, "failed to scrape source");
                summary.sources_failed += 1;
            }
        }
    }

    let calendar = EventCalendar::from_reports(scraped.iter().flat_map(|(_, r)| r));
    for (source, reports) in &mut scraped {
        reports.retain_mut(|report| calendar.place(&mut report.matchup));
        summary.fights_seen += reports.len();

        for report in reports.iter() {
            match save(db, source, report, today).await? {
                Saved::Created => summary.fights_created += 1,
                Saved::Updated => summary.fights_updated += 1,
                Saved::Unchanged => summary.fights_unchanged += 1,
                Saved::Skipped => summary.fights_skipped += 1,
            }
        }
    }
    Ok(summary)
}

/// Upcoming UFC events and their dates, as listed on published fight cards.
/// Used to date fights reported in the news and to spot news about events
/// that already happened.
struct EventCalendar {
    dates: HashMap<String, NaiveDate>,
    /// Lowest upcoming number per numbered series ("UFC" → 333,
    /// "UFC Fight Night" → 291).
    next_number: HashMap<String, u32>,
}

impl EventCalendar {
    fn from_reports<'a>(reports: impl Iterator<Item = &'a Report>) -> Self {
        let mut dates = HashMap::new();
        let mut next_number: HashMap<String, u32> = HashMap::new();
        for matchup in reports.map(|r| &r.matchup).filter(|m| m.from_fight_card) {
            let (Some(name), Some(date)) = (&matchup.event_name, matchup.event_date) else {
                continue;
            };
            dates.insert(name.clone(), date);
            if let Some((series, number)) = numbered_event(name) {
                let lowest = next_number.entry(series.to_string()).or_insert(number);
                *lowest = (*lowest).min(number);
            }
        }
        Self { dates, next_number }
    }

    /// Fills in the date of a known event. Returns `false` for a numbered
    /// event that's older than every upcoming one in its series.
    fn place(&self, matchup: &mut Matchup) -> bool {
        let Some(name) = &matchup.event_name else {
            return true;
        };
        if let Some(date) = self.dates.get(name) {
            matchup.event_date.get_or_insert(*date);
            return true;
        }
        match numbered_event(name) {
            Some((series, number)) => self
                .next_number
                .get(series)
                .is_none_or(|next| number >= *next),
            None => true,
        }
    }
}

/// "UFC 333" → ("UFC", 333); "UFC Fight Night 291" → ("UFC Fight Night", 291).
fn numbered_event(name: &str) -> Option<(&str, u32)> {
    let (series, number) = name.rsplit_once(' ')?;
    Some((series, number.parse().ok()?))
}

/// Matchups in the headlines on a source's news pages. Each matching article
/// is opened for its teaser and publish date, which help place the fight.
async fn news_reports(
    client: &reqwest::Client,
    source: &Source,
    pages: &[&str],
    today: NaiveDate,
) -> Result<Vec<Report>, String> {
    let mut links = Vec::new();
    let mut last_err = None;
    for page in pages {
        let page_url = Url::parse(page).map_err(|e| e.to_string())?;
        match fetch_html(client, page).await {
            Ok(html) => {
                for link in matchup_links(&html, &page_url) {
                    if !links.iter().any(|l: &HeadlineLink| l.url == link.url) {
                        links.push(link);
                    }
                }
            }
            Err(err) => last_err = Some(err),
        }
    }
    if links.is_empty() {
        if let Some(err) = last_err {
            return Err(err);
        }
    }

    let mut reports = Vec::new();
    for link in links.into_iter().take(MAX_ARTICLES_PER_SOURCE) {
        let details = article(client, &link.url).await;
        let item = NewsItem {
            title: link.headline.clone(),
            summary: details.summary,
            published: details.published,
            official: source.official,
        };
        for matchup in extract_matchups(&item, today) {
            reports.push(Report {
                matchup,
                url: link.url.clone(),
                headline: link.headline.clone(),
                published: details.published.map(|p| p.date_naive()),
            });
        }
    }
    Ok(reports)
}

/// Every bout on Sherdog's upcoming UFC fight cards.
async fn sherdog_reports(
    client: &reqwest::Client,
    today: NaiveDate,
) -> Result<Vec<Report>, String> {
    let page_url = Url::parse(sherdog::UFC_EVENTS_URL).expect("valid URL");
    let html = fetch_html(client, sherdog::UFC_EVENTS_URL).await?;

    let mut reports = Vec::new();
    for event in sherdog::upcoming_events(&html, &page_url, today) {
        tokio::time::sleep(ARTICLE_DELAY).await;
        let html = match fetch_html(client, &event.url).await {
            Ok(html) => html,
            Err(err) => {
                tracing::warn!(event = event.name, %err, "failed to fetch Sherdog event");
                continue;
            }
        };
        for (fighter_a, fighter_b) in sherdog::bouts(&html) {
            reports.push(Report {
                headline: format!("{}: {fighter_a} vs. {fighter_b}", event.name),
                url: event.url.clone(),
                published: None,
                matchup: Matchup {
                    fighter_a,
                    fighter_b,
                    status: FightStatus::Confirmed,
                    event_name: Some(event.name.clone()),
                    event_date: Some(event.date),
                    from_fight_card: true,
                },
            });
        }
    }
    Ok(reports)
}

/// An article's metadata, or nothing if it can't be fetched; the headline
/// alone is still usable.
async fn article(client: &reqwest::Client, url: &str) -> ArticleDetails {
    let host = Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string));
    if host.is_some_and(|h| {
        SKIP_ARTICLE_HOSTS
            .iter()
            .any(|s| h == *s || h.ends_with(&format!(".{s}")))
    }) {
        return ArticleDetails::default();
    }
    tokio::time::sleep(ARTICLE_DELAY).await;
    match fetch_html(client, url).await {
        Ok(html) => article_details(&html),
        Err(err) => {
            tracing::debug!(url, %err, "failed to fetch article");
            ArticleDetails::default()
        }
    }
}

async fn fetch_html(client: &reqwest::Client, url: &str) -> Result<String, String> {
    client
        .get(url)
        .header(reqwest::header::ACCEPT, "text/html,application/xhtml+xml")
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())
}

/// Saves a report.
///
/// A pair of fighters has at most one upcoming fight, so a later report about
/// the same pair updates that row: rumors get promoted to confirmed (never the
/// reverse) and a missing event name or date gets filled in. A published fight
/// card is authoritative and overwrites the event and date.
///
/// News published before (or just after) the pair's most recent fight, or
/// undated, is about that fight, so it's skipped rather than saved as a new
/// one. Outlets keep old stories listed for days after an event.
async fn save(
    db: &DbConn,
    source: &Source,
    report: &Report,
    today: NaiveDate,
) -> Result<Saved, sea_orm::DbErr> {
    let matchup = &report.matchup;
    let (a, b, existing) = match booked_pair(db, matchup, today).await? {
        Some(fight) => (fight.fighter_a_id, fight.fighter_b_id, Some(fight)),
        None => {
            let a = fighter_id(db, &matchup.fighter_a).await?;
            let b = fighter_id(db, &matchup.fighter_b).await?;
            (a.min(b), a.max(b), upcoming_fight(db, a, b, today).await?)
        }
    };

    if existing.is_none() && !matchup.from_fight_card {
        if let Some(fought_on) = last_fight_date(db, a, b, today).await? {
            if report
                .published
                .is_none_or(|published| published <= fought_on + POST_FIGHT_COVERAGE)
            {
                return Ok(Saved::Skipped);
            }
        }
    }

    let now = Utc::now();
    let headline = normalize(&report.headline);
    let Some(existing) = existing else {
        fight::ActiveModel {
            fighter_a_id: Set(a),
            fighter_b_id: Set(b),
            status: Set(matchup.status),
            event_name: Set(matchup.event_name.clone()),
            event_date: Set(matchup.event_date),
            source_name: Set(source.name.to_string()),
            source_url: Set(report.url.clone()),
            headline: Set(headline),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(db)
        .await?;
        tracing::debug!(source = source.name, url = report.url, published = ?report.published, "new fight");
        return Ok(Saved::Created);
    };

    let promoted =
        existing.status == FightStatus::Rumored && matchup.status == FightStatus::Confirmed;
    let new_event = matchup.event_name.is_some()
        && (existing.event_name.is_none()
            || (matchup.from_fight_card && existing.event_name != matchup.event_name));
    let new_date = matchup.event_date.is_some()
        && (existing.event_date.is_none()
            || (matchup.from_fight_card && existing.event_date != matchup.event_date));
    if !(promoted || new_event || new_date) {
        return Ok(Saved::Unchanged);
    }

    let mut update: fight::ActiveModel = existing.into();
    if promoted {
        update.status = Set(FightStatus::Confirmed);
    }
    if new_event {
        update.event_name = Set(matchup.event_name.clone());
    }
    if new_date {
        update.event_date = Set(matchup.event_date);
    }
    update.source_name = Set(source.name.to_string());
    update.source_url = Set(report.url.clone());
    update.headline = Set(headline);
    update.updated_at = Set(now);
    update.update(db).await?;
    Ok(Saved::Updated)
}

/// When the two fighters most recently fought, if that's before `today`.
async fn last_fight_date(
    db: &DbConn,
    a: i32,
    b: i32,
    today: NaiveDate,
) -> Result<Option<NaiveDate>, sea_orm::DbErr> {
    let (a, b) = (a.min(b), a.max(b));
    let fight = fight::Entity::find()
        .filter(fight::Column::FighterAId.eq(a))
        .filter(fight::Column::FighterBId.eq(b))
        .filter(fight::Column::EventDate.lt(today))
        .order_by_desc(fight::Column::EventDate)
        .one(db)
        .await?;
    Ok(fight.and_then(|f| f.event_date))
}

/// The upcoming (or undated) fight between two fighters, if any.
async fn upcoming_fight(
    db: &DbConn,
    a: i32,
    b: i32,
    today: NaiveDate,
) -> Result<Option<fight::Model>, sea_orm::DbErr> {
    let (a, b) = (a.min(b), a.max(b));
    fight::Entity::find()
        .filter(fight::Column::FighterAId.eq(a))
        .filter(fight::Column::FighterBId.eq(b))
        .filter(
            Condition::any()
                .add(fight::Column::EventDate.is_null())
                .add(fight::Column::EventDate.gte(today)),
        )
        .one(db)
        .await
}

/// An upcoming fight already saved between fighters these names could refer
/// to. Lets "Allen vs. Duncan" find Brendan Allen vs. Christian Leroy Duncan
/// even when another Duncan exists.
async fn booked_pair(
    db: &DbConn,
    matchup: &Matchup,
    today: NaiveDate,
) -> Result<Option<fight::Model>, sea_orm::DbErr> {
    let a_candidates = candidates(db, &matchup.fighter_a).await?;
    let b_candidates = candidates(db, &matchup.fighter_b).await?;
    for a in &a_candidates {
        for b in &b_candidates {
            if let Some(fight) = upcoming_fight(db, a.id, b.id, today).await? {
                return Ok(Some(fight));
            }
        }
    }
    Ok(None)
}

/// Fighters `name` could refer to: an exact match, or anyone with that
/// surname when `name` is a single word.
async fn candidates(db: &DbConn, name: &str) -> Result<Vec<fighter::Model>, sea_orm::DbErr> {
    let mut condition = Condition::any().add(fighter::Column::Name.eq(name));
    if !name.contains(' ') {
        condition = condition.add(fighter::Column::Name.ends_with(format!(" {name}")));
    }
    fighter::Entity::find().filter(condition).all(db).await
}

/// Finds or creates the fighter. Headlines often use only a surname, so
/// "Buckley" and "Joaquin Buckley" are treated as the same fighter when that's
/// unambiguous, keeping the full name.
async fn fighter_id(db: &DbConn, name: &str) -> Result<i32, sea_orm::DbErr> {
    if let Some(found) = fighter::Entity::find()
        .filter(fighter::Column::Name.eq(name))
        .one(db)
        .await?
    {
        return Ok(found.id);
    }

    match name.rsplit_once(' ') {
        // Surname only: reuse the one full-named fighter with that surname.
        None => {
            let matches = fighter::Entity::find()
                .filter(fighter::Column::Name.ends_with(format!(" {name}")))
                .all(db)
                .await?;
            if let [only] = matches.as_slice() {
                return Ok(only.id);
            }
        }
        // Full name: upgrade a surname-only fighter saved earlier.
        Some((_, surname)) => {
            if let Some(short) = fighter::Entity::find()
                .filter(fighter::Column::Name.eq(surname))
                .one(db)
                .await?
            {
                let id = short.id;
                let mut update: fighter::ActiveModel = short.into();
                update.name = Set(name.to_string());
                update.update(db).await?;
                return Ok(id);
            }
        }
    }

    fighter::Entity::insert(fighter::ActiveModel {
        name: Set(name.to_string()),
        created_at: Set(Utc::now()),
        ..Default::default()
    })
    .on_conflict(
        OnConflict::column(fighter::Column::Name)
            .do_nothing()
            .to_owned(),
    )
    .do_nothing()
    .exec(db)
    .await?;

    let fighter = fighter::Entity::find()
        .filter(fighter::Column::Name.eq(name))
        .one(db)
        .await?
        .expect("fighter row exists after insert");
    Ok(fighter.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(event: &str, date: Option<(i32, u32, u32)>, from_fight_card: bool) -> Report {
        Report {
            matchup: Matchup {
                fighter_a: "A".into(),
                fighter_b: "B".into(),
                status: FightStatus::Confirmed,
                event_name: Some(event.into()),
                event_date: date.and_then(|(y, m, d)| NaiveDate::from_ymd_opt(y, m, d)),
                from_fight_card,
            },
            url: String::new(),
            headline: String::new(),
            published: None,
        }
    }

    #[test]
    fn calendar_dates_known_events_and_drops_past_numbered_ones() {
        let cards = [
            report("UFC 333", Some((2026, 10, 24)), true),
            report("UFC 334", Some((2026, 11, 14)), true),
            report("UFC Fight Night 292", Some((2026, 10, 31)), true),
        ];
        let calendar = EventCalendar::from_reports(cards.iter());

        let mut known = report("UFC 334", None, false).matchup;
        assert!(calendar.place(&mut known));
        assert_eq!(known.event_date, NaiveDate::from_ymd_opt(2026, 11, 14));

        assert!(!calendar.place(&mut report("UFC 332", None, false).matchup));
        assert!(calendar.place(&mut report("UFC 336", None, false).matchup));
        assert!(!calendar.place(&mut report("UFC Fight Night 290", None, false).matchup));
        assert!(calendar.place(&mut report("UFC Edmonton", None, false).matchup));
    }
}
