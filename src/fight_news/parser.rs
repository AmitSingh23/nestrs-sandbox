//! Pulls upcoming UFC matchups out of news headlines.
//!
//! This is a keyword/regex heuristic, not NLP: it looks for "A vs. B" in the
//! headline, decides confirmed vs rumored from the wording, and picks up an
//! event name and date when the headline or summary carries one.

use std::sync::LazyLock;

use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use regex::Regex;

use crate::entities::fight::FightStatus;

/// Undated rumors older than this are treated as stale.
const MAX_UNDATED_AGE_DAYS: i64 = 60;

/// Most names in headlines are one to three words ("Jones", "Jon Jones",
/// "Gilbert Burns"), plus particles like "dos" or "de".
const MAX_NAME_WORDS: usize = 3;

#[derive(Debug, Clone)]
pub struct NewsItem {
    pub title: String,
    pub summary: Option<String>,
    pub url: String,
    pub published: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matchup {
    pub fighter_a: String,
    pub fighter_b: String,
    pub status: FightStatus,
    pub event_name: Option<String>,
    pub event_date: Option<NaiveDate>,
}

static VS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s+(?:vs\.?|versus|v\.)\s+").unwrap());

static UFC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bUFC\b").unwrap());

/// Headlines about fights that already happened.
static RESULT_WORDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(results?|def\.|defeats?|beats?|knocks? out|knockout of|submits|stops|edges|outpoints|highlights|scorecards?|post-?fight|recap|reacts?|wins|won|retains|loses|lost|bonuses|salaries|payouts)\b",
    )
    .unwrap()
});

static RUMOR_WORDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(rumou?r(?:s|ed)?|targeted|targets?|targeting|in the works|in talks|talks|negotiat\w*|eyes|eyeing|wants?|calls? out|callout|possible|potential|could|might|expected|expects?|plans?|planned|planning|discuss\w*|linked|reportedly|reports?|sources?|nearing|close to|proposed|propos\w*|offer\w*|pitch\w*|open to|interested|hopes?|hoping|teases?|hints?|next\?|\?)",
    )
    .unwrap()
});

static CONFIRM_WORDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(official(?:ly)?|confirm(?:s|ed)?|announc\w*|set|booked|books|added|adds|headlines?|headliner|co-main|main event|finali[sz]ed|signed|locked|scheduled|lands|preview|predictions?|picks|odds|press conference|weigh-?ins?|face-?offs?|fight week)\b",
    )
    .unwrap()
});

static EVENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\bUFC\s+(?:\d{3}\b|Fight Night(?:\s+\d+)?|on\s+(?:ESPN|ABC|FOX|Fox)(?:\s+\d+)?|[A-Z][a-z]+(?:\s+[A-Z][a-z]+)?\s+\d+\b)",
    )
    .unwrap()
});

static DATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b(Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|June?|July?|Aug(?:ust)?|Sept?(?:ember)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?)\.?\s+(\d{1,2})(?:st|nd|rd|th)?\b(?:,?\s+(\d{4}))?",
    )
    .unwrap()
});

static HTML_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());

/// Lowercase connecting words that belong inside a name ("Rafael dos Anjos").
const NAME_PARTICLES: &[&str] = &[
    "de", "da", "das", "do", "dos", "du", "van", "von", "der", "den", "la", "le", "el", "al", "bin",
];

/// Capitalized headline words that are never part of a fighter's name. Matters
/// for title-cased headlines ("Dana White Confirms Jones Vs. Aspinall").
const NOT_NAME_WORDS: &[&str] = &[
    "a", "after", "against", "and", "announces", "at", "bantamweight", "before", "belt", "between",
    "bmf", "booked", "bout", "breaking", "callout", "championship", "co-main", "confirmed",
    "confirms", "could", "eyes", "exclusive", "featherweight", "fight", "flyweight", "for",
    "heavyweight", "how", "in", "interim", "is", "lightweight", "main", "middleweight", "new",
    "next", "odds", "official", "officially", "on", "picks", "predictions", "preview", "rematch",
    "report", "reportedly", "says", "set", "sources", "strawweight", "targeted", "targets", "the",
    "title", "ufc", "wants", "watch", "welterweight", "what", "who", "why", "will", "with",
    "women's",
];

