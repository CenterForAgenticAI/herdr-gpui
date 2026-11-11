//! The dashboard account, its cookies, and the login form.

use crate::dashboard::auth::{Account, SESSION, cookie, login_form, set_cookie};

#[test]
fn a_short_password_is_refused() {
    assert!(matches!(
        Account::new("admin", "short"),
        Err(crate::Error::WeakSecret { .. })
    ));
}

#[test]
fn only_the_right_pair_signs_in() {
    let account = Account::new("admin", "a long password").unwrap();
    assert!(account.check("admin", "a long password"));
    assert!(!account.check("admin", "a long passwore"));
    assert!(!account.check("root", "a long password"));
    assert!(!format!("{account:?}").contains("long"));
}

#[test]
fn cookies_expire_and_cannot_be_forged() {
    let account = Account::new("admin", "a long password").unwrap();
    let value = account.issue(1_000);
    assert!(account.valid(&value, 1_000));
    assert!(!account.valid(&value, 1_000 + SESSION.as_secs()));
    let (expires, tag) = value.split_once('.').unwrap();
    let extended = format!("{}.{tag}", expires.parse::<u64>().unwrap() + 1);
    assert!(!account.valid(&extended, 1_000));
    let other = Account::new("admin", "a long password").unwrap();
    assert!(!other.valid(&value, 1_000));
    assert!(!account.valid("garbage", 1_000));
}

#[test]
fn the_cookie_is_found_and_set_strictly() {
    assert_eq!(cookie("a=1; sheprd_relay=v.t; b=2"), Some("v.t"));
    assert_eq!(cookie("a=1"), None);
    let set = set_cookie(Some("v.t"));
    for attribute in ["HttpOnly", "Secure", "SameSite=Strict", "Path=/dashboard"] {
        assert!(set.contains(attribute), "{set}");
    }
    assert!(set_cookie(None).contains("Max-Age=0"));
}

#[test]
fn the_login_form_decodes() {
    assert_eq!(
        login_form(b"user=ad%20min&password=p%26ss+word"),
        Some(("ad min".to_owned(), "p&ss word".to_owned()))
    );
    assert_eq!(login_form(b"user=admin"), None);
    assert_eq!(login_form(&[0xff, 0xfe]), None);
}
