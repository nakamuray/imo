use std::fs;
use std::io::Read;
use std::io::Result;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;
use url::Url;

use clap::Parser;

mod generator;
mod handlers;
mod site;
#[cfg(test)]
mod tests;
mod utils;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// site name
    #[clap(short = 'n', long)]
    site_name: String,

    /// site URL (used by atom feed)
    #[clap(short = 'u', long)]
    site_url: Option<Url>,

    /// generate atom feed
    #[clap(short, long)]
    feed: bool,

    /// output directory name (if not specified, write data to stdout)
    #[clap(short, long)]
    output: Option<String>,

    /// include "draft" article or not
    #[clap(short, long)]
    draft: bool,

    /// org files
    #[clap(required = true)]
    files: Vec<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let start = Instant::now();

    let mut site = site::Site::new(args.site_name, args.site_url, args.feed, args.draft);
    for fname in args.files {
        let mut f = fs::File::open(fname)?;
        let mut buf = String::new();
        f.read_to_string(&mut buf)?;

        site.load_org_data(buf);
    }

    let output = if let Some(path) = args.output {
        generator::Output::Directory(PathBuf::from(path))
    } else {
        generator::Output::Stdout
    };

    let site = Rc::new(site);

    generator::generate(site.clone(), output)?;

    let duration = start.elapsed();
    let articles = site.articles.len();
    let drafts = site.drafts.len();
    let indices = site.index.len();
    let statics = generator::StaticFiles::iter().count();
    if site.feed && site.include_draft {
        eprintln!(
            "generate {} files ({} articles, {} drafts, {} indices, 1 feed, {} static files) in {:.2}s",
            articles + drafts + indices + 1 + statics,
            articles,
            drafts,
            indices,
            statics,
            duration.as_secs_f32()
        );
    } else if site.feed {
        eprintln!(
            "generate {} files ({} articles, {} indices, 1 feed, {} static files) in {:.2}s",
            articles + indices + 1 + statics,
            articles,
            indices,
            statics,
            duration.as_secs_f32()
        );
    } else if site.include_draft {
        eprintln!(
            "generate {} files ({} articles, {} drafts, {} indices, {} static files) in {:.2}s",
            articles + drafts + indices + statics,
            articles,
            drafts,
            indices,
            statics,
            duration.as_secs_f32()
        );
    } else {
        eprintln!(
            "generate {} files ({} articles, {} indices, {} static files) in {:.2}s",
            articles + indices + statics,
            articles,
            indices,
            statics,
            duration.as_secs_f32()
        );
    }

    Ok(())
}