/// Matchups announced in `item` that haven't happened as of `today`.
pub fn extract_matchups(item: &NewsItem, today: NaiveDate) -> Vec<Matchup> {
    let title = normalize(&item.title);
    let summary = item.summary.as_deref().map(|s| normalize(&strip_html(s)));
    let full_text = match &summary {
        Some(s) => format!("{title} {s}"),
        None => title.clone(),
    };

    if !UFC.is_match(&full_text) || RESULT_WORDS.is_match(&title) {
        return Vec::new();
    }

    let status = classify(&title);
    let event_name = EVENT
        .find(&title)
        .or_else(|| summary.as_deref().and_then(|s| EVENT.find(s)))
        .map(|m| m.as_str().trim().to_string());
    let anchor = item.published.map(|p| p.date_naive()).unwrap_or(today);
    let event_date =
        find_date(&title, anchor).or_else(|| summary.as_deref().and_then(|s| find_date(s, anchor)));

    match event_date {
        Some(date) if date < today => return Vec::new(),
        None if today - anchor > Duration::days(MAX_UNDATED_AGE_DAYS) => return Vec::new(),
        _ => {}
    }

    find_pairs(&title)
        .into_iter()
        .map(|(fighter_a, fighter_b)| Matchup {
            fighter_a,
            fighter_b,
            status,
            event_name: event_name.clone(),
            event_date,
        })
        .collect()
}

/// Rumor wording wins over confirmation wording ("Report: X vs. Y set for
/// UFC 320" is still a report), and anything without clear confirmation
/// wording is treated as a rumor.
fn classify(title: &str) -> FightStatus {
    if RUMOR_WORDS.is_match(title) {
        FightStatus::Rumored
    } else if CONFIRM_WORDS.is_match(title) {
        FightStatus::Confirmed
    } else {
        FightStatus::Rumored
    }
}

fn find_pairs(title: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let separators: Vec<_> = VS.find_iter(title).collect();
    for (i, sep) in separators.iter().enumerate() {
        let left_start = if i == 0 { 0 } else { separators[i - 1].end() };
        let right_end = separators.get(i + 1).map_or(title.len(), |next| next.start());
        let left = name_before(&title[left_start..sep.start()]);
        let right = name_after(&title[sep.end()..right_end]);
        if let (Some(a), Some(b)) = (left, right) {
            if !a.eq_ignore_ascii_case(&b) {
                pairs.push((a, b));
            }
        }
    }
    pairs
}

/// The name that ends right before "vs.", read backwards.
fn name_before(text: &str) -> Option<String> {
    let mut words = Vec::new();
    for raw in text.split_whitespace().rev() {
        if trailing_punct(raw) && !words.is_empty() {
            break;
        }
        let word = clean_word(raw);
        if !accept_word(word, real_words(&words)) {
            break;
        }
        words.push(word);
        if trailing_punct(raw) || leading_punct(raw) {
            break;
        }
    }
    words.reverse();
    finish_name(words)
}

/// The name that starts right after "vs.", read forwards.
fn name_after(text: &str) -> Option<String> {
    let mut words = Vec::new();
    for raw in text.split_whitespace() {
        let word = clean_word(raw);
        if !accept_word(word, real_words(&words)) {
            break;
        }
        words.push(word);
        if trailing_punct(raw) {
            break;
        }
    }
    finish_name(words)
}

