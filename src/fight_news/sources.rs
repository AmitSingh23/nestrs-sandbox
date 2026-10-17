/// A news outlet's RSS/Atom feed.
pub struct Source {
    pub name: &'static str,
    pub feed_url: &'static str,
}

pub const SOURCES: &[Source] = &[
    Source { name: "MMA Fighting", feed_url: "https://www.mmafighting.com/rss/current.xml" },
    Source { name: "MMA Junkie", feed_url: "https://mmajunkie.usatoday.com/feed" },
    Source { name: "MMA Mania", feed_url: "https://www.mmamania.com/rss/current.xml" },
    Source { name: "Sherdog", feed_url: "https://www.sherdog.com/rss/news.xml" },
    Source { name: "Bloody Elbow", feed_url: "https://www.bloodyelbow.com/feed" },
    Source { name: "ESPN MMA", feed_url: "https://www.espn.com/espn/rss/mma/news" },
    Source { name: "MMA News", feed_url: "https://www.mmanews.com/feed" },
    Source { name: "Cageside Press", feed_url: "https://cagesidepress.com/feed/" },
    Source { name: "BJPenn.com", feed_url: "https://www.bjpenn.com/feed/" },
];
