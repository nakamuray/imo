use chrono::{Datelike, NaiveDateTime};
use indextree::NodeEdge;
use orgize::{
    elements::{Element, Timestamp},
    export::HtmlHandler,
    Headline, Org,
};
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Error, Write};
use std::rc::Rc;
use std::sync::Mutex;
use url::Url;

#[derive(Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct Id(String);

impl Id {
    pub fn new(id: String) -> Self {
        Id(id)
    }
}

pub struct Article {
    pub id: Id,
    pub published: NaiveDateTime,
    pub updated: Option<NaiveDateTime>,
    pub title: String,
    pub org: Rc<Mutex<Org<'static>>>,
    pub headline: Headline,
}

impl Article {
    pub fn html<E: From<Error>, H: HtmlHandler<E>>(&self, handler: &mut H) -> Result<String, E> {
        let mut buf = Vec::new();
        write_headline_html(&self.org.lock().unwrap(), &self.headline, &mut buf, handler)?;

        Ok(String::from_utf8(buf).unwrap())
    }
    pub fn path(&self) -> String {
        id_to_path(&self.id)
    }
}

pub fn id_to_path(id: &Id) -> String {
    format!("articles/{}/{}.html", id.0.chars().last().unwrap(), id.0)
}

impl PartialEq for Article {
    fn eq(&self, other: &Self) -> bool {
        self.id.eq(&other.id)
    }
}

impl Eq for Article {
    fn assert_receiver_is_total_eq(&self) {}
}

impl PartialOrd for Article {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.published != other.published {
            self.published.partial_cmp(&other.published)
        } else {
            self.id.partial_cmp(&other.id)
        }
    }
}

impl Ord for Article {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.published != other.published {
            self.published.cmp(&other.published)
        } else {
            self.id.cmp(&other.id)
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct Year(pub i32);

pub struct Site {
    pub name: String,
    pub url: Option<Url>,
    pub feed: bool,
    pub index: BTreeMap<Year, BTreeSet<Rc<Article>>>,
    pub articles: BTreeMap<Id, Rc<Article>>,
    pub last_update: Option<NaiveDateTime>,
}

impl Site {
    pub fn new(name: String, url: Option<Url>, feed: bool) -> Self {
        Self {
            name: name,
            url: url,
            feed: feed,
            index: BTreeMap::new(),
            articles: BTreeMap::new(),
            last_update: None,
        }
    }
    pub fn load_org_data(&mut self, data: String) {
        let org = Rc::new(Mutex::new(Org::parse_string(data)));

        let headlines = org.lock().unwrap().headlines().collect::<Vec<_>>();
        for headline in headlines {
            if let Some(article) = load_article(org.clone(), headline) {
                let article = Rc::new(article);

                let updated = article.updated.unwrap_or(article.published);
                if let Some(last_update) = self.last_update {
                    if updated > last_update {
                        self.last_update = Some(updated);
                    }
                } else {
                    self.last_update = Some(updated);
                }

                self.articles.insert(article.id.clone(), article.clone());
                let year = Year(article.published.year());
                if let Some(set) = self.index.get_mut(&year) {
                    set.insert(article);
                } else {
                    let mut set = BTreeSet::new();
                    set.insert(article);
                    self.index.insert(year, set);
                }
            }
        }
    }
}

fn notice(message: &str) {
    eprintln!("\x1b[90mNOTICE: {}\x1b[0m", message);
}

fn load_article(org: Rc<Mutex<Org<'static>>>, headline: Headline) -> Option<Article> {
    let mut org_ = org.lock().unwrap();
    let title = headline.title(&org_);
    if !title.tags.contains(&Cow::Borrowed("blog")) {
        return None;
    }
    let published = match title.planning.as_ref()?.scheduled.as_ref()? {
        Timestamp::Active {
            start,
            repeater: None,
            delay: None,
        } => Some(start.into()),
        Timestamp::Inactive {
            start,
            repeater: None,
            delay: None,
        } => Some(start.into()),
        _ => {
            notice(&format!(
                "headline \"{}\" has blog tag, but not SCHEDULED",
                title.raw
            ));
            None
        }
    }?;
    let id = title
        .properties
        .iter()
        .find_map(|(key, value)| if key == "ID" { Some(value) } else { None })
        .or_else(|| {
            notice(&format!(
                "headline \"{}\" has blog tag, but does not have ID",
                title.raw
            ));
            None
        })?;
    if id.is_empty() {
        notice(&format!(
            "headline \"{}\" has blog tag, but ID is empty",
            title.raw
        ));
        return None;
    }
    let title = title.raw.to_string();
    let id = Id(id.to_string());

    let mut updated = None;

    if let Some(sec_node) = headline.section_node() {
        let children = sec_node.children(&org_.arena()).collect::<Vec<_>>();
        for child in children {
            if let Element::Drawer(drawer) = &org_[child] {
                if drawer.name == "LOGBOOK" {
                    for c in child.descendants(&org_.arena()) {
                        // if LOGBOOK has timestamp, record it as an "updated"
                        if let Element::Timestamp(timestamp) = &org_[c] {
                            if let Timestamp::Active {
                                start,
                                repeater: None,
                                delay: None,
                            }
                            | Timestamp::Inactive {
                                start,
                                repeater: None,
                                delay: None,
                            } = timestamp
                            {
                                let start = start.into();
                                if start > published {
                                    if let Some(u) = updated {
                                        if start > u {
                                            updated = Some(start);
                                        }
                                    } else {
                                        updated = Some(start);
                                    }
                                }
                            }
                        }
                    }
                    // remove LOGBOOK drawer from section node
                    child.detach(org_.arena_mut());
                }
            }
        }
    }

    drop(org_);

    Some(Article {
        id: id,
        published: published,
        updated: updated,
        title: title,
        org: org,
        headline: headline,
    })
}

fn write_headline_html<W, H, E>(
    org: &Org,
    headline: &Headline,
    mut writer: W,
    handler: &mut H,
) -> Result<(), E>
where
    W: Write,
    E: From<Error>,
    H: HtmlHandler<E>,
{
    let node_id = headline.headline_node();
    for edge in node_id.traverse(org.arena()) {
        match edge {
            NodeEdge::Start(node) => {
                let elem = &org[node];
                if let Element::Title(title) = elem {
                    let mut title = title.clone();
                    // adjust all headline level started from 2 (<h2>)
                    title.level = 2 + title.level - headline.level();
                    handler.start(&mut writer, &Element::Title(title))?
                } else {
                    handler.start(&mut writer, elem)?
                }
            }
            NodeEdge::End(node) => {
                let elem = &org[node];
                if let Element::Title(title) = elem {
                    let mut title = title.clone();
                    // adjust all headline level started from 2 (<h2>)
                    title.level = 2 + title.level - headline.level();
                    handler.end(&mut writer, &Element::Title(title))?
                } else {
                    handler.end(&mut writer, elem)?
                }
            }
        }
    }
    Ok(())
}