/// Whether `word` can extend a name that already has `taken` non-particle words.
fn accept_word(word: &str, taken: usize) -> bool {
    if word.is_empty() || NOT_NAME_WORDS.contains(&word.to_lowercase().as_str()) {
        return false;
    }
    if NAME_PARTICLES.contains(&word) {
        return true;
    }
    if taken >= MAX_NAME_WORDS {
        return false;
    }
    let mut chars = word.chars();
    let first_upper = chars.next().is_some_and(|c| c.is_uppercase());
    // All-caps words are acronyms (UFC, PFL, BMF), not names.
    let is_acronym = word.len() > 1 && word.chars().all(|c| !c.is_lowercase());
    first_upper && !is_acronym && word.chars().all(|c| c.is_alphabetic() || "'’.-".contains(c))
}

fn finish_name(mut words: Vec<&str>) -> Option<String> {
    // Particles only make sense in the middle of a name.
    while words.first().is_some_and(|w| NAME_PARTICLES.contains(w)) {
        words.remove(0);
    }
    while words.last().is_some_and(|w| NAME_PARTICLES.contains(w)) {
        words.pop();
    }
    if real_words(&words) == 0 {
        return None;
    }
    Some(words.join(" "))
}

fn real_words(words: &[&str]) -> usize {
    words.iter().filter(|w| !NAME_PARTICLES.contains(w)).count()
}

fn clean_word(raw: &str) -> &str {
    let word = raw.trim_matches(|c: char| ",;:!?\"“”()[]".contains(c));
    // Possessives: "Jones' next fight", "Pereira's".
    let word = word.strip_suffix("'s").or_else(|| word.strip_suffix("’s")).unwrap_or(word);
    word.trim_end_matches(['\'', '’', '.'])
}

fn trailing_punct(raw: &str) -> bool {
    raw.ends_with([',', ';', ':', '!', '?', ')', '"', '”'])
}

fn leading_punct(raw: &str) -> bool {
    raw.starts_with(['(', '"', '“'])
}

/// First "Month day[, year]" in `text`. Without a year, takes the first
/// occurrence on or after `anchor` (with a little slack for recaps written
/// just after the date).
fn find_date(text: &str, anchor: NaiveDate) -> Option<NaiveDate> {
    let caps = DATE.captures(text)?;
    let month = month_number(&caps[1])?;
    let day: u32 = caps[2].parse().ok()?;
    if let Some(year) = caps.get(3) {
        return NaiveDate::from_ymd_opt(year.as_str().parse().ok()?, month, day);
    }
    let this_year = NaiveDate::from_ymd_opt(anchor.year(), month, day)?;
    if this_year >= anchor - Duration::days(7) {
        Some(this_year)
    } else {
        NaiveDate::from_ymd_opt(anchor.year() + 1, month, day)
    }
}

fn month_number(name: &str) -> Option<u32> {
    let months = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
    let prefix = name.get(..3)?.to_lowercase();
    months.iter().position(|m| *m == prefix).map(|i| i as u32 + 1)
}

fn strip_html(html: &str) -> String {
    HTML_TAG.replace_all(html, " ").into_owned()
}

