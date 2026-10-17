/// Where a source's fights come from.
pub enum SourceKind {
    /// News listing pages: matchups are read from headlines.
    NewsPages(&'static [&'static str]),
    /// Sherdog's published UFC fight cards.
    SherdogEvents,
}

pub struct Source {
    pub name: &'static str,
    pub kind: SourceKind,
    /// Published by the UFC itself, so anything it lists is confirmed.
    pub official: bool,
}

const fn news(name: &'static str, pages: &'static [&'static str]) -> Source {
    Source {
        name,
        kind: SourceKind::NewsPages(pages),
        official: false,
    }
}

pub const SOURCES: &[Source] = &[
    Source {
        name: "UFC.com",
        kind: SourceKind::NewsPages(&["https://www.ufc.com/events", "https://www.ufc.com/news"]),
        official: true,
    },
    Source {
        name: "Sherdog events",
        kind: SourceKind::SherdogEvents,
        official: false,
    },
    news("Sherdog", &["https://www.sherdog.com/news/news/list"]),
    news("Bleacher Report", &["https://bleacherreport.com/mma"]),
    news("MMA Fighting", &["https://www.mmafighting.com/latest-news"]),
    news("MMA Mania", &["https://www.mmamania.com/latest-news"]),
    news("MMA Junkie", &["https://mmajunkie.usatoday.com/"]),
    news("Bloody Elbow", &["https://bloodyelbow.com/"]),
    news("ESPN MMA", &["https://www.espn.com/mma/"]),
    news("MMA News", &["https://www.mmanews.com/"]),
    news("Cageside Press", &["https://cagesidepress.com/"]),
    news("BJPenn.com", &["https://www.bjpenn.com/mma-news/"]),
];
