use orgize::{
    elements::Element,
    export::{HtmlEscape, HtmlHandler},
};
use std::io::{Error, Write};
use std::marker::PhantomData;
use url::{ParseError, Url};

use crate::site::{id_to_path, Id};

pub struct ImoHtmlHandler<E: From<Error>, H: HtmlHandler<E>> {
    base: String,
    inner: H,
    e: PhantomData<E>,
}

impl<E: From<Error>, H: HtmlHandler<E>> ImoHtmlHandler<E, H> {
    pub fn new(base: String, inner: H) -> Self {
        ImoHtmlHandler {
            base,
            inner,
            ..Default::default()
        }
    }
    pub fn set_base(&mut self, base: String) {
        self.base = base;
    }
}

impl<E: From<Error>, H: HtmlHandler<E>> Default for ImoHtmlHandler<E, H> {
    fn default() -> Self {
        ImoHtmlHandler {
            base: "".to_string(),
            inner: H::default(),
            e: PhantomData,
        }
    }
}

impl<E: From<Error>, H: HtmlHandler<E>> HtmlHandler<E> for ImoHtmlHandler<E, H> {
    fn start<W: Write>(&mut self, mut w: W, element: &Element) -> Result<(), E> {
        match element {
            Element::Link(link) => {
                if link.path.starts_with("id:") {
                    let id = link.path[3..].to_string();
                    write!(
                        w,
                        "<a href=\"{}{}\">{}</a>",
                        HtmlEscape(&self.base),
                        HtmlEscape(id_to_path(&Id::new(id))),
                        HtmlEscape(link.desc.as_ref().unwrap_or(&link.path))
                    )?;
                } else if link.path.starts_with("file:") {
                    // remove "file:" prefix and re-start
                    let mut fixed = link.clone();
                    fixed.path = link.path[5..].into();
                    self.start(w, &Element::Link(fixed))?;
                } else {
                    let link =
                        if let Err(ParseError::RelativeUrlWithoutBase) = Url::parse(&link.path) {
                            // prepend `base` to path if it is local, relative link
                            let mut link = link.clone();
                            link.path = format!("{}{}", &self.base, link.path).into();
                            link
                        } else {
                            link.clone()
                        };
                    let is_image = link
                        .path
                        .rsplit("/")
                        .next()
                        .and_then(|may_filename| may_filename.rsplit_once("."))
                        .and_then(|(_, ext)| {
                            if ["jpeg", "jpg", "png", "svg"].contains(&ext) {
                                Some(())
                            } else {
                                None
                            }
                        })
                        .is_some();
                    if is_image {
                        write!(
                            w,
                            "<a href=\"{path}\"><img src=\"{path}\"></a>",
                            path = HtmlEscape(&link.path),
                        )?;
                    } else {
                        self.inner.start(w, &Element::Link(link))?;
                    }
                }
            }
            Element::FnDef(fn_def) => {
                write!(w, "<small>[{}]</small>", fn_def.label)?;
            }
            Element::FnRef(fn_ref) => {
                write!(w, "<small>[{}]</small>", fn_ref.label)?;
            }
            _ => self.inner.start(w, element)?,
        }
        Ok(())
    }
    fn end<W: Write>(&mut self, w: W, element: &Element) -> Result<(), E> {
        match element {
            _ => self.inner.end(w, element)?,
        }
        Ok(())
    }
}