fn normalize(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&#8217;", "’")
        .replace("&#039;", "'")
        .replace("&nbsp;", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 17).unwrap()
    }

    fn item(title: &str, summary: Option<&str>) -> NewsItem {
        NewsItem {
            title: title.into(),
            summary: summary.map(Into::into),
            url: "https://example.com/a".into(),
            published: Some(
                NaiveDate::from_ymd_opt(2026, 10, 16)
                    .unwrap()
                    .and_hms_opt(12, 0, 0)
                    .unwrap()
                    .and_utc(),
            ),
        }
    }

    fn one(title: &str, summary: Option<&str>) -> Matchup {
        let found = extract_matchups(&item(title, summary), today());
        assert_eq!(found.len(), 1, "{title}: {found:?}");
        found.into_iter().next().unwrap()
    }

    #[test]
    fn official_booking_with_event_and_date() {
        let m = one("Jon Jones vs. Tom Aspinall official for UFC 330 on Dec. 12", None);
        assert_eq!(m.fighter_a, "Jon Jones");
        assert_eq!(m.fighter_b, "Tom Aspinall");
        assert_eq!(m.status, FightStatus::Confirmed);
        assert_eq!(m.event_name.as_deref(), Some("UFC 330"));
        assert_eq!(m.event_date, NaiveDate::from_ymd_opt(2026, 12, 12));
    }

    #[test]
    fn report_is_rumored_even_when_it_says_set() {
        let m = one("Report: Islam Makhachev vs. Ilia Topuria set for UFC 331", None);
        assert_eq!(m.fighter_a, "Islam Makhachev");
        assert_eq!(m.fighter_b, "Ilia Topuria");
        assert_eq!(m.status, FightStatus::Rumored);
    }

    #[test]
    fn in_the_works_is_rumored() {
        let m = one("Sean O'Malley vs. Merab Dvalishvili trilogy in the works for UFC", None);
        assert_eq!(m.fighter_a, "Sean O'Malley");
        assert_eq!(m.fighter_b, "Merab Dvalishvili");
        assert_eq!(m.status, FightStatus::Rumored);
        assert_eq!(m.event_date, None);
    }

    #[test]
    fn title_case_headline_drops_leading_verbs() {
        let m = one("Dana White Confirms Jones Vs. Aspinall For UFC 330", None);
        assert_eq!(m.fighter_a, "Jones");
        assert_eq!(m.fighter_b, "Aspinall");
        assert_eq!(m.status, FightStatus::Confirmed);
    }

    #[test]
    fn event_prefix_and_rematch_number_are_not_names() {
        let m = one("UFC 331: Magomed Ankalaev vs. Alex Pereira 3 booked", None);
        assert_eq!(m.fighter_a, "Magomed Ankalaev");
        assert_eq!(m.fighter_b, "Alex Pereira");
        assert_eq!(m.event_name.as_deref(), Some("UFC 331"));
    }

    #[test]
    fn name_particles_are_kept() {
        let m = one("Rafael dos Anjos vs. Paddy Pimblett added to UFC London", None);
        assert_eq!(m.fighter_a, "Rafael dos Anjos");
        assert_eq!(m.fighter_b, "Paddy Pimblett");
    }

    #[test]
    fn several_matchups_in_one_headline() {
        let found = extract_matchups(
            &item("UFC Fight Night adds Burns vs. Brady, Lopes vs. Evloev", None),
            today(),
        );
        let names: Vec<_> = found.iter().map(|m| (m.fighter_a.as_str(), m.fighter_b.as_str())).collect();
        assert_eq!(names, [("Burns", "Brady"), ("Lopes", "Evloev")]);
    }

    #[test]
    fn date_and_event_can_come_from_summary() {
        let m = one(
            "Shevchenko vs. Grasso 4 official",
            Some("<p>The bout headlines UFC Fight Night on January 9 at the Apex.</p>"),
        );
        assert_eq!(m.event_name.as_deref(), Some("UFC Fight Night"));
        assert_eq!(m.event_date, NaiveDate::from_ymd_opt(2027, 1, 9));
    }

    #[test]
    fn results_and_past_fights_are_skipped() {
        assert!(extract_matchups(&item("UFC 329 results: Jones def. Aspinall", None), today()).is_empty());
        assert!(extract_matchups(&item("Jones beats Aspinall vs. nobody at UFC 329", None), today()).is_empty());
        assert!(extract_matchups(
            &item("Jones vs. Aspinall set for UFC 329 on Oct. 4, 2026", None),
            today()
        )
        .is_empty());
    }

    #[test]
    fn non_ufc_fights_are_skipped() {
        assert!(extract_matchups(&item("Canelo vs. Crawford set for September", None), today()).is_empty());
    }

    #[test]
    fn stale_undated_rumors_are_skipped() {
        let mut old = item("Jones vs. Aspinall in the works for UFC", None);
        old.published = Some(
            NaiveDate::from_ymd_opt(2026, 6, 1).unwrap().and_hms_opt(0, 0, 0).unwrap().and_utc(),
        );
        assert!(extract_matchups(&old, today()).is_empty());
    }
}
