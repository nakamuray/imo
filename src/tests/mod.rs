use super::*;
use crate::{generator, site};
use similar_asserts::assert_eq;
use std::cell::RefCell;

#[test]
fn test_empty() {
    let org_data = include_str!("empty.org");
    let expected = include_str!("empty.out.txt");

    let output = Rc::new(RefCell::new(String::new()));
    let mut site = site::Site::new(
        "Test Site".to_string(),
        Some(Url::parse("http://test.site/").unwrap()),
        true,
    );
    site.load_org_data(org_data.to_string());

    generator::generate(Rc::new(site), generator::Output::Test(output.clone()))
        .expect("generator success");

    assert_eq!(output.borrow().as_str(), expected);
}

#[test]
fn test_it() {
    let org_data = include_str!("it.org");
    let expected = include_str!("it.out.txt");

    let output = Rc::new(RefCell::new(String::new()));
    let mut site = site::Site::new(
        "Test Site".to_string(),
        Some(Url::parse("http://test.site/").unwrap()),
        true,
    );
    site.load_org_data(org_data.to_string());

    generator::generate(Rc::new(site), generator::Output::Test(output.clone()))
        .expect("generator success");

    assert_eq!(output.borrow().as_str(), expected);
}
