//! The dashboard's token, its cookies, and the login form.

use crate::dashboard::auth::{Account, MIN_TOKEN, SESSION, cookie, login_form, set_cookie};

const TOKEN: &str = "a dashboard token of enough length";

#[test]
fn a_short_token_is_refused() {
    assert!(matches!(
        Account::new(&"x".repeat(MIN_TOKEN - 1)),
        Err(crate::Error::WeakSecret { .. })
    ));
    assert!(Account::new(&"x".repeat(MIN_TOKEN)).is_ok());
}

#[test]
fn only_the_token_signs_in() {
    let account = Account::new(TOKEN).unwrap();
    assert!(account.check(TOKEN));
    assert!(!account.check("a dashboard token of enough lengtH"));
    assert!(!account.check(""));
    assert!(!format!("{account:?}").contains("dashboard"));
}

#[test]
fn cookies_expire_and_cannot_be_forged() {
    let account = Account::new(TOKEN).unwrap();
    let value = account.issue(1_000);
    assert!(account.valid(&value, 1_000));
    assert_eq!(
        account.expiry(&value, 1_000),
        Some(1_000 + SESSION.as_secs())
    );
    assert!(!account.valid(&value, 1_000 + SESSION.as_secs()));
    assert_eq!(account.expiry(&value, 1_000 + SESSION.as_secs()), None);
    let (expires, tag) = value.split_once('.').unwrap();
    let extended = format!("{}.{tag}", expires.parse::<u64>().unwrap() + 1);
    assert!(!account.valid(&extended, 1_000));
    let other = Account::new(TOKEN).unwrap();
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
    assert_eq!(login_form(b"token=a%2Bb+c%26d"), Some("a+b c&d".to_owned()));
    assert_eq!(login_form(b"token=+padded+"), Some("padded".to_owned()));
    assert_eq!(login_form(b"user=admin"), None);
    assert_eq!(login_form(&[0xff, 0xfe]), None);
}
