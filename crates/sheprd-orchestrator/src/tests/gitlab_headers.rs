//! glab's `custom_headers`, such as the service token Cloudflare Access asks
//! for in front of a self-managed GitLab: read from its config, resolved from
//! the environment, sent only over HTTPS, and never shown.

use super::*;
use crate::gitlab::{self, Api, Scheme, cli_hosts};
use crate::repo::parse_remote_url;
use std::{
    ffi::OsString,
    io::{BufRead, BufReader, Write},
    net::TcpListener,
};

const SECRET: &str = "not-a-real-client-secret";

/// glab's config for an instance behind Cloudflare Access, as glab writes it.
const CONFIG: &str = "\
hosts:
    gitlab.example.com:
        api_protocol: https
        api_host: gitlab.example.com
        ssh_host: gitlab-ssh.example.com
        custom_headers:
            - name: CF-Access-Client-Id
              valueFromEnv: CF_ACCESS_CLIENT_ID
            - name: CF-Access-Client-Secret
              valueFromEnv: CF_ACCESS_CLIENT_SECRET
        user: penso
    gitlab.com:
        api_host: gitlab.com
";

fn environment(name: &str) -> Option<OsString> {
    match name {
        "CF_ACCESS_CLIENT_ID" => Some("client-id.access".into()),
        "CF_ACCESS_CLIENT_SECRET" => Some(SECRET.into()),
        _ => None,
    }
}

fn nothing_set(_: &str) -> Option<OsString> {
    None
}

fn api_with(
    remote: &str,
    config: &str,
    variable: &dyn Fn(&str) -> Option<OsString>,
) -> (SourceKey, Api) {
    let key = parse_remote_url(remote).unwrap();
    let api = Api::new(
        &key.host,
        Scheme::of_remote(remote),
        &cli_hosts::parse_with(config, variable),
    );
    (key, api)
}

/// The headers a request would carry over HTTPS, by lowercase name.
fn sent(api: &Api) -> Vec<(String, String)> {
    let request = api
        .headers
        .apply(ureq::get("https://unused.invalid/"), true)
        .unwrap();
    let mut headers: Vec<_> = request
        .headers_ref()
        .unwrap()
        .iter()
        .map(|(name, value)| (name.as_str().to_owned(), value.to_str().unwrap().to_owned()))
        .collect();
    headers.sort();
    headers
}

/// #873: a GitLab behind Cloudflare Access redirected every request to its
/// sign-in page, because the headers glab sends were never read. They are
/// now resolved from the environment and carried to the instance's API, and
/// only that instance's.
#[test]
fn an_instances_custom_headers_are_resolved_and_sent_to_its_api() {
    let (_, api) = api_with(
        "git@gitlab-ssh.example.com:group/project.git",
        CONFIG,
        &environment,
    );
    assert_eq!(api.host, "gitlab.example.com");
    assert_eq!(
        sent(&api),
        vec![
            (
                "cf-access-client-id".to_owned(),
                "client-id.access".to_owned()
            ),
            ("cf-access-client-secret".to_owned(), SECRET.to_owned()),
        ]
    );
    // The instance's own host, or its name typed in the account menu, carries
    // them too.
    let (_, own) = api_with(
        "https://gitlab.example.com/group/project.git",
        CONFIG,
        &environment,
    );
    assert_eq!(sent(&own).len(), 2);
    let cli = cli_hosts::parse_with(CONFIG, &environment);
    assert_eq!(sent(&Api::of_instance("gitlab.example.com", &cli)).len(), 2);
    // Another instance and an unknown host get none.
    assert!(sent(&Api::of_instance("gitlab.com", &cli)).is_empty());
    let (_, other) = api_with(
        "git@gitlab.other.example:group/project.git",
        CONFIG,
        &environment,
    );
    assert!(sent(&other).is_empty());
}

/// Keys the list may hold beyond the header: a `-` on its own line, items at
/// the key's own indentation, quoted values, and a deeper host-like key that
/// is not a host field.
#[test]
fn the_forms_of_a_custom_headers_list_are_read() {
    let config = "\
hosts:
  gitlab.example.com:
    custom_headers:
    - name: 'X-One'
      value: \"one\" # a comment
    -
      name: X-Two
      valueFromEnv: CF_ACCESS_CLIENT_ID
      ssh_host: not-a-field.example.com
    api_host: api.example.com
";
    let hosts = cli_hosts::parse_with(config, &environment);
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].api_host.as_deref(), Some("api.example.com"));
    assert_eq!(hosts[0].ssh_host, None);
    assert!(hosts[0].headers.sends("x-one", "one"));
    assert!(hosts[0].headers.sends("X-Two", "client-id.access"));
    assert_eq!(hosts[0].headers.names().count(), 2);
    // Without the environment, as for host mapping alone, none are read.
    assert_eq!(cli_hosts::parse(config)[0].headers.names().count(), 0);
}

