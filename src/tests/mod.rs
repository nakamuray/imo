use super::*;
use crate::{generator, site};
use similar_asserts::assert_eq;
use std::cell::RefCell;
use std::sync::Once;

static INIT: Once = Once::new();
fn init() {
    INIT.call_once(|| {
        //std::env::set_var("TZ", "Asia/Tokyo");
        std::env::set_var("TZ", "JST");
    });
}

#[test]
fn test_empty() {
    init();

    let org_data = include_str!("empty.org");
    let expected = include_str!("empty.out.txt");

    let output = Rc::new(RefCell::new(String::new()));
    let mut site = site::Site::new(
        "Test Site".to_string(),
        Some(Url::parse("http://test.site/").unwrap()),
        true,
        false,
    );
    site.load_org_data(org_data.to_string());

    generator::generate(Rc::new(site), generator::Output::Test(output.clone()))
        .expect("generator success");

    assert_eq!(output.borrow().as_str(), expected);
}

#[test]
fn test_it() {
    init();

    let org_data = include_str!("it.org");
    let expected = include_str!("it.out.txt");

    let output = Rc::new(RefCell::new(String::new()));
    let mut site = site::Site::new(
        "Test Site".to_string(),
        Some(Url::parse("http://test.site/").unwrap()),
        true,
        false,
    );
    site.load_org_data(org_data.to_string());

    generator::generate(Rc::new(site), generator::Output::Test(output.clone()))
        .expect("generator success");

    assert_eq!(output.borrow().as_str(), expected);
}

#[test]
fn test_draft() {
    init();

    let org_data = include_str!("draft.org");
    let expected = include_str!("draft.out.txt");

    let output = Rc::new(RefCell::new(String::new()));
    let mut site = site::Site::new(
        "Test Site".to_string(),
        Some(Url::parse("http://test.site/").unwrap()),
        true,
        true,
    );
    site.load_org_data(org_data.to_string());

    generator::generate(Rc::new(site), generator::Output::Test(output.clone()))
        .expect("generator success");

    assert_eq!(output.borrow().as_str(), expected);
}
