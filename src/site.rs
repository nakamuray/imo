use chrono::{Datelike, NaiveDateTime};
use indextree::NodeEdge;
use orgize::{
    elements::{Element, Timestamp, Title},
    export::HtmlHandler,
    Headline, Org,
};
use std::borrow::Cow;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Error, Write};
use std::rc::Rc;
use url::Url;

use crate::utils::notice;

#[derive(Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct Id(String);

impl Id {
    pub fn new(id: String) -> Self {
        Id(id)
    }
    pub fn to_string(&self) -> String {
        self.0.clone()
    }
}

pub struct Article {
    pub id: Id,
    pub published: NaiveDateTime,
    pub updated: Option<NaiveDateTime>,
    pub title: String,
    pub org: Rc<RefCell<Org<'static>>>,
    pub headline: Headline,
    pub subids: Vec<Id>,
}

impl Article {
    pub fn html<E: From<Error>, H: HtmlHandler<E>>(&self, handler: &mut H) -> Result<String, E> {
        let mut buf = Vec::new();
        write_headline_html(&self.org.borrow(), &self.headline, &mut buf, handler)?;

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
    pub subid_to_articleid_map: BTreeMap<Id, Id>,
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
            subid_to_articleid_map: BTreeMap::new(),
        }
    }
    pub fn load_org_data(&mut self, data: String) {
        let org = Rc::new(RefCell::new(Org::parse_string(data)));

        let headlines = org.borrow().headlines().collect::<Vec<_>>();
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

                for subid in &article.subids {
                    self.subid_to_articleid_map
                        .insert(subid.clone(), article.id.clone());
                }

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

fn load_article(org: Rc<RefCell<Org<'static>>>, headline: Headline) -> Option<Article> {
    let mut org_ = org.borrow_mut();
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
    let id = get_id(&title).or_else(|| {
        notice(&format!(
            "headline \"{}\" has blog tag, but does not have ID",
            title.raw
        ));
        None
    })?;
    if id.0.is_empty() {
        notice(&format!(
            "headline \"{}\" has blog tag, but ID is empty",
            title.raw
        ));
        return None;
    }
    let title = title.raw.to_string();
    let subids = collect_ids(&headline, &org_);

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

    // detach (remove) headline node which have "PRIVATE" tag
    for subheadline in headlines(&headline, &org_) {
        let title = subheadline.title(&org_);
        if title.tags.contains(&Cow::Borrowed("PRIVATE")) {
            subheadline.detach(&mut org_);
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
        subids: subids,
    })
}

fn collect_ids(headline: &Headline, org: &Org) -> Vec<Id> {
    headlines(headline, org)
        .iter()
        .filter_map(|child| get_id(&child.title(org)))
        .collect()
}

fn headlines(headline: &Headline, org: &Org) -> Vec<Headline> {
    let mut r = Vec::new();
    for child in headline.children(org) {
        r.push(child);
        r.extend(headlines(&child, org));
    }
    r
}

pub fn get_id(title: &Title) -> Option<Id> {
    title.properties.iter().find_map(|(key, value)| {
        if key == "ID" {
            Some(Id::new(value.to_string()))
        } else {
            None
        }
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
