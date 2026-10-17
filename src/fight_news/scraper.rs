use std::time::Duration;

use chrono::{NaiveDate, Utc};
use nestrs::sea_orm::DbConn;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ActiveModelTrait, ColumnTrait, Condition, EntityTrait, QueryFilter, Set};

use crate::entities::fight::{self, FightStatus};
use crate::entities::fighter;
use crate::fight_news::parser::{extract_matchups, Matchup, NewsItem};
use crate::fight_news::sources::{Source, SOURCES};

const USER_AGENT: &str = "nestrs-sandbox fight-news bot";

#[derive(Debug, Default)]
pub struct ScrapeSummary {
    pub sources_ok: usize,
    pub sources_failed: usize,
    pub items_seen: usize,
    pub fights_created: usize,
    pub fights_updated: usize,
}

/// Fetches every outlet, extracts upcoming matchups and saves them. One
/// outlet failing doesn't stop the others.
pub async fn scrape_all(db: &DbConn) -> Result<ScrapeSummary, sea_orm::DbErr> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(20))
        .build()
        .expect("failed to build HTTP client");
    let today = Utc::now().date_naive();
    let mut summary = ScrapeSummary::default();

    for source in SOURCES {
        let items = match fetch(&client, source).await {
            Ok(items) => items,
            Err(err) => {
                tracing::warn!(source = source.name, %err, "failed to fetch feed");
                summary.sources_failed += 1;
                continue;
            }
        };
        summary.sources_ok += 1;
        summary.items_seen += items.len();

        for item in &items {
            for matchup in extract_matchups(item, today) {
                if save(db, source, item, &matchup, today).await? {
                    summary.fights_created += 1;
                } else {
                    summary.fights_updated += 1;
                }
            }
        }
    }
    Ok(summary)
}

async fn fetch(client: &reqwest::Client, source: &Source) -> Result<Vec<NewsItem>, String> {
    let body = client
        .get(source.feed_url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    let feed = feed_rs::parser::parse(&body[..]).map_err(|e| e.to_string())?;

    Ok(feed
        .entries
        .into_iter()
        .filter_map(|entry| {
            let title = entry.title?.content;
            let url = entry.links.first().map(|l| l.href.clone()).unwrap_or_default();
            Some(NewsItem {
                title,
                summary: entry
                    .summary
                    .map(|s| s.content)
                    .or_else(|| entry.content.and_then(|c| c.body)),
                url,
                published: entry.published.or(entry.updated),
            })
        })
        .collect())
}

/// Saves `matchup`, returning `true` if it's a new fight.
///
/// A pair of fighters has at most one upcoming fight, so a later report about
/// the same pair updates that row: rumors get promoted to confirmed (never the
/// reverse) and a missing event name or date gets filled in.
async fn save(
    db: &DbConn,
    source: &Source,
    item: &NewsItem,
    matchup: &Matchup,
    today: NaiveDate,
) -> Result<bool, sea_orm::DbErr> {
    let a = fighter_id(db, &matchup.fighter_a).await?;
    let b = fighter_id(db, &matchup.fighter_b).await?;
    let (a, b) = (a.min(b), a.max(b));

    let existing = fight::Entity::find()
        .filter(fight::Column::FighterAId.eq(a))
        .filter(fight::Column::FighterBId.eq(b))
        .filter(
            Condition::any()
                .add(fight::Column::EventDate.is_null())
                .add(fight::Column::EventDate.gte(today)),
        )
        .one(db)
        .await?;

    let now = Utc::now();
    let Some(existing) = existing else {
        fight::ActiveModel {
            fighter_a_id: Set(a),
            fighter_b_id: Set(b),
            status: Set(matchup.status),
            event_name: Set(matchup.event_name.clone()),
            event_date: Set(matchup.event_date),
            source_name: Set(source.name.to_string()),
            source_url: Set(item.url.clone()),
            headline: Set(item.title.clone()),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(db)
        .await?;
        return Ok(true);
    };

    let promoted = existing.status == FightStatus::Rumored && matchup.status == FightStatus::Confirmed;
    let new_event = existing.event_name.is_none() && matchup.event_name.is_some();
    let new_date = existing.event_date.is_none() && matchup.event_date.is_some();
    if !(promoted || new_event || new_date) {
        return Ok(false);
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
    update.source_url = Set(item.url.clone());
    update.headline = Set(item.title.clone());
    update.updated_at = Set(now);
    update.update(db).await?;
    Ok(false)
}

async fn fighter_id(db: &DbConn, name: &str) -> Result<i32, sea_orm::DbErr> {
    fighter::Entity::insert(fighter::ActiveModel {
        name: Set(name.to_string()),
        created_at: Set(Utc::now()),
        ..Default::default()
    })
    .on_conflict(OnConflict::column(fighter::Column::Name).do_nothing().to_owned())
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
