use crate::{handlers, site};
use askama::Template;
use atom_syndication::{ContentBuilder, EntryBuilder, FeedBuilder, LinkBuilder};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use filetime::{set_file_mtime, FileTime};
use orgize::export::{DefaultHtmlHandler, SyntectHtmlHandler};
use rust_embed::RustEmbed;
use std::fs;
use std::io::{stdout, Result, Write};
use std::path::PathBuf;
use std::rc::Rc;

#[derive(RustEmbed)]
#[folder = "static/"]
#[exclude = ".*"]
#[prefix = "static/"]
pub struct StaticFiles;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    site: &'a site::Site,
    base: String,
}

#[derive(Template)]
#[template(path = "archive.html")]
struct ArchiveTemplate<'a> {
    site: &'a site::Site,
    base: String,
    year: site::Year,
}

#[derive(Template)]
#[template(path = "articles/article.html")]
struct ArticleTemplate<'a, 'b> {
    site: &'a site::Site,
    article: &'b site::Article,
    base: String,
    content: String,
}

pub enum Output {
    Stdout,
    Directory(PathBuf),
    #[cfg(test)]
    Test(Rc<std::cell::RefCell<String>>),
}

impl Output {
    pub fn write(&self, path: &str, data: &str, mtime: Option<NaiveDateTime>) -> Result<()> {
        match self {
            Output::Stdout => {
                let datetime = if let Some(mtime) = mtime {
                    format!(" ({mtime})")
                } else {
                    "".to_string()
                };
                stdout().write_all(&format!("{}{}:\n", path, datetime).as_bytes())?;
                stdout().write_all(data.as_bytes())?;
            }
            Output::Directory(p) => {
                let mut p = p.clone();
                p.push(path);
                if let Some(parent) = p.parent() {
                    if !parent.exists() {
                        fs::create_dir_all(parent)?;
                    }
                }
                let mut file = fs::File::create(&p)?;
                file.write_all(data.as_bytes())?;

                if let Some(mtime) = mtime {
                    let mtime = FileTime::from_unix_time(
                        mtime.and_utc().timestamp(),
                        mtime.and_utc().timestamp_subsec_nanos(),
                    );
                    set_file_mtime(&p, mtime)?;
                }
            }
            #[cfg(test)]
            Output::Test(s) => {
                let datetime = if let Some(mtime) = mtime {
                    if path.starts_with("static/") {
                        // XXX: static files have unknown mtimes which couldn't provide expected
                        // value
                        " (XXXX-XX-XX XX:XX:XX)".to_string()
                    } else {
                        format!(" ({mtime})")
                    }
                } else {
                    "".to_string()
                };
                s.borrow_mut().push_str(&format!("{}{}:\n", path, datetime));
                s.borrow_mut().push_str(data);
            }
        }
        Ok(())
    }
}

pub fn generate(site: Rc<site::Site>, output: Output) -> Result<()> {
    let index = IndexTemplate {
        site: &site,
        base: "".to_string(),
    };
    let html = index.render().unwrap();
    output.write("index.html", &html, site.last_update)?;

    for (year, articles) in site.index.iter().rev().skip(1) {
        let archive = ArchiveTemplate {
            site: &site,
            base: "".to_string(),
            year: year.clone(),
        };
        let html = archive.render().unwrap();
        let last_update = articles
            .iter()
            .map(|a| a.updated.unwrap_or(a.published))
            .max();
        output.write(&format!("{}.html", year.0), &html, last_update)?;
    }

    let base = "../../".to_string();
    let mut handler = handlers::ImoHtmlHandler::new(
        site.clone(),
        base.clone(),
        SyntectHtmlHandler::new(DefaultHtmlHandler),
    );

    for article in site.articles.values() {
        let content = article.html(&mut handler)?;
        let tmpl = ArticleTemplate {
            site: &site,
            article: &article,
            base: base.clone(),
            content: content,
        };
        let html = tmpl.render().unwrap();
        let mtime = article.updated.unwrap_or(article.published);
        output.write(&article.path(), &html, Some(mtime))?;
    }

    if site.include_draft {
        for draft in site.drafts.values() {
            let content = draft.html(&mut handler)?;
            let tmpl = ArticleTemplate {
                site: &site,
                article: &draft,
                base: base.clone(),
                content: content,
            };
            let html = tmpl.render().unwrap();
            let mtime = draft.updated.unwrap_or(draft.published);
            output.write(&draft.path(), &html, Some(mtime))?;
        }
    }

    if site.feed {
        let site_url = site.url.as_ref().expect("atom feed needs site_url");
        handler.set_base(site_url.to_string());
        const FEED_ENTRY_COUNT: usize = 10;
        let mut recent_entries = Vec::new();
        for article in site
            .index
            .values()
            .rev()
            .flat_map(|articles| articles.iter().rev())
            .take(FEED_ENTRY_COUNT)
        {
            let entry_url = site_url.join(&article.path()).unwrap();
            let published = Local
                .from_local_datetime(&article.published)
                .unwrap()
                .with_timezone(&Utc);
            let updated = Local
                .from_local_datetime(&article.updated.unwrap_or(article.published))
                .unwrap()
                .with_timezone(&Utc);
            let content = ContentBuilder::default()
                .content_type(Some("html".to_string()))
                .value(Some(article.html(&mut handler)?))
                .build();
            let link = LinkBuilder::default().href(entry_url.to_string()).build();
            let entry = EntryBuilder::default()
                .title(article.title.clone())
                .id(&entry_url.to_string())
                .links(vec![link])
                .published(Some(published.into()))
                .updated(updated)
                .content(Some(content))
                .build();
            recent_entries.push(entry);
        }
        let mut feed = FeedBuilder::default()
            .title(site.name.clone())
            .id(site_url.to_string())
            .entries(recent_entries)
            .build();
        if let Some(updated) = site.last_update {
            let updated = Local
                .from_local_datetime(&updated)
                .unwrap()
                .with_timezone(&Utc);
            feed.set_updated(updated);
        }
        output.write("atom.xml", &feed.to_string(), site.last_update)?;
    }

    for filename in StaticFiles::iter() {
        let file = StaticFiles::get(&filename).unwrap();
        let mtime = file
            .metadata
            .last_modified()
            .and_then(|m| DateTime::from_timestamp(m as i64, 0).map(|m| m.naive_local()));
        output.write(&filename, std::str::from_utf8(&file.data).unwrap(), mtime)?;
    }

    Ok(())
}