/// A header value never shows in `Debug`, a log or an error.
#[test]
fn a_header_value_is_never_shown() {
    let (_, api) = api_with(
        "git@gitlab-ssh.example.com:group/project.git",
        CONFIG,
        &environment,
    );
    let debug = format!("{api:?} {:?}", cli_hosts::parse_with(CONFIG, &environment));
    assert!(!debug.contains(SECRET), "{debug}");
    assert!(!debug.contains("client-id.access"), "{debug}");
}

/// A variable the login shell does not set fails the request before it is
/// made, as glab does, naming the header and the variable.
#[test]
fn an_unset_variable_refuses_the_request_unsent() {
    let address = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let (key, mut api) = api_with(
        "git@gitlab-ssh.example.com:group/project.git",
        CONFIG,
        &nothing_set,
    );
    // Nothing listens there: a request that was made would fail to connect.
    api.host = address.to_string();
    let error = gitlab::sync((&key, &api), None, None, &|| false, &mut None)
        .err()
        .unwrap();
    assert!(
        matches!(
            &error,
            Error::Gitlab(gitlab::Error::HeaderUnset { header, variable })
                if header == "CF-Access-Client-Id" && variable == "CF_ACCESS_CLIENT_ID"
        ),
        "{error:?}"
    );
    assert!(matches!(&error, Error::Gitlab(source) if source.needs_user()));
}

/// A header from a command is never run; one with no value, or two, or a
/// name no request can carry is refused.
#[test]
fn a_header_glab_could_not_send_safely_is_refused() {
    let refusal = |item: &str| {
        let config = format!(
            "hosts:\n    gitlab.example.com:\n        custom_headers:\n            - {item}\n"
        );
        let host = cli_hosts::parse_with(&config, &environment).remove(0);
        host.headers
            .apply(ureq::get("https://unused.invalid/"), true)
            .err()
            .unwrap()
    };
    assert!(matches!(
        refusal("name: X-Token\n              valueFromCommand: cat /etc/token"),
        gitlab::Error::HeaderCommand { header } if header == "X-Token"
    ));
    for item in [
        "name: X-Token",
        "name: X-Token\n              value: a\n              valueFromEnv: CF_ACCESS_CLIENT_ID",
        "name: Bad Header\n              value: a",
        "value: a",
    ] {
        assert!(
            matches!(refusal(item), gitlab::Error::HeaderInvalid { .. }),
            "{item}"
        );
    }
}

/// Over plain HTTP the headers are withheld, as a token is: anyone watching
/// the connection could take them.
#[test]
fn headers_are_withheld_over_plain_http() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        serve(&listener, b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]")
    });
    let config = CONFIG
        .replace("api_protocol: https", "api_protocol: http")
        .replace(
            "api_host: gitlab.example.com",
            &format!("api_host: {address}"),
        );
    let (key, api) = api_with(
        "git@gitlab-ssh.example.com:group/project.git",
        &config,
        &environment,
    );
    // Only the issues are read: the listing stops at the first error, and
    // here there is none, so the merge requests follow.
    let _ = gitlab::sync((&key, &api), None, None, &|| false, &mut None);
    let head = server.join().unwrap();
    assert!(head[0].starts_with("GET /api/v4/"), "{head:?}");
    assert!(
        !head
            .iter()
            .any(|line| line.to_ascii_lowercase().starts_with("cf-access")),
        "{head:?}"
    );
}

/// An access proxy that refuses the request redirects it to its sign-in
/// page. The error names where, without the page's query, and asks for
/// `custom_headers` rather than reporting an unexplained status.
#[test]
fn a_redirect_names_its_target_without_its_query() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        serve(&listener, b"HTTP/1.1 302 Found\r\nLocation: https://team.cloudflareaccess.com/cdn-cgi/access/login/gitlab.example.com?kid=state-token\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
    });
    let key = parse_remote_url("git@gitlab-ssh.example.com:group/project.git").unwrap();
    let api = Api {
        instance: "gitlab.example.com".into(),
        host: address.to_string(),
        scheme: Scheme::Http,
        headers: Default::default(),
    };
    let error = gitlab::sync((&key, &api), None, None, &|| false, &mut None)
        .err()
        .unwrap();
    server.join().unwrap();
    assert!(
        matches!(
            &error,
            Error::Gitlab(gitlab::Error::Redirect { origin, location, status: 302 })
                if *origin == format!("http://{address}")
                    && location == "https://team.cloudflareaccess.com"
        ),
        "{error:?}"
    );
    assert!(!error.to_string().contains("state-token"), "{error}");
}

/// Answers one request with `reply` and returns its head.
fn serve(listener: &TcpListener, reply: &[u8]) -> Vec<String> {
    let (stream, _) = listener.accept().unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut head = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line.trim_end().is_empty() {
            break;
        }
        head.push(line.trim_end().to_owned());
    }
    (&stream).write_all(reply).unwrap();
    head
}
